//! Polyglot-compatible Zobrist hashing.
//!
//! These keys are bit-for-bit compatible with the Polyglot opening-book format
//! (and python-chess `chess.polyglot.zobrist_hash`), so hashes can be checked
//! against published reference values. See [`crate::zobrist_table`] for the
//! constants and index layout.

use crate::types::{CastlingRights, Color, File, Piece};
use crate::zobrist_table::POLYGLOT_RANDOM;

const CASTLING_BASE: usize = 768;
const EP_BASE: usize = 772;
const TURN_INDEX: usize = 780;

/// Key contribution of `piece` standing on `square` (0..64, a1=0).
#[inline]
pub fn piece_key(piece: Piece, square: u8) -> u64 {
    // kind = 2*(piece_type as 0-based) + (white ? 1 : 0)
    let kind = 2 * piece.piece_type.index() + (piece.color == Color::White) as usize;
    POLYGLOT_RANDOM[64 * kind + square as usize]
}

/// Key for a single castling right by its canonical index
/// (0 = White O-O, 1 = White O-O-O, 2 = Black O-O, 3 = Black O-O-O).
#[inline]
pub fn castling_index_key(i: usize) -> u64 {
    POLYGLOT_RANDOM[CASTLING_BASE + i]
}

/// Combined key for a whole set of castling rights.
#[inline]
pub fn castling_key(rights: CastlingRights) -> u64 {
    let mut h = 0;
    if rights.0 & CastlingRights::WHITE_KING != 0 {
        h ^= POLYGLOT_RANDOM[CASTLING_BASE];
    }
    if rights.0 & CastlingRights::WHITE_QUEEN != 0 {
        h ^= POLYGLOT_RANDOM[CASTLING_BASE + 1];
    }
    if rights.0 & CastlingRights::BLACK_KING != 0 {
        h ^= POLYGLOT_RANDOM[CASTLING_BASE + 2];
    }
    if rights.0 & CastlingRights::BLACK_QUEEN != 0 {
        h ^= POLYGLOT_RANDOM[CASTLING_BASE + 3];
    }
    h
}

/// Key for an en-passant file. Per the Polyglot rule, the caller must only
/// apply this when a pawn can actually make the en-passant capture.
#[inline]
pub fn ep_file_key(file: File) -> u64 {
    POLYGLOT_RANDOM[EP_BASE + file.index()]
}

/// Key xored in when it is White's turn to move.
#[inline]
pub fn turn_key() -> u64 {
    POLYGLOT_RANDOM[TURN_INDEX]
}
