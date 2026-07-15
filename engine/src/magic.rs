//! Magic bitboards for sliding-piece attacks — built entirely at compile time.
//!
//! Each square has a "magic" multiplier that hashes the relevant occupancy (the
//! blocker squares on the piece's rays, excluding board edges) into a dense
//! index, so an attack lookup is a single `(occ & mask) * magic >> shift`
//! followed by a table read.
//!
//! Everything here is `const`/`static`: the magics were found once by a
//! deterministic search (see the `dump_magics` test in git history) and are
//! embedded below; the masks, shifts, offsets, and the ~108k-entry attack
//! tables are all evaluated at compile time and live in read-only `.rodata`.
//! There is therefore **no runtime initialization, no `LazyLock` atomic, and no
//! heap pointer** on the lookup path — just static-address indexing, which is
//! both faster per call and better for instruction/data locality than a
//! lazily-built `Box<[u64]>`.
//!
//! Table memory (shared across all boards, zero per-board bytes): rook
//! `102_400 × 8 B ≈ 800 KiB`, bishop `5_248 × 8 B ≈ 41 KiB`.

use crate::bitboard::Bitboard;
use crate::types::Square;

// Ray directions as (file_delta, rank_delta).
const ROOK_DIRS: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const BISHOP_DIRS: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];

/// Relevant-occupancy mask for a rook on `sq` (rays minus the board edges).
const fn rook_mask(sq: u8) -> u64 {
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
const fn bishop_mask(sq: u8) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask = 0u64;
    let mut d = 0;
    while d < 4 {
        let dr = BISHOP_DIRS[d].1 as i32;
        let df = BISHOP_DIRS[d].0 as i32;
        let mut rr = r + dr;
        let mut ff = f + df;
        while rr >= 1 && rr <= 6 && ff >= 1 && ff <= 6 {
            mask |= 1u64 << (rr * 8 + ff);
            rr += dr;
            ff += df;
        }
        d += 1;
    }
    mask
}

/// True sliding attack from `sq` under occupancy `occ`, by walking each ray
/// until (and including) the first blocker. Used to fill the tables at compile
/// time; the runtime path never calls this.
const fn slider_attack(sq: u8, occ: u64, dirs: &[(i8, i8); 4]) -> u64 {
    let f0 = (sq % 8) as i8;
    let r0 = (sq / 8) as i8;
    let mut result = 0u64;
    let mut d = 0;
    while d < 4 {
        let df = dirs[d].0;
        let dr = dirs[d].1;
        let mut f = f0 + df;
        let mut r = r0 + dr;
        while f >= 0 && f < 8 && r >= 0 && r < 8 {
            let s = (r * 8 + f) as u8;
            result |= 1u64 << s;
            if occ & (1u64 << s) != 0 {
                break;
            }
            f += df;
            r += dr;
        }
        d += 1;
    }
    result
}

const fn masks_for(rook: bool) -> [u64; 64] {
    let mut m = [0u64; 64];
    let mut s = 0;
    while s < 64 {
        m[s] = if rook {
            rook_mask(s as u8)
        } else {
            bishop_mask(s as u8)
        };
        s += 1;
    }
    m
}

const fn shifts_for(masks: &[u64; 64]) -> [u32; 64] {
    let mut sh = [0u32; 64];
    let mut s = 0;
    while s < 64 {
        sh[s] = 64 - masks[s].count_ones();
        s += 1;
    }
    sh
}

/// Per-square offsets into the flat attack table, and the total size.
const fn layout(masks: &[u64; 64]) -> ([usize; 64], usize) {
    let mut offs = [0usize; 64];
    let mut total = 0usize;
    let mut s = 0;
    while s < 64 {
        offs[s] = total;
        total += 1usize << masks[s].count_ones();
        s += 1;
    }
    (offs, total)
}

