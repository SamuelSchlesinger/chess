//! Magic bitboards for sliding-piece attacks.
//!
//! Each square has a "magic" multiplier that hashes the relevant occupancy
//! (the blocker squares on the piece's rays, excluding board edges) into a dense
//! index, so an attack lookup is a single `(occ & mask).wrapping_mul(magic) >>
//! shift` followed by a table read. This replaces the classical ray scan in the
//! hot path.
//!
//! The magics are *found at startup* by a deterministic search (a fixed-seed
//! xorshift PRNG), so there are no hand-embedded constants to get wrong and the
//! result is reproducible across runs and platforms. Initialization builds the
//! ~108k-entry rook table and ~5k-entry bishop table once via [`LazyLock`],
//! bootstrapping the true attacks from the classical scan in [`crate::attacks`].
//!
//! Total table memory: rook ≈ 102,400 × 8 B ≈ 800 KiB, bishop ≈ 5,248 × 8 B ≈
//! 41 KiB — shared across all boards, so it does not affect per-board size.

use crate::attacks::{bishop_attacks_classical, rook_attacks_classical};
use crate::bitboard::Bitboard;
use crate::types::Square;
use std::sync::LazyLock;

/// Per-square magic descriptor into a shared attack table.
struct Magic {
    mask: u64,
    magic: u64,
    shift: u32,
    offset: usize,
}

impl Magic {
    #[inline]
    fn index(&self, occ: u64) -> usize {
        self.offset + (((occ & self.mask).wrapping_mul(self.magic)) >> self.shift) as usize
    }
}

struct SlidingTables {
    // Inline [Magic; 64] (no heap pointer to chase) plus a single flat attack
    // table behind one pointer — the textbook compact magic layout.
    rook: [Magic; 64],
    bishop: [Magic; 64],
    rook_table: Box<[u64]>,
    bishop_table: Box<[u64]>,
}

/// Deterministic xorshift64 PRNG used to search for magics.
struct Rng(u64);

impl Rng {
    #[inline]
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// Sparse candidate (few set bits) — magics with few bits work best.
    #[inline]
    fn sparse(&mut self) -> u64 {
        self.next() & self.next() & self.next()
    }
}

/// Relevant-occupancy mask for a rook on `sq` (rays minus the board edges).
fn rook_mask(sq: u8) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask = 0u64;
    let mut rr = r + 1;
    while rr <= 6 {
        mask |= 1u64 << (rr * 8 + f);
        rr += 1;
    }
    rr = r - 1;
    while rr >= 1 {
        mask |= 1u64 << (rr * 8 + f);
        rr -= 1;
    }
    let mut ff = f + 1;
    while ff <= 6 {
        mask |= 1u64 << (r * 8 + ff);
        ff += 1;
    }
    ff = f - 1;
    while ff >= 1 {
        mask |= 1u64 << (r * 8 + ff);
        ff -= 1;
    }
    mask
}

/// Relevant-occupancy mask for a bishop on `sq` (diagonals minus the edges).
fn bishop_mask(sq: u8) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask = 0u64;
    let dirs = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
    for (dr, df) in dirs {
        let mut rr = r + dr;
        let mut ff = f + df;
        while (1..=6).contains(&rr) && (1..=6).contains(&ff) {
            mask |= 1u64 << (rr * 8 + ff);
            rr += dr;
            ff += df;
        }
    }
    mask
}

/// Enumerate every subset of `mask` (Carry-Rippler), calling `f` with each.
fn for_each_subset(mask: u64, mut f: impl FnMut(u64)) {
    let mut subset = 0u64;
    loop {
        f(subset);
        subset = subset.wrapping_sub(mask) & mask;
        if subset == 0 {
            break;
        }
    }
}

/// Find a magic for one square and return `(magic, filled_table)`.
fn find_magic(
    sq: u8,
    mask: u64,
    rng: &mut Rng,
    classical: impl Fn(Square, Bitboard) -> Bitboard,
) -> (u64, Vec<u64>) {
    let bits = mask.count_ones();
    let size = 1usize << bits;
    let shift = 64 - bits;

    // Precompute every occupancy subset and its true attack set.
    let mut occs = Vec::with_capacity(size);
    let mut atts = Vec::with_capacity(size);
    for_each_subset(mask, |subset| {
        occs.push(subset);
        atts.push(classical(Square(sq), Bitboard(subset)).0);
    });

    const EMPTY: u64 = u64::MAX; // sentinel; real attack sets never equal this
    let mut table = vec![EMPTY; size];
    loop {
        let magic = rng.sparse();
        // Quick reject: the high byte of mask*magic should spread enough bits.
        if (mask.wrapping_mul(magic) & 0xFF00_0000_0000_0000).count_ones() < 6 {
            continue;
        }
        for slot in table.iter_mut() {
            *slot = EMPTY;
        }
        let mut ok = true;
        for k in 0..size {
            let idx = ((occs[k].wrapping_mul(magic)) >> shift) as usize;
            if table[idx] == EMPTY {
                table[idx] = atts[k];
            } else if table[idx] != atts[k] {
                ok = false;
                break;
            }
        }
        if ok {
            return (magic, table);
        }
    }
}

fn build_side(
    classical: impl Fn(Square, Bitboard) -> Bitboard + Copy,
    mask_fn: impl Fn(u8) -> u64,
    rng: &mut Rng,
) -> ([Magic; 64], Box<[u64]>) {
    let mut magics = Vec::with_capacity(64);
    let mut table = Vec::new();
    for sq in 0..64u8 {
        let mask = mask_fn(sq);
        let (magic, sub) = find_magic(sq, mask, rng, classical);
        let offset = table.len();
        table.extend_from_slice(&sub);
        magics.push(Magic {
            mask,
            magic,
            shift: 64 - mask.count_ones(),
            offset,
        });
    }
    let arr: [Magic; 64] = match magics.try_into() {
        Ok(a) => a,
        Err(_) => unreachable!("exactly 64 magics built"),
    };
    (arr, table.into_boxed_slice())
}

fn build() -> SlidingTables {
    let mut rng = Rng(0x00C0_FFEE_1234_5678);
    let (rook, rook_table) = build_side(rook_attacks_classical, rook_mask, &mut rng);
    let (bishop, bishop_table) = build_side(bishop_attacks_classical, bishop_mask, &mut rng);
    SlidingTables {
        rook,
        bishop,
        rook_table,
        bishop_table,
    }
}

static TABLES: LazyLock<SlidingTables> = LazyLock::new(build);

/// Rook attacks from `sq` under occupancy `occ`, via magic lookup.
#[inline]
pub fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let t = &*TABLES;
    // SAFETY: `sq.index() < 64` is a `Square` invariant, and `m.index(occ)`
    // always lands inside this square's contiguous sub-table (the magic was
    // chosen so every masked occupancy maps within `[offset, offset+size)`,
    // and the sub-table was appended into `rook_table`).
    unsafe {
        let m = t.rook.get_unchecked(sq.index());
        Bitboard(*t.rook_table.get_unchecked(m.index(occ.0)))
    }
}

/// Bishop attacks from `sq` under occupancy `occ`, via magic lookup.
#[inline]
pub fn bishop_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let t = &*TABLES;
    // SAFETY: see `rook_attacks`.
    unsafe {
        let m = t.bishop.get_unchecked(sq.index());
        Bitboard(*t.bishop_table.get_unchecked(m.index(occ.0)))
    }
}

/// Force table initialization (useful before timing so init isn't measured).
pub fn init() {
    LazyLock::force(&TABLES);
}
