//! Fundamental chess value types: [`Color`], [`PieceType`], [`Piece`],
//! [`File`], [`Rank`], [`Square`], and [`CastlingRights`].
//!
//! Square numbering is little-endian rank-file (LERF): `a1 = 0`, `b1 = 1`,
//! ..., `h1 = 7`, `a2 = 8`, ..., `h8 = 63`. This matches python-chess and the
//! Polyglot hashing convention, which our test oracles depend on.

use core::fmt;

/// Side to move / piece owner.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    /// All colors in canonical order.
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    /// The opposing color.
    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// Index `0` (White) or `1` (Black), for array lookups.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// `+1` for White, `-1` for Black — the direction pawns advance in ranks.
    #[inline]
    pub const fn forward(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }
}

/// The six kinds of piece. Discriminants `0..6` are used directly as array
/// indices; the Polyglot convention's `1..6` numbering is derived where needed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    /// All piece types in discriminant order.
    pub const ALL: [PieceType; 6] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    /// Promotion targets, in conventional preference order.
    pub const PROMOTIONS: [PieceType; 4] = [
        PieceType::Queen,
        PieceType::Rook,
        PieceType::Bishop,
        PieceType::Knight,
    ];

    /// Index `0..6` for array lookups.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Construct from a `0..6` discriminant, if in range.
    #[inline]
    pub const fn from_index(i: usize) -> Option<PieceType> {
        Some(match i {
            0 => PieceType::Pawn,
            1 => PieceType::Knight,
            2 => PieceType::Bishop,
            3 => PieceType::Rook,
            4 => PieceType::Queen,
            5 => PieceType::King,
            _ => return None,
        })
    }

    /// Lowercase piece letter as used in FEN/SAN (`p n b r q k`).
    #[inline]
    pub const fn to_char(self) -> char {
        match self {
            PieceType::Pawn => 'p',
            PieceType::Knight => 'n',
            PieceType::Bishop => 'b',
            PieceType::Rook => 'r',
            PieceType::Queen => 'q',
            PieceType::King => 'k',
        }
    }

    /// Parse a piece letter (case-insensitive).
    #[inline]
    pub const fn from_char(c: char) -> Option<PieceType> {
        Some(match c.to_ascii_lowercase() {
            'p' => PieceType::Pawn,
            'n' => PieceType::Knight,
            'b' => PieceType::Bishop,
            'r' => PieceType::Rook,
            'q' => PieceType::Queen,
            'k' => PieceType::King,
            _ => return None,
        })
    }

    /// Whether this piece slides along rays (bishop, rook, queen).
    #[inline]
    pub const fn is_slider(self) -> bool {
        matches!(self, PieceType::Bishop | PieceType::Rook | PieceType::Queen)
    }
}

/// A colored piece.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Piece {
    pub color: Color,
    pub piece_type: PieceType,
}

impl Piece {
    #[inline]
    pub const fn new(color: Color, piece_type: PieceType) -> Piece {
        Piece { color, piece_type }
    }

    /// FEN/SAN letter: uppercase for White, lowercase for Black.
    #[inline]
    pub const fn to_char(self) -> char {
        let c = self.piece_type.to_char();
        match self.color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }

    /// Parse a FEN piece letter; case selects the color.
    #[inline]
    pub const fn from_char(c: char) -> Option<Piece> {
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        match PieceType::from_char(c) {
            Some(piece_type) => Some(Piece { color, piece_type }),
            None => None,
        }
    }
}

impl fmt::Display for Piece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.color {
            Color::White => match self.piece_type {
                PieceType::Pawn => "P",
                PieceType::Knight => "N",
                PieceType::Bishop => "B",
                PieceType::Rook => "R",
                PieceType::Queen => "Q",
                PieceType::King => "K",
            },
            Color::Black => match self.piece_type {
                PieceType::Pawn => "p",
                PieceType::Knight => "n",
                PieceType::Bishop => "b",
                PieceType::Rook => "r",
                PieceType::Queen => "q",
                PieceType::King => "k",
            },
        })
    }
}

