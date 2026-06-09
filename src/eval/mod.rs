//! Position evaluation.
//!
//! Evaluation is abstracted behind the [`Evaluator`] trait so the search can run
//! on the handcrafted PeSTO evaluator today and a trained NNUE network later
//! without changing the search — the engine is generic over `E: Evaluator`. The
//! trait also exposes optional incremental hooks ([`Evaluator::on_make`] /
//! [`Evaluator::on_unmake`]) that an NNUE accumulator would use; the handcrafted
//! evaluator ignores them (default no-ops), so there is no cost today.

pub mod handcrafted;
mod pesto_tables;

pub use handcrafted::HandcraftedEval;

use crate::board::Board;
use crate::moves::Move;

/// A value larger than any real evaluation — used as the alpha/beta bounds.
pub const INFINITY: i32 = 32_000;
/// Base score for checkmate; a mate `n` plies away scores `MATE - n`.
pub const MATE: i32 = 31_000;
/// Maximum search ply (depth of the in-tree path).
pub const MAX_PLY: usize = 128;
/// Scores at or beyond this magnitude denote a forced mate.
pub const MATE_IN_MAX: i32 = MATE - MAX_PLY as i32;
/// A drawn position.
pub const DRAW: i32 = 0;

/// Static evaluation of a position, in centipawns, from the **side-to-move's**
/// perspective (positive = the side to move is better). This is the negamax
/// convention the search relies on.
pub trait Evaluator {
    /// Evaluate `board` from the side-to-move's perspective.
    fn evaluate(&mut self, board: &Board) -> i32;

    /// Called by the search just after `board.make_move(mv)`. An incremental
    /// (e.g. NNUE-accumulator) evaluator updates its state here; the default is
    /// a no-op for from-scratch evaluators.
    #[inline]
    fn on_make(&mut self, board: &Board, mv: Move) {
        let _ = (board, mv);
    }

    /// Called by the search just after `board.unmake_move(mv, undo)`.
    #[inline]
    fn on_unmake(&mut self, board: &Board, mv: Move) {
        let _ = (board, mv);
    }

    /// Recompute any cached state from scratch for `board` (e.g. at the search
    /// root, or after a null move).
    #[inline]
    fn refresh(&mut self, board: &Board) {
        let _ = board;
    }
}

/// Whether `score` denotes a forced mate (for either side).
#[inline]
pub fn is_mate(score: i32) -> bool {
    score.abs() >= MATE_IN_MAX
}

/// For a mate score, the number of *moves* (not plies) until mate: positive when
/// the side to move is mating, negative when being mated. `None` if not a mate.
pub fn mate_in_moves(score: i32) -> Option<i32> {
    if !is_mate(score) {
        return None;
    }
    Some(if score > 0 {
        (MATE - score + 1) / 2
    } else {
        -((MATE + score) / 2 + 1)
    })
}
