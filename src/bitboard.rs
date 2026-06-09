//! [`Bitboard`]: a 64-bit set of squares, the workhorse of move generation.
//!
//! Bit `i` corresponds to [`Square`]`(i)` in LERF order (`a1 = bit 0`). All
//! directional shifts pre-mask the wrap-around file so bits never leak across
//! the board edge.

use crate::types::{File, Rank, Square};
use core::fmt;
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not, Shl, Shr};

/// A set of squares represented as a 64-bit mask.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Bitboard(pub u64);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(!0);

    pub const FILE_A: Bitboard = Bitboard(0x0101_0101_0101_0101);
    pub const FILE_B: Bitboard = Bitboard(0x0202_0202_0202_0202);
    pub const FILE_G: Bitboard = Bitboard(0x4040_4040_4040_4040);
    pub const FILE_H: Bitboard = Bitboard(0x8080_8080_8080_8080);
    pub const RANK_1: Bitboard = Bitboard(0x0000_0000_0000_00FF);
    pub const RANK_2: Bitboard = Bitboard(0x0000_0000_0000_FF00);
    pub const RANK_4: Bitboard = Bitboard(0x0000_0000_FF00_0000);
    pub const RANK_5: Bitboard = Bitboard(0x0000_00FF_0000_0000);
    pub const RANK_7: Bitboard = Bitboard(0x00FF_0000_0000_0000);
    pub const RANK_8: Bitboard = Bitboard(0xFF00_0000_0000_0000);

    /// A bitboard with only `square` set.
    #[inline]
    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1u64 << square.0)
    }

    /// The mask for an entire file.
    #[inline]
    pub const fn file(file: File) -> Bitboard {
        Bitboard(Self::FILE_A.0 << file.0)
    }

    /// The mask for an entire rank.
    #[inline]
    pub const fn rank(rank: Rank) -> Bitboard {
        Bitboard(Self::RANK_1.0 << (rank.0 * 8))
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn any(self) -> bool {
        self.0 != 0
    }

    /// Number of squares in the set.
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether `square` is in the set.
    #[inline]
    pub const fn has(self, square: Square) -> bool {
        self.0 & (1u64 << square.0) != 0
    }

    /// Whether the two sets share any square.
    #[inline]
    pub const fn intersects(self, other: Bitboard) -> bool {
        self.0 & other.0 != 0
    }

    #[inline]
    pub fn set(&mut self, square: Square) {
        self.0 |= 1u64 << square.0;
    }

    #[inline]
    pub fn clear(&mut self, square: Square) {
        self.0 &= !(1u64 << square.0);
    }

    #[inline]
    pub fn toggle(&mut self, square: Square) {
        self.0 ^= 1u64 << square.0;
    }

    /// The least-significant set square, or `None` if empty.
    #[inline]
    pub const fn lsb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square(self.0.trailing_zeros() as u8))
        }
    }

    /// The least-significant set square, assuming the set is non-empty.
    #[inline]
    pub const fn lsb_unchecked(self) -> Square {
        Square(self.0.trailing_zeros() as u8)
    }

    /// Remove and return the least-significant set square.
    #[inline]
    pub fn pop_lsb(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        let sq = Square(self.0.trailing_zeros() as u8);
        self.0 &= self.0 - 1;
        Some(sq)
    }

    /// Whether exactly one square is set.
    #[inline]
    pub const fn is_single(self) -> bool {
        self.0 != 0 && (self.0 & (self.0.wrapping_sub(1))) == 0
    }

    // --- directional shifts (with edge masking) ---

    #[inline]
    pub const fn north(self) -> Bitboard {
        Bitboard(self.0 << 8)
    }
    #[inline]
    pub const fn south(self) -> Bitboard {
        Bitboard(self.0 >> 8)
    }
    #[inline]
    pub const fn east(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_H.0) << 1)
    }
    #[inline]
    pub const fn west(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_A.0) >> 1)
    }
    #[inline]
    pub const fn north_east(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_H.0) << 9)
    }
    #[inline]
    pub const fn north_west(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_A.0) << 7)
    }
    #[inline]
    pub const fn south_east(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_H.0) >> 7)
    }
    #[inline]
    pub const fn south_west(self) -> Bitboard {
        Bitboard((self.0 & !Self::FILE_A.0) >> 9)
    }
}

impl Iterator for Bitboard {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        self.pop_lsb()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.count() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Bitboard {}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}
impl BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}
impl BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}
impl Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}
impl BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}
impl BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}
impl BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}
impl Shl<u32> for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn shl(self, rhs: u32) -> Bitboard {
        Bitboard(self.0 << rhs)
    }
}
impl Shr<u32> for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn shr(self, rhs: u32) -> Bitboard {
        Bitboard(self.0 >> rhs)
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bitboard(0x{:016x})", self.0)
    }
}

impl fmt::Display for Bitboard {
    /// Render as an 8x8 grid, rank 8 at the top, `.`/`X` per square.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            for file in 0..8 {
                let sq = Square(rank * 8 + file);
                f.write_str(if self.has(sq) { "X " } else { ". " })?;
            }
            f.write_str("\n")?;
        }
        Ok(())
    }
}