/// A board file (column), `0 = a` … `7 = h`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct File(pub u8);

impl File {
    pub const A: File = File(0);
    pub const B: File = File(1);
    pub const C: File = File(2);
    pub const D: File = File(3);
    pub const E: File = File(4);
    pub const F: File = File(5);
    pub const G: File = File(6);
    pub const H: File = File(7);

    /// Construct from `0..8`, if in range.
    #[inline]
    pub const fn new(i: u8) -> Option<File> {
        if i < 8 { Some(File(i)) } else { None }
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn to_char(self) -> char {
        (b'a' + self.0) as char
    }

    #[inline]
    pub const fn from_char(c: char) -> Option<File> {
        if c.is_ascii_lowercase() && (c as u8) <= b'h' {
            Some(File(c as u8 - b'a'))
        } else {
            None
        }
    }
}

/// A board rank (row), `0 = rank 1` … `7 = rank 8`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Rank(pub u8);

impl Rank {
    pub const R1: Rank = Rank(0);
    pub const R2: Rank = Rank(1);
    pub const R3: Rank = Rank(2);
    pub const R4: Rank = Rank(3);
    pub const R5: Rank = Rank(4);
    pub const R6: Rank = Rank(5);
    pub const R7: Rank = Rank(6);
    pub const R8: Rank = Rank(7);

    /// Construct from `0..8`, if in range.
    #[inline]
    pub const fn new(i: u8) -> Option<Rank> {
        if i < 8 { Some(Rank(i)) } else { None }
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn to_char(self) -> char {
        (b'1' + self.0) as char
    }

    #[inline]
    pub const fn from_char(c: char) -> Option<Rank> {
        let b = c as u8;
        if b >= b'1' && b <= b'8' {
            Some(Rank(b - b'1'))
        } else {
            None
        }
    }
}

/// A square `0..64` in little-endian rank-file order (`a1 = 0`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Square(pub u8);

impl Square {
    /// Total number of squares.
    pub const COUNT: usize = 64;

    /// Construct from a raw `0..64` index, if in range.
    #[inline]
    pub const fn new(i: u8) -> Option<Square> {
        if i < 64 { Some(Square(i)) } else { None }
    }

    /// Construct from a raw `0..64` index without bounds checking.
    ///
    /// # Safety / correctness
    /// The caller must ensure `i < 64`; passing a larger value yields a
    /// nonsensical square and may cause out-of-bounds table indexing later.
    #[inline]
    pub const fn new_unchecked(i: u8) -> Square {
        Square(i)
    }

    /// Compose a square from a file and rank.
    #[inline]
    pub const fn make(file: File, rank: Rank) -> Square {
        Square(rank.0 * 8 + file.0)
    }

    /// Raw `0..64` index.
    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn file(self) -> File {
        File(self.0 & 7)
    }

    #[inline]
    pub const fn rank(self) -> Rank {
        Rank(self.0 >> 3)
    }

    /// Mirror vertically (swap ranks): `a1 <-> a8`.
    #[inline]
    pub const fn flip_rank(self) -> Square {
        Square(self.0 ^ 56)
    }

    /// Mirror horizontally (swap files): `a1 <-> h1`.
    #[inline]
    pub const fn flip_file(self) -> Square {
        Square(self.0 ^ 7)
    }

    /// The single-bit bitboard for this square.
    #[inline]
    pub const fn bit(self) -> u64 {
        1u64 << self.0
    }

    /// Two-character algebraic name, e.g. `"e4"`.
    pub fn to_algebraic(self) -> [u8; 2] {
        [self.file().to_char() as u8, self.rank().to_char() as u8]
    }

    /// Parse algebraic coordinates like `"e4"`.
    pub fn from_algebraic(s: &str) -> Option<Square> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = File::from_char(bytes[0] as char)?;
        let rank = Rank::from_char(bytes[1] as char)?;
        Some(Square::make(file, rank))
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file().to_char(), self.rank().to_char())
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file().to_char(), self.rank().to_char())
    }
}

