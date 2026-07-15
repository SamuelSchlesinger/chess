//! Standard Algebraic Notation: [`Board::san`] (render) and
//! [`Board::parse_san`] (read).
//!
//! Rendering computes disambiguation and check/checkmate suffixes exactly.
//! Parsing is done by generating the SAN of every legal move and matching the
//! normalized input — robust against spelling variants (`0-0`, trailing
//! `+`/`#`/`!`/`?`, `e.p.`) and never accepts an illegal move.

use crate::board::Board;
use crate::moves::Move;
use crate::types::PieceType;

impl Board {
    /// Standard Algebraic Notation for a legal move in this position.
    pub fn san(&self, mv: Move) -> String {
        let mut s = self.san_no_suffix(mv);
        // Check / checkmate suffix.
        let mut after = self.clone();
        after.make_move(mv);
        if after.in_check() {
            s.push(if after.legal_moves().is_empty() { '#' } else { '+' });
        }
        s
    }

    fn san_no_suffix(&self, mv: Move) -> String {
        if mv.is_king_castle() {
            return "O-O".to_string();
        }
        if mv.is_queen_castle() {
            return "O-O-O".to_string();
        }

        let from = mv.from();
        let to = mv.to();
        let pt = self.piece_type_at(from).expect("piece on from-square");
        let mut s = String::new();

        if pt == PieceType::Pawn {
            if mv.is_capture() {
                s.push(from.file().to_char());
                s.push('x');
            }
            s.push_str(&to.to_string());
            if let Some(promo) = mv.promotion_piece() {
                s.push('=');
                s.push(promo.to_char().to_ascii_uppercase());
            }
            return s;
        }

        s.push(pt.to_char().to_ascii_uppercase());

        // Disambiguation: other same-type pieces that can also move to `to`.
        let mut same_file = false;
        let mut same_rank = false;
        let mut ambiguous = false;
        for &other in self.legal_moves().iter() {
            if other.to() == to
                && other.from() != from
                && self.piece_type_at(other.from()) == Some(pt)
            {
                ambiguous = true;
                if other.from().file() == from.file() {
                    same_file = true;
                }
                if other.from().rank() == from.rank() {
                    same_rank = true;
                }
            }
        }
        if ambiguous {
            if !same_file {
                s.push(from.file().to_char());
            } else if !same_rank {
                s.push(from.rank().to_char());
            } else {
                s.push(from.file().to_char());
                s.push(from.rank().to_char());
            }
        }

        if mv.is_capture() {
            s.push('x');
        }
        s.push_str(&to.to_string());
        s
    }

    /// Parse a SAN move in the context of this position.
    pub fn parse_san(&self, san: &str) -> Option<Move> {
        let want = normalize_san(san);
        let moves = self.legal_moves();
        moves
            .iter()
            .copied()
            .find(|&mv| normalize_san(&self.san_no_suffix(mv)) == want)
    }
}

/// Strip decorations so equivalent spellings compare equal.
fn normalize_san(san: &str) -> String {
    let mut s = san.trim().to_string();
    // Remove common annotations / e.p. marker.
    for pat in ["e.p.", "ep"] {
        if let Some(idx) = s.find(pat) {
            // Only strip a trailing "e.p."-style marker, not the 'e' file etc.
            if idx + pat.len() == s.len() && pat == "e.p." {
                s.truncate(idx);
            }
        }
    }
    // Normalize castling zeros to capital O.
    s = s.replace('0', "O");
    // Drop check / mate / annotation glyphs and any surrounding whitespace.
    s.trim_end_matches(['+', '#', '!', '?', ' '])
        .trim()
        .to_string()
}
