//! Forsyth-Edwards Notation: [`Board::from_fen`] and [`Board::to_fen`].
//!
//! The piece-placement, side, castling, and en-passant fields are required; the
//! half-move clock and full-move number default to `0` and `1` when omitted
//! (so 4-field "EPD-style" position strings parse too).

use crate::board::Board;
use crate::types::{CastlingRights, Color, File, Piece, Rank, Square};
use core::fmt;

/// An error encountered while parsing a FEN string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenError {
    WrongFieldCount(usize),
    BadRankCount(usize),
    BadPlacement(String),
    BadSide(String),
    BadCastling(String),
    BadEnPassant(String),
    BadNumber(String),
    MissingKing(Color),
}

impl fmt::Display for FenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FenError::WrongFieldCount(n) => write!(f, "FEN must have 4 or 6 fields, got {n}"),
            FenError::BadRankCount(n) => write!(f, "FEN placement must have 8 ranks, got {n}"),
            FenError::BadPlacement(s) => write!(f, "invalid FEN placement: {s}"),
            FenError::BadSide(s) => write!(f, "invalid side to move: {s}"),
            FenError::BadCastling(s) => write!(f, "invalid castling field: {s}"),
            FenError::BadEnPassant(s) => write!(f, "invalid en-passant square: {s}"),
            FenError::BadNumber(s) => write!(f, "invalid number field: {s}"),
            FenError::MissingKing(c) => write!(f, "position is missing the {c:?} king"),
        }
    }
}

impl std::error::Error for FenError {}

impl Board {
    /// Parse a position from FEN (or 4-field EPD position).
    pub fn from_fen(fen: &str) -> Result<Board, FenError> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() != 6 && fields.len() != 4 {
            return Err(FenError::WrongFieldCount(fields.len()));
        }

        let mut board = Board::empty();

        // Field 1: piece placement, rank 8 first.
        let rows: Vec<&str> = fields[0].split('/').collect();
        if rows.len() != 8 {
            return Err(FenError::BadRankCount(rows.len()));
        }
        for (i, row) in rows.iter().enumerate() {
            let rank = 7 - i as u8;
            let mut file = 0u8;
            for ch in row.chars() {
                if let Some(d) = ch.to_digit(10) {
                    file += d as u8;
                    if file > 8 {
                        return Err(FenError::BadPlacement(fields[0].to_string()));
                    }
                } else {
                    let piece =
                        Piece::from_char(ch).ok_or_else(|| FenError::BadPlacement(fields[0].to_string()))?;
                    if file >= 8 {
                        return Err(FenError::BadPlacement(fields[0].to_string()));
                    }
                    board.set_square(Square::make(File(file), Rank(rank)), piece);
                    file += 1;
                }
            }
            if file != 8 {
                return Err(FenError::BadPlacement(fields[0].to_string()));
            }
        }

        // Field 2: side to move.
        board.side_to_move = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            other => return Err(FenError::BadSide(other.to_string())),
        };

        // Field 3: castling availability.
        let mut rights = CastlingRights::NONE;
        if fields[2] != "-" {
            for ch in fields[2].chars() {
                rights.0 |= match ch {
                    'K' => CastlingRights::WHITE_KING,
                    'Q' => CastlingRights::WHITE_QUEEN,
                    'k' => CastlingRights::BLACK_KING,
                    'q' => CastlingRights::BLACK_QUEEN,
                    _ => return Err(FenError::BadCastling(fields[2].to_string())),
                };
            }
        }
        board.castling = rights;

        // Field 4: en-passant target.
        board.ep_square = if fields[3] == "-" {
            None
        } else {
            Some(Square::from_algebraic(fields[3]).ok_or_else(|| {
                FenError::BadEnPassant(fields[3].to_string())
            })?)
        };

        // Fields 5 & 6: clocks (optional).
        if fields.len() == 6 {
            board.halfmove_clock = fields[4]
                .parse()
                .map_err(|_| FenError::BadNumber(fields[4].to_string()))?;
            board.fullmove_number = fields[5]
                .parse()
                .map_err(|_| FenError::BadNumber(fields[5].to_string()))?;
        }

        // Validate kings exist (required for legal move generation).
        for color in Color::ALL {
            if board.pieces_colored(crate::types::PieceType::King, color).is_empty() {
                return Err(FenError::MissingKing(color));
            }
        }

        board.finalize_hash();
        Ok(board)
    }

    /// Serialize the position to a 6-field FEN string.
    pub fn to_fen(&self) -> String {
        let mut s = String::with_capacity(80);
        for rank in (0..8).rev() {
            let mut empty = 0u8;
            for file in 0..8 {
                let sq = Square::make(File(file), Rank(rank));
                match self.piece_at(sq) {
                    Some(p) => {
                        if empty > 0 {
                            s.push((b'0' + empty) as char);
                            empty = 0;
                        }
                        s.push(p.to_char());
                    }
                    None => empty += 1,
                }
            }
            if empty > 0 {
                s.push((b'0' + empty) as char);
            }
            if rank > 0 {
                s.push('/');
            }
        }

        s.push(' ');
        s.push(match self.side_to_move {
            Color::White => 'w',
            Color::Black => 'b',
        });

        s.push(' ');
        if self.castling.is_empty() {
            s.push('-');
        } else {
            if self.castling.0 & CastlingRights::WHITE_KING != 0 {
                s.push('K');
            }
            if self.castling.0 & CastlingRights::WHITE_QUEEN != 0 {
                s.push('Q');
            }
            if self.castling.0 & CastlingRights::BLACK_KING != 0 {
                s.push('k');
            }
            if self.castling.0 & CastlingRights::BLACK_QUEEN != 0 {
                s.push('q');
            }
        }

        s.push(' ');
        match self.ep_square {
            Some(sq) => s.push_str(&sq.to_string()),
            None => s.push('-'),
        }

        s.push(' ');
        s.push_str(&self.halfmove_clock.to_string());
        s.push(' ');
        s.push_str(&self.fullmove_number.to_string());
        s
    }
}