/// Named squares, for convenience and tests.
#[rustfmt::skip]
pub mod squares {
    use super::Square;
    macro_rules! sq { ($name:ident = $idx:expr) => { pub const $name: Square = Square($idx); }; }
    sq!(A1=0);  sq!(B1=1);  sq!(C1=2);  sq!(D1=3);  sq!(E1=4);  sq!(F1=5);  sq!(G1=6);  sq!(H1=7);
    sq!(A2=8);  sq!(B2=9);  sq!(C2=10); sq!(D2=11); sq!(E2=12); sq!(F2=13); sq!(G2=14); sq!(H2=15);
    sq!(A3=16); sq!(B3=17); sq!(C3=18); sq!(D3=19); sq!(E3=20); sq!(F3=21); sq!(G3=22); sq!(H3=23);
    sq!(A4=24); sq!(B4=25); sq!(C4=26); sq!(D4=27); sq!(E4=28); sq!(F4=29); sq!(G4=30); sq!(H4=31);
    sq!(A5=32); sq!(B5=33); sq!(C5=34); sq!(D5=35); sq!(E5=36); sq!(F5=37); sq!(G5=38); sq!(H5=39);
    sq!(A6=40); sq!(B6=41); sq!(C6=42); sq!(D6=43); sq!(E6=44); sq!(F6=45); sq!(G6=46); sq!(H6=47);
    sq!(A7=48); sq!(B7=49); sq!(C7=50); sq!(D7=51); sq!(E7=52); sq!(F7=53); sq!(G7=54); sq!(H7=55);
    sq!(A8=56); sq!(B8=57); sq!(C8=58); sq!(D8=59); sq!(E8=60); sq!(F8=61); sq!(G8=62); sq!(H8=63);
}

/// One side of one color's castling ability.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CastlingSide {
    /// Short castle (toward the h-file rook).
    King,
    /// Long castle (toward the a-file rook).
    Queen,
}

/// The set of remaining castling rights, as a 4-bit set.
///
/// Bit layout: `0 = White O-O`, `1 = White O-O-O`, `2 = Black O-O`,
/// `3 = Black O-O-O`. This ordering matches the Polyglot castling key order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct CastlingRights(pub u8);

impl CastlingRights {
    pub const NONE: CastlingRights = CastlingRights(0);
    pub const WHITE_KING: u8 = 1 << 0;
    pub const WHITE_QUEEN: u8 = 1 << 1;
    pub const BLACK_KING: u8 = 1 << 2;
    pub const BLACK_QUEEN: u8 = 1 << 3;
    pub const ALL: CastlingRights = CastlingRights(0b1111);

    /// Bit mask for a given color + side.
    #[inline]
    pub const fn mask(color: Color, side: CastlingSide) -> u8 {
        match (color, side) {
            (Color::White, CastlingSide::King) => Self::WHITE_KING,
            (Color::White, CastlingSide::Queen) => Self::WHITE_QUEEN,
            (Color::Black, CastlingSide::King) => Self::BLACK_KING,
            (Color::Black, CastlingSide::Queen) => Self::BLACK_QUEEN,
        }
    }

    #[inline]
    pub const fn has(self, color: Color, side: CastlingSide) -> bool {
        self.0 & Self::mask(color, side) != 0
    }

    #[inline]
    pub const fn with(self, color: Color, side: CastlingSide) -> CastlingRights {
        CastlingRights(self.0 | Self::mask(color, side))
    }

    /// Remove a specific right.
    #[inline]
    pub fn remove(&mut self, color: Color, side: CastlingSide) {
        self.0 &= !Self::mask(color, side);
    }

    /// Remove both of a color's rights (e.g. after the king moves).
    #[inline]
    pub fn remove_color(&mut self, color: Color) {
        let both = match color {
            Color::White => Self::WHITE_KING | Self::WHITE_QUEEN,
            Color::Black => Self::BLACK_KING | Self::BLACK_QUEEN,
        };
        self.0 &= !both;
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}
