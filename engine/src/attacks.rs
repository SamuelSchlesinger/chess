//! Attack generation: precomputed leaper tables (knight, king, pawn) and
//! classical ray-scan sliding attacks (bishop, rook, queen), plus the
//! `between` / `line` geometry tables used for pins and check evasion.
//!
//! All tables are built at compile time via `const fn`, so there is no runtime
//! initialization cost and no need for `OnceLock`/lazy statics. The sliding
//! interface (`bishop_attacks` / `rook_attacks`) is deliberately blocker-keyed
//! so it can later be swapped for magic bitboards without touching callers.

use crate::bitboard::Bitboard;
use crate::types::{Color, PieceType, Square};

// Direction step vectors as (file_delta, rank_delta), indexed by direction.
// 0=N 1=S 2=E 3=W 4=NE 5=NW 6=SE 7=SW
const DIR_DF: [i32; 8] = [0, 0, 1, -1, 1, -1, 1, -1];
const DIR_DR: [i32; 8] = [1, -1, 0, 0, 1, 1, -1, -1];
/// Whether a direction moves toward higher square indices (forward bitscan).
const DIR_POSITIVE: [bool; 8] = [true, false, true, false, true, true, false, false];
/// Opposite-direction index for each direction.
const DIR_OPP: [usize; 8] = [1, 0, 3, 2, 7, 6, 5, 4];

const BISHOP_DIRS: [usize; 4] = [4, 5, 6, 7];
const ROOK_DIRS: [usize; 4] = [0, 1, 2, 3];

#[inline]
const fn on_board(f: i32, r: i32) -> bool {
    f >= 0 && f < 8 && r >= 0 && r < 8
}

const fn knight_from(sq: u8) -> u64 {
    let f = (sq % 8) as i32;
    let r = (sq / 8) as i32;
    let df = [1, 2, 2, 1, -1, -2, -2, -1];
    let dr = [2, 1, -1, -2, -2, -1, 1, 2];
    let mut bb = 0u64;
    let mut i = 0;
    while i < 8 {
        let nf = f + df[i];
        let nr = r + dr[i];
        if on_board(nf, nr) {
            bb |= 1u64 << (nr * 8 + nf);
        }
        i += 1;
    }
    bb
}

const fn king_from(sq: u8) -> u64 {
    let f = (sq % 8) as i32;
    let r = (sq / 8) as i32;
    let df = [0, 0, 1, -1, 1, -1, 1, -1];
    let dr = [1, -1, 0, 0, 1, 1, -1, -1];
    let mut bb = 0u64;
    let mut i = 0;
    while i < 8 {
        let nf = f + df[i];
        let nr = r + dr[i];
        if on_board(nf, nr) {
            bb |= 1u64 << (nr * 8 + nf);
        }
        i += 1;
    }
    bb
}

const fn pawn_from(color: usize, sq: u8) -> u64 {
    let f = (sq % 8) as i32;
    let r = (sq / 8) as i32;
    // White advances toward rank 8 (+1), Black toward rank 1 (-1).
    let dr = if color == 0 { 1 } else { -1 };
    let mut bb = 0u64;
    let mut k = 0;
    let dfs = [-1, 1];
    while k < 2 {
        let nf = f + dfs[k];
        let nr = r + dr;
        if on_board(nf, nr) {
            bb |= 1u64 << (nr * 8 + nf);
        }
        k += 1;
    }
    bb
}

const fn ray_from(sq: u8, dir: usize) -> u64 {
    let mut f = (sq % 8) as i32;
    let mut r = (sq / 8) as i32;
    let df = DIR_DF[dir];
    let dr = DIR_DR[dir];
    let mut bb = 0u64;
    loop {
        f += df;
        r += dr;
        if !on_board(f, r) {
            break;
        }
        bb |= 1u64 << (r * 8 + f);
    }
    bb
}

const KNIGHT_ATTACKS: [Bitboard; 64] = {
    let mut t = [Bitboard(0); 64];
    let mut sq = 0;
    while sq < 64 {
        t[sq] = Bitboard(knight_from(sq as u8));
        sq += 1;
    }
    t
};

const KING_ATTACKS: [Bitboard; 64] = {
    let mut t = [Bitboard(0); 64];
    let mut sq = 0;
    while sq < 64 {
        t[sq] = Bitboard(king_from(sq as u8));
        sq += 1;
    }
    t
};

const PAWN_ATTACKS: [[Bitboard; 64]; 2] = {
    let mut t = [[Bitboard(0); 64]; 2];
    let mut c = 0;
    while c < 2 {
        let mut sq = 0;
        while sq < 64 {
            t[c][sq] = Bitboard(pawn_from(c, sq as u8));
            sq += 1;
        }
        c += 1;
    }
    t
};

const RAYS: [[Bitboard; 64]; 8] = {
    let mut t = [[Bitboard(0); 64]; 8];
    let mut dir = 0;
    while dir < 8 {
        let mut sq = 0;
        while sq < 64 {
            t[dir][sq] = Bitboard(ray_from(sq as u8, dir));
            sq += 1;
        }
        dir += 1;
    }
    t
};

