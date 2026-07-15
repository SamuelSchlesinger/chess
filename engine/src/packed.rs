//! The compact 34-byte canonical position form.
//!
//! Layout:
//! ```text
//!  bytes 0..32  nibble board: square s -> byte s/2, low nibble if s even.
//!               nibble 0 = empty, else (color<<3)|(piece_type+1)
//!               (White 1..=6, Black 9..=14) — same code as the mailbox.
//!  bytes 32..34 little-endian u16 state:
//!               bit  0      side to move (0 = White, 1 = Black)
//!               bits 1..5   castling rights (WK,WQ,BK,BQ)
//!               bits 5..9   en passant: 0 = none, else file+1 (1=a .. 8=h)
//!               bits 9..16  half-move clock, clamped to 127
//! ```
//!
//! The full-move number is intentionally dropped and the half-move clock is
//! clamped to 7 bits — neither affects legality, outcome, or the Zobrist key.
//! Random access (`piece_at`, `side_to_move`, …) works without unpacking.

use crate::board::{Board, decode_piece, encode_piece};
use crate::types::{CastlingRights, Color, File, Piece, Square};

/// A position packed into 34 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Packed {
    pub bytes: [u8; 34],
}

impl Packed {
    /// Read the nibble code for a square (`0` = empty).
    #[inline]
    fn nibble(&self, sq: Square) -> u8 {
        let byte = self.bytes[sq.index() >> 1];
        if sq.0 & 1 == 0 {
            byte & 0x0F
        } else {
            byte >> 4
        }
    }

    #[inline]
    fn set_nibble(&mut self, sq: Square, code: u8) {
        let idx = sq.index() >> 1;
        if sq.0 & 1 == 0 {
            self.bytes[idx] = (self.bytes[idx] & 0xF0) | (code & 0x0F);
        } else {
            self.bytes[idx] = (self.bytes[idx] & 0x0F) | (code << 4);
        }
    }

    #[inline]
    fn state(&self) -> u16 {
        u16::from_le_bytes([self.bytes[32], self.bytes[33]])
    }

    #[inline]
    fn set_state(&mut self, state: u16) {
        let [a, b] = state.to_le_bytes();
        self.bytes[32] = a;
        self.bytes[33] = b;
    }

    /// The piece on `sq`, if any — without unpacking the whole board.
    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        decode_piece(self.nibble(sq))
    }

    #[inline]
    pub fn side_to_move(&self) -> Color {
        if self.state() & 1 == 0 {
            Color::White
        } else {
            Color::Black
        }
    }

    #[inline]
    pub fn castling_rights(&self) -> CastlingRights {
        CastlingRights(((self.state() >> 1) & 0x0F) as u8)
    }

    #[inline]
    pub fn en_passant_square(&self) -> Option<Square> {
        let code = (self.state() >> 5) & 0x0F;
        if code == 0 {
            None
        } else {
            // Reconstruct the rank from side to move: the ep target sits on the
            // rank "behind" the pawn that just moved.
            let file = File((code - 1) as u8);
            let rank = match self.side_to_move() {
                Color::White => crate::types::Rank(5), // Black just pushed -> rank 6
                Color::Black => crate::types::Rank(2), // White just pushed -> rank 3
            };
            Some(Square::make(file, rank))
        }
    }

    #[inline]
    pub fn halfmove_clock(&self) -> u16 {
        (self.state() >> 9) & 0x7F
    }

    /// Reconstruct a full working [`Board`].
    pub fn unpack(&self) -> Board {
        let mut board = Board::empty();
        for i in 0..64u8 {
            let sq = Square(i);
            if let Some(p) = self.piece_at(sq) {
                board.set_square(sq, p);
            }
        }
        board.set_state(
            self.side_to_move(),
            self.castling_rights(),
            self.en_passant_square(),
            self.halfmove_clock(),
            1,
        );
        board.finalize_hash();
        board
    }
}

impl Board {
    /// Pack this position into its 34-byte canonical form.
    pub fn pack(&self) -> Packed {
        let mut p = Packed { bytes: [0; 34] };
        for i in 0..64u8 {
            let sq = Square(i);
            if let Some(piece) = self.piece_at(sq) {
                p.set_nibble(sq, encode_piece(piece));
            }
        }
        let mut state = (self.side_to_move() as u16) & 1;
        state |= (self.castling_rights().0 as u16 & 0x0F) << 1;
        let ep_code = match self.en_passant_square() {
            Some(sq) => (sq.file().0 as u16) + 1,
            None => 0,
        };
        state |= (ep_code & 0x0F) << 5;
        let hm = self.halfmove_clock().min(127);
        state |= (hm & 0x7F) << 9;
        p.set_state(state);
        p
    }
}

impl core::fmt::Debug for Packed {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Packed({})", self.unpack().to_fen())
    }
}
