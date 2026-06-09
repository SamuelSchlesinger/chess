//! The default handcrafted evaluator: tapered PeSTO (material + piece-square
//! tables interpolated between midgame and endgame), plus the bishop pair and
//! pawn-structure terms (doubled, isolated, passed) and a tempo bonus.

use super::Evaluator;
use super::pesto_tables::{
    EG_MATERIAL, EG_PST, MG_MATERIAL, MG_PST, PHASE_WEIGHT, TOTAL_PHASE,
};
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::types::{Color, PieceType};

const BISHOP_PAIR: i32 = 30;
const DOUBLED: i32 = -10;
const ISOLATED: i32 = -14;
const TEMPO: i32 = 12;
/// Passed-pawn bonus indexed by the pawn's rank (white perspective; 0 = rank 1).
const PASSED: [i32; 8] = [0, 8, 12, 24, 45, 75, 120, 0];

#[inline]
const fn file_bb(f: usize) -> u64 {
    0x0101_0101_0101_0101u64 << f
}

const ADJACENT_FILES: [u64; 8] = {
    let mut t = [0u64; 8];
    let mut f = 0;
    while f < 8 {
        let mut m = 0u64;
        if f > 0 {
            m |= file_bb(f - 1);
        }
        if f < 7 {
            m |= file_bb(f + 1);
        }
        t[f] = m;
        f += 1;
    }
    t
};

/// Squares on the same and adjacent files strictly ahead of `sq` for `white` —
/// empty of enemy pawns ⇒ passed.
const fn passed_mask(white: bool, sq: usize) -> u64 {
    let f = sq % 8;
    let r = sq / 8;
    let files = file_bb(f) | ADJACENT_FILES[f];
    let mut ranks = 0u64;
    if white {
        let mut rr = r + 1;
        while rr < 8 {
            ranks |= 0xFFu64 << (rr * 8);
            rr += 1;
        }
    } else {
        let mut rr = 0;
        while rr < r {
            ranks |= 0xFFu64 << (rr * 8);
            rr += 1;
        }
    }
    files & ranks
}

const WHITE_PASSED: [u64; 64] = {
    let mut t = [0u64; 64];
    let mut s = 0;
    while s < 64 {
        t[s] = passed_mask(true, s);
        s += 1;
    }
    t
};
const BLACK_PASSED: [u64; 64] = {
    let mut t = [0u64; 64];
    let mut s = 0;
    while s < 64 {
        t[s] = passed_mask(false, s);
        s += 1;
    }
    t
};

/// Stateless PeSTO + structure evaluator.
#[derive(Clone, Copy, Default)]
pub struct HandcraftedEval;

impl HandcraftedEval {
    pub fn new() -> Self {
        HandcraftedEval
    }
}

impl Evaluator for HandcraftedEval {
    fn evaluate(&mut self, board: &Board) -> i32 {
        evaluate_white(board).perspective(board.side_to_move()) + TEMPO
    }
}

/// Compute the white-perspective score (positive = good for White).
fn evaluate_white(board: &Board) -> WhiteScore {
    let mut mg = 0i32;
    let mut eg = 0i32;
    let mut phase = 0i32;

    for pt in PieceType::ALL {
        let pti = pt.index();
        let mat_mg = MG_MATERIAL[pti];
        let mat_eg = EG_MATERIAL[pti];
        let pst_mg = &MG_PST[pti];
        let pst_eg = &EG_PST[pti];

        let mut white = board.pieces_colored(pt, Color::White);
        while let Some(sq) = white.pop_lsb() {
            let i = sq.index();
            mg += mat_mg + pst_mg[i];
            eg += mat_eg + pst_eg[i];
            phase += PHASE_WEIGHT[pti];
        }
        let mut black = board.pieces_colored(pt, Color::Black);
        while let Some(sq) = black.pop_lsb() {
            let i = sq.index() ^ 56; // mirror for black
            mg -= mat_mg + pst_mg[i];
            eg -= mat_eg + pst_eg[i];
            phase += PHASE_WEIGHT[pti];
        }
    }

    let mg_phase = phase.min(TOTAL_PHASE);
    let eg_phase = TOTAL_PHASE - mg_phase;
    let tapered = (mg * mg_phase + eg * eg_phase) / TOTAL_PHASE;

    WhiteScore(tapered + pawn_structure(board) + bishop_pair(board))
}

fn bishop_pair(board: &Board) -> i32 {
    let mut s = 0;
    if board.pieces_colored(PieceType::Bishop, Color::White).count() >= 2 {
        s += BISHOP_PAIR;
    }
    if board.pieces_colored(PieceType::Bishop, Color::Black).count() >= 2 {
        s -= BISHOP_PAIR;
    }
    s
}

fn pawn_structure(board: &Board) -> i32 {
    let wp = board.pieces_colored(PieceType::Pawn, Color::White);
    let bp = board.pieces_colored(PieceType::Pawn, Color::Black);
    let mut s = 0;

    let mut bb = wp;
    while let Some(sq) = bb.pop_lsb() {
        let f = sq.file().index();
        if (wp & Bitboard(file_bb(f))).count() > 1 {
            s += DOUBLED;
        }
        if (wp & Bitboard(ADJACENT_FILES[f])).is_empty() {
            s += ISOLATED;
        }
        if (bp & Bitboard(WHITE_PASSED[sq.index()])).is_empty() {
            s += PASSED[sq.rank().index()];
        }
    }

    let mut bb = bp;
    while let Some(sq) = bb.pop_lsb() {
        let f = sq.file().index();
        if (bp & Bitboard(file_bb(f))).count() > 1 {
            s -= DOUBLED;
        }
        if (bp & Bitboard(ADJACENT_FILES[f])).is_empty() {
            s -= ISOLATED;
        }
        if (wp & Bitboard(BLACK_PASSED[sq.index()])).is_empty() {
            s -= PASSED[7 - sq.rank().index()];
        }
    }
    s
}

/// A white-perspective score that can be flipped to the side to move.
struct WhiteScore(i32);
impl WhiteScore {
    #[inline]
    fn perspective(self, stm: Color) -> i32 {
        match stm {
            Color::White => self.0,
            Color::Black => -self.0,
        }
    }
}