/// Fill a flat attack table of size `N` for one slider kind.
const fn build_table<const N: usize>(
    masks: &[u64; 64],
    magics: &[u64; 64],
    offsets: &[usize; 64],
    shifts: &[u32; 64],
    dirs: &[(i8, i8); 4],
) -> [u64; N] {
    let mut table = [0u64; N];
    let mut sq = 0;
    while sq < 64 {
        let mask = masks[sq];
        let magic = magics[sq];
        let off = offsets[sq];
        let shift = shifts[sq];
        // Carry-Rippler enumeration of every subset of `mask`.
        let mut subset = 0u64;
        loop {
            let idx = off + ((subset.wrapping_mul(magic) >> shift) as usize);
            table[idx] = slider_attack(sq as u8, subset, dirs);
            subset = subset.wrapping_sub(mask) & mask;
            if subset == 0 {
                break;
            }
        }
        sq += 1;
    }
    table
}

// Deterministically-searched magics (fixed-seed xorshift; see git history).
#[rustfmt::skip]
const ROOK_MAGICS: [u64; 64] = [
    0x8280004002218050, 0x2100210080400011, 0x0200201008420080, 0x0200100804204200,
    0xc88004008800804e, 0x130004000813000a, 0x1080208002000100, 0x0100084021001082,
    0x4002800240008038, 0x0010802000804000, 0x2001001020004100, 0xd011001000a10188,
    0x0449000408011100, 0x0108012004104008, 0x1104001014014288, 0x000100010030408a,
    0x0040008002984160, 0x2060004000403002, 0x0010002008002402, 0x1801010008201002,
    0x2900808004000800, 0x0004008004020080, 0x0000040030812208, 0x200002002040810c,
    0x200080008020400b, 0x0060002040100040, 0x8ca8200080100880, 0x0000100080080080,
    0x4000100500080100, 0x0800040080800200, 0x0028100400010208, 0x2000088200110044,
    0x0044400028800080, 0x60c0804000802002, 0x4020088020801000, 0x1110000800808010,
    0x1000804402800800, 0x4008800400800200, 0x0200a2180c000190, 0x2001408052000104,
    0x0180002000414000, 0x0000500020044000, 0x9020200010008080, 0x0100081200420020,
    0xa080080004008080, 0x0100020004008080, 0x0000521001040028, 0x08a0008400420001,
    0x0400210040801100, 0x0281002098400100, 0x0000420010248200, 0x0000300108008180,
    0x0000080100455100, 0x020a001008040200, 0x8000525008210400, 0x0040040849008a00,
    0x0000210080004011, 0x0480400080210015, 0x28000a0010204082, 0x1202001040040822,
    0x00a1001028000423, 0x6101000802040001, 0x400a9001020800e4, 0x0590010c00408426,
];

#[rustfmt::skip]
const BISHOP_MAGICS: [u64; 64] = [
    0x0082080101020202, 0x1a04010849010080, 0x0022022400210041, 0x1018248100100560,
    0x040c504080200800, 0x0040821140600880, 0x4102221004048040, 0x0202a08808081240,
    0x0108610202480108, 0x04840208480d1040, 0x0106304082a1000c, 0x8000044400810020,
    0x0080942420001000, 0x018402020222001a, 0xa315005144104008, 0x000016520c842004,
    0x0204401020023412, 0xc608004288082084, 0x1402000404140008, 0x200604202200c270,
    0x0004004200940202, 0x0002000700825100, 0x4090a18200900802, 0x1094208104021a01,
    0x0284400086100402, 0x0288022020020209, 0x20803000420400c0, 0x01810800240a0020,
    0x2801001001004000, 0x0008048008088400, 0x6101041002008400, 0x090c848431042090,
    0x0488840404106010, 0x2001080900202110, 0x50c0840400c04840, 0x0400080800220a01,
    0x4404008208040100, 0x2008020020241010, 0x40c4184088020080, 0x404600a301182c02,
    0x010482201100e104, 0x2040421004001006, 0x0200140028041401, 0x1020401148000400,
    0x0008700200980200, 0x8001811000805900, 0x804801080200408a, 0x1024810405000820,
    0x01004704b0400000, 0x008100880402d808, 0x000200310808000c, 0x0001020820a80008,
    0x02028022042c0000, 0x0001041092021100, 0x0809505012085240, 0x80a0840082084024,
    0x0084150401044000, 0x20250084040104c0, 0x2040000104881101, 0x0874809000420200,
    0x1000000091020200, 0x2104001010010844, 0xe90004a004042082, 0x0020021003070410,
];

