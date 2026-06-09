//! UCI long algebraic move notation (`e2e4`, `e1g1`, `e7e8q`).
//!
//! Parsing resolves the bare from/to/promotion against the legal move list, so
//! the returned [`Move`] always carries correct capture / en-passant / castle /
//! double-push flags.

use crate::board::Board;
use crate::moves::Move;
use crate::types::{PieceType, Square};

impl Move {
    /// Render in UCI notation. Castling is encoded as the king's two-square
    /// move (`e1g1`, `e8c8`), matching standard UCI.
    pub fn to_uci(self) -> String {
        self.to_string()
    }
}

impl Board {
    /// Parse a UCI move string in the context of this position, returning the
    /// matching legal move (or `None` if illegal / malformed).
    pub fn parse_uci(&self, s: &str) -> Option<Move> {
        let b = s.as_bytes();
        if b.len() < 4 {
            return None;
        }
        let from = Square::from_algebraic(&s[0..2])?;
        let to = Square::from_algebraic(&s[2..4])?;
        let promo = if b.len() >= 5 {
            Some(PieceType::from_char(b[4] as char)?)
        } else {
            None
        };
        let moves = self.legal_moves();
        moves
            .iter()
            .copied()
            .find(|&mv| mv.from() == from && mv.to() == to && mv.promotion_piece() == promo)
    }
}
