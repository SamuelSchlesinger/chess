//! A 16-bit packed [`Move`] and a stack-allocated [`MoveList`].
//!
//! Encoding (Chess Programming Wiki "Encoding Moves"):
//! ```text
//!  bits  0..6   from square (0..64)
//!  bits  6..12  to square   (0..64)
//!  bits 12..16  flags:
//!    0000 quiet            0001 double pawn push
//!    0010 king castle      0011 queen castle
//!    0100 capture          0101 en-passant capture
//!    1000 N-promo  1001 B-promo  1010 R-promo  1011 Q-promo
//!    1100 N-promo-capture ... 1111 Q-promo-capture
//! ```
//! Bit `0b1000` marks a promotion, bit `0b0100` marks a capture, and for
//! promotions the low two bits select the piece (`0=N,1=B,2=R,3=Q`).

use crate::types::{PieceType, Square};
use core::fmt;

/// Move flag nibble.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MoveFlag {
    Quiet = 0b0000,
    DoublePawnPush = 0b0001,
    KingCastle = 0b0010,
    QueenCastle = 0b0011,
    Capture = 0b0100,
    EnPassant = 0b0101,
    KnightPromo = 0b1000,
    BishopPromo = 0b1001,
    RookPromo = 0b1010,
    QueenPromo = 0b1011,
    KnightPromoCapture = 0b1100,
    BishopPromoCapture = 0b1101,
    RookPromoCapture = 0b1110,
    QueenPromoCapture = 0b1111,
}

const PROMO_FLAG: u16 = 0b1000;
const CAPTURE_FLAG: u16 = 0b0100;

/// A chess move packed into 16 bits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move(pub u16);

impl Move {
    /// Sentinel "no move" (encodes a1->a1 quiet, which is never a legal move).
    pub const NONE: Move = Move(0);

    #[inline]
    const fn encode(from: Square, to: Square, flag: u16) -> Move {
        Move((from.0 as u16) | ((to.0 as u16) << 6) | (flag << 12))
    }

    #[inline]
    pub const fn new(from: Square, to: Square, flag: MoveFlag) -> Move {
        Self::encode(from, to, flag as u16)
    }

    #[inline]
    pub const fn quiet(from: Square, to: Square) -> Move {
        Self::encode(from, to, MoveFlag::Quiet as u16)
    }

    #[inline]
    pub const fn capture(from: Square, to: Square) -> Move {
        Self::encode(from, to, MoveFlag::Capture as u16)
    }

    #[inline]
    pub const fn double_push(from: Square, to: Square) -> Move {
        Self::encode(from, to, MoveFlag::DoublePawnPush as u16)
    }

    #[inline]
    pub const fn en_passant(from: Square, to: Square) -> Move {
        Self::encode(from, to, MoveFlag::EnPassant as u16)
    }

    /// Promotion to `piece`; `capture` selects the promo-capture variant.
    #[inline]
    pub fn promotion(from: Square, to: Square, piece: PieceType, capture: bool) -> Move {
        let base = match piece {
            PieceType::Knight => 0b1000,
            PieceType::Bishop => 0b1001,
            PieceType::Rook => 0b1010,
            PieceType::Queen => 0b1011,
            // Pawns and kings cannot be promotion targets; default to queen.
            _ => 0b1011,
        };
        Self::encode(from, to, base | if capture { CAPTURE_FLAG } else { 0 })
    }

    #[inline]
    pub const fn from(self) -> Square {
        Square((self.0 & 0x3F) as u8)
    }

    #[inline]
    pub const fn to(self) -> Square {
        Square(((self.0 >> 6) & 0x3F) as u8)
    }

    #[inline]
    pub const fn flag_bits(self) -> u16 {
        self.0 >> 12
    }

    #[inline]
    pub const fn is_capture(self) -> bool {
        self.flag_bits() & CAPTURE_FLAG != 0
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        self.flag_bits() & PROMO_FLAG != 0
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        self.flag_bits() == MoveFlag::EnPassant as u16
    }

    #[inline]
    pub const fn is_double_push(self) -> bool {
        self.flag_bits() == MoveFlag::DoublePawnPush as u16
    }

    #[inline]
    pub const fn is_king_castle(self) -> bool {
        self.flag_bits() == MoveFlag::KingCastle as u16
    }

    #[inline]
    pub const fn is_queen_castle(self) -> bool {
        self.flag_bits() == MoveFlag::QueenCastle as u16
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        self.is_king_castle() || self.is_queen_castle()
    }

    /// The promoted-to piece type, if this is a promotion.
    #[inline]
    pub const fn promotion_piece(self) -> Option<PieceType> {
        if !self.is_promotion() {
            return None;
        }
        Some(match self.flag_bits() & 0b11 {
            0 => PieceType::Knight,
            1 => PieceType::Bishop,
            2 => PieceType::Rook,
            _ => PieceType::Queen,
        })
    }

    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from(), self.to())?;
        if let Some(p) = self.promotion_piece() {
            write!(f, "{}", p.to_char())?;
        }
        Ok(())
    }
}

impl fmt::Display for Move {
    /// UCI long algebraic notation (e.g. `e2e4`, `e7e8q`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.from(), self.to())?;
        if let Some(p) = self.promotion_piece() {
            write!(f, "{}", p.to_char())?;
        }
        Ok(())
    }
}

/// The maximum number of legal moves in any reachable chess position is 218;
/// 256 gives generous headroom and a power-of-two capacity.
const MAX_MOVES: usize = 256;

/// A fixed-capacity, stack-allocated list of moves — no heap allocation in the
/// move-generation hot path, and no per-list zero-initialization (the backing
/// array is `MaybeUninit`, so constructing an empty list is free).
pub struct MoveList {
    moves: [core::mem::MaybeUninit<Move>; MAX_MOVES],
    len: usize,
}

impl MoveList {
    #[inline]
    pub fn new() -> MoveList {
        MoveList {
            moves: [core::mem::MaybeUninit::uninit(); MAX_MOVES],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, mv: Move) {
        debug_assert!(self.len < MAX_MOVES, "move list overflow");
        // SAFETY: `len < MAX_MOVES` (218 is the true maximum; 256 is the cap).
        unsafe {
            self.moves.get_unchecked_mut(self.len).write(mv);
        }
        self.len += 1;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        // SAFETY: the first `len` entries have been initialized by `push`, and
        // `MaybeUninit<Move>` has the same layout as `Move`.
        unsafe { core::slice::from_raw_parts(self.moves.as_ptr() as *const Move, self.len) }
    }

    #[inline]
    pub fn contains(&self, mv: Move) -> bool {
        self.as_slice().contains(&mv)
    }

    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

impl core::ops::Index<usize> for MoveList {
    type Output = Move;
    #[inline]
    fn index(&self, i: usize) -> &Move {
        &self.as_slice()[i]
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = core::slice::Iter<'a, Move>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}