const ROOK_MASKS: [u64; 64] = masks_for(true);
const BISHOP_MASKS: [u64; 64] = masks_for(false);
const ROOK_SHIFTS: [u32; 64] = shifts_for(&ROOK_MASKS);
const BISHOP_SHIFTS: [u32; 64] = shifts_for(&BISHOP_MASKS);

const ROOK_LAYOUT: ([usize; 64], usize) = layout(&ROOK_MASKS);
const BISHOP_LAYOUT: ([usize; 64], usize) = layout(&BISHOP_MASKS);
const ROOK_OFFSETS: [usize; 64] = ROOK_LAYOUT.0;
const BISHOP_OFFSETS: [usize; 64] = BISHOP_LAYOUT.0;
const ROOK_SIZE: usize = ROOK_LAYOUT.1;
const BISHOP_SIZE: usize = BISHOP_LAYOUT.1;

static ROOK_TABLE: [u64; ROOK_SIZE] = build_table::<ROOK_SIZE>(
    &ROOK_MASKS,
    &ROOK_MAGICS,
    &ROOK_OFFSETS,
    &ROOK_SHIFTS,
    &ROOK_DIRS,
);
static BISHOP_TABLE: [u64; BISHOP_SIZE] = build_table::<BISHOP_SIZE>(
    &BISHOP_MASKS,
    &BISHOP_MAGICS,
    &BISHOP_OFFSETS,
    &BISHOP_SHIFTS,
    &BISHOP_DIRS,
);

/// Rook attacks from `sq` under occupancy `occ`, via a single magic lookup into
/// the static `.rodata` table.
#[inline]
pub fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let i = sq.index();
    // SAFETY: `i < 64` (Square invariant). The magic for square `i` maps every
    // masked occupancy into `[OFFSETS[i], OFFSETS[i] + 2^bits)`, all of which is
    // within `ROOK_TABLE` by construction of the layout.
    unsafe {
        let idx = *ROOK_OFFSETS.get_unchecked(i)
            + (((occ.0 & ROOK_MASKS.get_unchecked(i)).wrapping_mul(*ROOK_MAGICS.get_unchecked(i)))
                >> ROOK_SHIFTS.get_unchecked(i)) as usize;
        Bitboard(*ROOK_TABLE.get_unchecked(idx))
    }
}

/// Bishop attacks from `sq` under occupancy `occ` (see [`rook_attacks`]).
#[inline]
pub fn bishop_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    let i = sq.index();
    // SAFETY: see `rook_attacks`.
    unsafe {
        let idx = *BISHOP_OFFSETS.get_unchecked(i)
            + (((occ.0 & BISHOP_MASKS.get_unchecked(i)).wrapping_mul(*BISHOP_MAGICS.get_unchecked(i)))
                >> BISHOP_SHIFTS.get_unchecked(i)) as usize;
        Bitboard(*BISHOP_TABLE.get_unchecked(idx))
    }
}

/// No-op: tables are compile-time constants. Kept for API compatibility with
/// the previous lazily-initialized implementation.
#[inline]
pub fn init() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attacks::{bishop_attacks_classical, rook_attacks_classical};

    /// The const tables must equal the classical ray scan for every square and
    /// every relevant-occupancy subset (exhaustive).
    #[test]
    fn const_tables_match_classical() {
        for sq in 0..64u8 {
            let square = Square(sq);
            // Rook
            let mut subset = 0u64;
            let rmask = ROOK_MASKS[sq as usize];
            loop {
                let got = rook_attacks(square, Bitboard(subset));
                let want = rook_attacks_classical(square, Bitboard(subset));
                assert_eq!(got, want, "rook sq={sq} occ={subset:#x}");
                subset = subset.wrapping_sub(rmask) & rmask;
                if subset == 0 {
                    break;
                }
            }
            // Bishop
            let mut subset = 0u64;
            let bmask = BISHOP_MASKS[sq as usize];
            loop {
                let got = bishop_attacks(square, Bitboard(subset));
                let want = bishop_attacks_classical(square, Bitboard(subset));
                assert_eq!(got, want, "bishop sq={sq} occ={subset:#x}");
                subset = subset.wrapping_sub(bmask) & bmask;
                if subset == 0 {
                    break;
                }
            }
        }
    }
}