/// `BETWEEN[a][b]` = squares strictly between `a` and `b` when they are aligned
/// on a rank, file, or diagonal; empty otherwise. `static` (not `const`) so the
/// 32 KiB table exists once rather than being inlined at each use site.
static BETWEEN: [[Bitboard; 64]; 64] = {
    let mut t = [[Bitboard(0); 64]; 64];
    let mut a = 0;
    while a < 64 {
        let mut dir = 0;
        while dir < 8 {
            let ray = RAYS[dir][a].0;
            // Walk the ray; for each square b on it, between = squares from a up
            // to (not including) b = ray_from_a AND ray_from_b_in_opposite_dir.
            let mut bb = ray;
            while bb != 0 {
                let b = bb.trailing_zeros() as usize;
                t[a][b] = Bitboard(RAYS[dir][a].0 & RAYS[DIR_OPP[dir]][b].0);
                bb &= bb - 1;
            }
            dir += 1;
        }
        a += 1;
    }
    t
};

/// `LINE[a][b]` = every square on the infinite line through `a` and `b` when
/// they are aligned; empty otherwise. Used to keep pinned pieces on their pin
/// ray. `static` for the same reason as [`BETWEEN`].
static LINE: [[Bitboard; 64]; 64] = {
    let mut t = [[Bitboard(0); 64]; 64];
    let mut a = 0;
    while a < 64 {
        let mut dir = 0;
        while dir < 8 {
            let full = RAYS[dir][a].0 | RAYS[DIR_OPP[dir]][a].0 | (1u64 << a);
            let mut bb = RAYS[dir][a].0;
            while bb != 0 {
                let b = bb.trailing_zeros() as usize;
                t[a][b] = Bitboard(full);
                bb &= bb - 1;
            }
            dir += 1;
        }
        a += 1;
    }
    t
};

#[inline]
pub fn knight_attacks(sq: Square) -> Bitboard {
    KNIGHT_ATTACKS[sq.index()]
}

#[inline]
pub fn king_attacks(sq: Square) -> Bitboard {
    KING_ATTACKS[sq.index()]
}

#[inline]
pub fn pawn_attacks(color: Color, sq: Square) -> Bitboard {
    PAWN_ATTACKS[color.index()][sq.index()]
}

/// Squares strictly between `a` and `b` (empty if not aligned).
#[inline]
pub fn between(a: Square, b: Square) -> Bitboard {
    BETWEEN[a.index()][b.index()]
}

/// The full line through `a` and `b` (empty if not aligned).
#[inline]
pub fn line(a: Square, b: Square) -> Bitboard {
    LINE[a.index()][b.index()]
}

/// Whether `a`, `b`, `c` are colinear (on one rank, file, or diagonal).
#[inline]
pub fn aligned(a: Square, b: Square, c: Square) -> bool {
    LINE[a.index()][b.index()].has(c)
}

#[inline]
fn ray_attacks(dir: usize, sq: Square, occ: Bitboard) -> Bitboard {
    let attacks = RAYS[dir][sq.index()];
    let blockers = attacks & occ;
    if blockers.is_empty() {
        return attacks;
    }
    let blocker = if DIR_POSITIVE[dir] {
        blockers.lsb_unchecked()
    } else {
        // reverse bitscan: highest set bit
        Square((63 - blockers.0.leading_zeros()) as u8)
    };
    Bitboard(attacks.0 ^ RAYS[dir][blocker.index()].0)
}

/// Bishop attacks via classical ray scan. Used to bootstrap the magic tables
/// and kept public for head-to-head benchmarking against the magic backend.
#[inline]
pub fn bishop_attacks_classical(sq: Square, occ: Bitboard) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    let mut i = 0;
    while i < 4 {
        bb |= ray_attacks(BISHOP_DIRS[i], sq, occ);
        i += 1;
    }
    bb
}

/// Rook attacks via classical ray scan (see [`bishop_attacks_classical`]).
#[inline]
pub fn rook_attacks_classical(sq: Square, occ: Bitboard) -> Bitboard {
    let mut bb = Bitboard::EMPTY;
    let mut i = 0;
    while i < 4 {
        bb |= ray_attacks(ROOK_DIRS[i], sq, occ);
        i += 1;
    }
    bb
}

/// Bishop attacks from `sq` given the full occupancy `occ`.
///
/// Backed by magic bitboards (a single multiply-shift table lookup); falls back
/// to the classical scan only while the tables are being initialized.
#[inline]
pub fn bishop_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    crate::magic::bishop_attacks(sq, occ)
}

/// Rook attacks from `sq` given the full occupancy `occ` (magic bitboards).
#[inline]
pub fn rook_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    crate::magic::rook_attacks(sq, occ)
}

/// Queen attacks = bishop ∪ rook.
#[inline]
pub fn queen_attacks(sq: Square, occ: Bitboard) -> Bitboard {
    bishop_attacks(sq, occ) | rook_attacks(sq, occ)
}

/// Attacks of an arbitrary piece type (pawns require a color).
#[inline]
pub fn piece_attacks(pt: PieceType, color: Color, sq: Square, occ: Bitboard) -> Bitboard {
    match pt {
        PieceType::Pawn => pawn_attacks(color, sq),
        PieceType::Knight => knight_attacks(sq),
        PieceType::Bishop => bishop_attacks(sq, occ),
        PieceType::Rook => rook_attacks(sq, occ),
        PieceType::Queen => queen_attacks(sq, occ),
        PieceType::King => king_attacks(sq),
    }
}
