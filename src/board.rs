//! The working board representation: per-piece-type and per-color bitboards
//! plus a byte mailbox for O(1) square lookups, an incremental Polyglot Zobrist
//! key, and make/unmake.
//!
//! This is the "fast" half of the hybrid design. The compact 34-byte canonical
//! form lives in [`crate::packed`]; convert with [`Board::pack`] / [`Board::unpack`].

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::moves::Move;
use crate::types::{
    CastlingRights, CastlingSide, Color, Piece, PieceType, Rank, Square, squares::*,
};
use crate::zobrist;

/// Nibble code for a piece in the mailbox / packed board: `0` = empty,
/// otherwise `(color << 3) | (piece_type + 1)`. White uses `1..=6`, Black `9..=14`.
#[inline]
pub(crate) const fn encode_piece(p: Piece) -> u8 {
    ((p.color as u8) << 3) | (p.piece_type as u8 + 1)
}

/// Decode a mailbox/packed nibble; `0` yields `None`.
#[inline]
pub(crate) const fn decode_piece(code: u8) -> Option<Piece> {
    if code == 0 {
        return None;
    }
    let color = if code & 0b1000 != 0 {
        Color::Black
    } else {
        Color::White
    };
    match PieceType::from_index((code & 0b111) as usize - 1) {
        Some(pt) => Some(Piece::new(color, pt)),
        None => None,
    }
}

/// Per-square mask AND-ed into the castling rights whenever a piece moves from
/// or to that square. Clears the rights invalidated by activity there.
#[rustfmt::skip]
const CASTLE_RIGHTS_MASK: [u8; 64] = {
    let mut m = [0b1111u8; 64];
    // White king home e1 (4): clears WK|WQ. Rooks a1 (0): WQ; h1 (7): WK.
    m[4]  = !(CastlingRights::WHITE_KING | CastlingRights::WHITE_QUEEN);
    m[0]  = !CastlingRights::WHITE_QUEEN;
    m[7]  = !CastlingRights::WHITE_KING;
    // Black king home e8 (60): clears BK|BQ. Rooks a8 (56): BQ; h8 (63): BK.
    m[60] = !(CastlingRights::BLACK_KING | CastlingRights::BLACK_QUEEN);
    m[56] = !CastlingRights::BLACK_QUEEN;
    m[63] = !CastlingRights::BLACK_KING;
    m
};

/// Information needed to reverse a [`Board::make_move`].
#[derive(Clone, Copy, Debug)]
pub struct Undo {
    pub captured: Option<Piece>,
    pub castling: CastlingRights,
    pub ep_square: Option<Square>,
    pub halfmove_clock: u16,
    pub hash: u64,
}

/// A full chess position.
#[derive(Clone, PartialEq, Eq)]
pub struct Board {
    /// Bitboards indexed by [`PieceType::index`], covering both colors.
    pub(crate) pieces: [Bitboard; 6],
    /// Occupancy by color, indexed by [`Color::index`].
    pub(crate) colors: [Bitboard; 2],
    /// Redundant byte-per-square view for O(1) `piece_at`.
    pub(crate) mailbox: [u8; 64],
    pub(crate) side_to_move: Color,
    pub(crate) castling: CastlingRights,
    /// En-passant target square (the square *behind* a pawn that just
    /// double-pushed), as in FEN — set whether or not a capture is possible.
    pub(crate) ep_square: Option<Square>,
    pub(crate) halfmove_clock: u16,
    pub(crate) fullmove_number: u16,
    pub(crate) hash: u64,
}

impl Board {
    /// A completely empty board, White to move, no rights. Mostly a building
    /// block for FEN parsing and tests.
    pub fn empty() -> Board {
        Board {
            pieces: [Bitboard::EMPTY; 6],
            colors: [Bitboard::EMPTY; 2],
            mailbox: [0; 64],
            side_to_move: Color::White,
            castling: CastlingRights::NONE,
            ep_square: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            hash: 0,
        }
    }

    /// The standard starting position.
    pub fn startpos() -> Board {
        Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("valid start FEN")
    }

    // --- accessors ---

    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }
    #[inline]
    pub fn castling_rights(&self) -> CastlingRights {
        self.castling
    }
    #[inline]
    pub fn en_passant_square(&self) -> Option<Square> {
        self.ep_square
    }
    #[inline]
    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }
    #[inline]
    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }
    /// The incremental Polyglot-compatible Zobrist key.
    #[inline]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    #[inline]
    pub fn occupied(&self) -> Bitboard {
        self.colors[0] | self.colors[1]
    }

    #[inline]
    pub fn pieces(&self, pt: PieceType) -> Bitboard {
        self.pieces[pt.index()]
    }

    #[inline]
    pub fn color_bb(&self, color: Color) -> Bitboard {
        self.colors[color.index()]
    }

    #[inline]
    pub fn pieces_colored(&self, pt: PieceType, color: Color) -> Bitboard {
        self.pieces[pt.index()] & self.colors[color.index()]
    }

    #[inline]
    pub fn piece_at(&self, sq: Square) -> Option<Piece> {
        decode_piece(self.mailbox[sq.index()])
    }

    #[inline]
    pub fn piece_type_at(&self, sq: Square) -> Option<PieceType> {
        decode_piece(self.mailbox[sq.index()]).map(|p| p.piece_type)
    }

    #[inline]
    pub fn color_at(&self, sq: Square) -> Option<Color> {
        decode_piece(self.mailbox[sq.index()]).map(|p| p.color)
    }

    #[inline]
    pub fn king_square(&self, color: Color) -> Square {
        (self.pieces[PieceType::King.index()] & self.colors[color.index()]).lsb_unchecked()
    }

    // --- piece mutation (keeps bitboards, mailbox, and hash in sync) ---

    #[inline]
    fn put_piece(&mut self, piece: Piece, sq: Square) {
        let bit = Bitboard::from_square(sq);
        self.pieces[piece.piece_type.index()] |= bit;
        self.colors[piece.color.index()] |= bit;
        self.mailbox[sq.index()] = encode_piece(piece);
        self.hash ^= zobrist::piece_key(piece, sq.0);
    }

    #[inline]
    fn remove_piece(&mut self, piece: Piece, sq: Square) {
        let bit = Bitboard::from_square(sq);
        self.pieces[piece.piece_type.index()].0 &= !bit.0;
        self.colors[piece.color.index()].0 &= !bit.0;
        self.mailbox[sq.index()] = 0;
        self.hash ^= zobrist::piece_key(piece, sq.0);
    }

    #[inline]
    fn move_piece(&mut self, piece: Piece, from: Square, to: Square) {
        self.remove_piece(piece, from);
        self.put_piece(piece, to);
    }

    /// Place a piece during construction (FEN parsing); updates the piece hash
    /// only. Callers fold in side/castling/ep keys via [`Board::recompute_hash`].
    pub(crate) fn set_square(&mut self, sq: Square, piece: Piece) {
        self.put_piece(piece, sq);
    }

    // --- attack / check queries ---

    /// All pieces of either color that attack `sq` under occupancy `occ`.
    #[inline]
    pub fn attackers_to(&self, sq: Square, occ: Bitboard) -> Bitboard {
        let knights = self.pieces[PieceType::Knight.index()] & attacks::knight_attacks(sq);
        let kings = self.pieces[PieceType::King.index()] & attacks::king_attacks(sq);
        let bishops_queens = (self.pieces[PieceType::Bishop.index()]
            | self.pieces[PieceType::Queen.index()])
            & attacks::bishop_attacks(sq, occ);
        let rooks_queens = (self.pieces[PieceType::Rook.index()]
            | self.pieces[PieceType::Queen.index()])
            & attacks::rook_attacks(sq, occ);
        let white_pawns = self.pieces_colored(PieceType::Pawn, Color::White)
            & attacks::pawn_attacks(Color::Black, sq);
        let black_pawns = self.pieces_colored(PieceType::Pawn, Color::Black)
            & attacks::pawn_attacks(Color::White, sq);
        knights | kings | bishops_queens | rooks_queens | white_pawns | black_pawns
    }

    /// Whether `sq` is attacked by any piece of `by`, under occupancy `occ`.
    #[inline]
    pub fn is_attacked(&self, sq: Square, by: Color, occ: Bitboard) -> bool {
        let pawns = self.pieces_colored(PieceType::Pawn, by);
        if (attacks::pawn_attacks(by.flip(), sq) & pawns).any() {
            return true;
        }
        if (attacks::knight_attacks(sq) & self.pieces_colored(PieceType::Knight, by)).any() {
            return true;
        }
        if (attacks::king_attacks(sq) & self.pieces_colored(PieceType::King, by)).any() {
            return true;
        }
        let bishops_queens =
            self.pieces[PieceType::Bishop.index()] | self.pieces[PieceType::Queen.index()];
        if (attacks::bishop_attacks(sq, occ) & bishops_queens & self.colors[by.index()]).any() {
            return true;
        }
        let rooks_queens =
            self.pieces[PieceType::Rook.index()] | self.pieces[PieceType::Queen.index()];
        if (attacks::rook_attacks(sq, occ) & rooks_queens & self.colors[by.index()]).any() {
            return true;
        }
        false
    }

    /// Enemy pieces giving check to `color`'s king.
    #[inline]
    pub fn checkers_of(&self, color: Color) -> Bitboard {
        let ksq = self.king_square(color);
        self.attackers_to(ksq, self.occupied()) & self.colors[color.flip().index()]
    }

    /// Whether the side to move is in check.
    #[inline]
    pub fn in_check(&self) -> bool {
        self.is_attacked(
            self.king_square(self.side_to_move),
            self.side_to_move.flip(),
            self.occupied(),
        )
    }

    /// Whether `color`'s king is in check.
    #[inline]
    pub fn is_check_for(&self, color: Color) -> bool {
        self.is_attacked(self.king_square(color), color.flip(), self.occupied())
    }

    // --- make / unmake ---

    /// The en-passant Zobrist contribution for `ep`, hashed only when a pawn of
    /// `capturer` can actually make the capture (the Polyglot rule).
    #[inline]
    fn ep_hash_contribution(&self, ep: Square, capturer: Color) -> u64 {
        let origins = attacks::pawn_attacks(capturer.flip(), ep);
        if (origins & self.pieces_colored(PieceType::Pawn, capturer)).any() {
            zobrist::ep_file_key(ep.file())
        } else {
            0
        }
    }

    /// Apply `mv`, returning the information needed to undo it. The move must be
    /// legal (or at least pseudo-legal with a real piece on `from`).
    pub fn make_move(&mut self, mv: Move) -> Undo {
        let undo = Undo {
            captured: None,
            castling: self.castling,
            ep_square: self.ep_square,
            halfmove_clock: self.halfmove_clock,
            hash: self.hash,
        };

        let us = self.side_to_move;
        let them = us.flip();
        let from = mv.from();
        let to = mv.to();
        let moving = self.piece_at(from).expect("piece on from-square");
        let pt = moving.piece_type;

        // Remove the previous en-passant hash contribution (computed for the
        // side that was to move, i.e. `us`).
        if let Some(ep) = self.ep_square {
            self.hash ^= self.ep_hash_contribution(ep, us);
        }
        self.ep_square = None;

        let mut captured = None;
        self.halfmove_clock += 1;

        // Captures.
        if mv.is_en_passant() {
            let cap_sq = Square::make(to.file(), from.rank());
            let cap_piece = Piece::new(them, PieceType::Pawn);
            self.remove_piece(cap_piece, cap_sq);
            captured = Some(cap_piece);
            self.halfmove_clock = 0;
        } else if mv.is_capture() {
            let cap_piece = self.piece_at(to).expect("piece on capture target");
            self.remove_piece(cap_piece, to);
            captured = Some(cap_piece);
            self.halfmove_clock = 0;
        }

        // Move the piece (handling promotion).
        if let Some(promo) = mv.promotion_piece() {
            self.remove_piece(moving, from);
            self.put_piece(Piece::new(us, promo), to);
        } else {
            self.move_piece(moving, from, to);
        }
        if pt == PieceType::Pawn {
            self.halfmove_clock = 0;
        }

        // Castling: relocate the rook.
        if mv.is_king_castle() {
            let (rfrom, rto) = match us {
                Color::White => (H1, F1),
                Color::Black => (H8, F8),
            };
            self.move_piece(Piece::new(us, PieceType::Rook), rfrom, rto);
        } else if mv.is_queen_castle() {
            let (rfrom, rto) = match us {
                Color::White => (A1, D1),
                Color::Black => (A8, D8),
            };
            self.move_piece(Piece::new(us, PieceType::Rook), rfrom, rto);
        }

        // Update castling rights (king/rook moves, or rook captured on home sq).
        // Skip entirely once no rights remain — true for most deep-subtree nodes.
        if self.castling.0 != 0 {
            let new_castling = CastlingRights(
                self.castling.0 & CASTLE_RIGHTS_MASK[from.index()] & CASTLE_RIGHTS_MASK[to.index()],
            );
            if new_castling != self.castling {
                self.hash ^=
                    zobrist::castling_key(self.castling) ^ zobrist::castling_key(new_castling);
                self.castling = new_castling;
            }
        }

        // Set the new en-passant square on a double push, and add its hash
        // contribution for the side now to move (`them`).
        if mv.is_double_push() {
            let ep = Square::make(from.file(), Rank((from.rank().0 + to.rank().0) / 2));
            self.ep_square = Some(ep);
            self.hash ^= self.ep_hash_contribution(ep, them);
        }

        // Flip side and clocks.
        self.hash ^= zobrist::turn_key();
        if us == Color::Black {
            self.fullmove_number += 1;
        }
        self.side_to_move = them;

        Undo {
            captured,
            ..undo
        }
    }

    /// Reverse a previously applied move.
    pub fn unmake_move(&mut self, mv: Move, undo: Undo) {
        let them = self.side_to_move; // side that did NOT move
        let us = them.flip(); // side that made `mv`
        self.side_to_move = us;
        if us == Color::Black {
            self.fullmove_number -= 1;
        }

        let from = mv.from();
        let to = mv.to();

        // Move the piece back (raw, since we restore the hash wholesale).
        if let Some(promo) = mv.promotion_piece() {
            self.remove_piece_raw(Piece::new(us, promo), to);
            self.put_piece_raw(Piece::new(us, PieceType::Pawn), from);
        } else {
            let pt = self.piece_type_at(to).expect("piece on destination");
            self.move_piece_raw(Piece::new(us, pt), to, from);
        }

        // Restore a captured piece.
        if mv.is_en_passant() {
            let cap_sq = Square::make(to.file(), from.rank());
            self.put_piece_raw(Piece::new(them, PieceType::Pawn), cap_sq);
        } else if let Some(cap) = undo.captured {
            self.put_piece_raw(cap, to);
        }

        // Undo the castling rook relocation.
        if mv.is_king_castle() {
            let (rfrom, rto) = match us {
                Color::White => (H1, F1),
                Color::Black => (H8, F8),
            };
            self.move_piece_raw(Piece::new(us, PieceType::Rook), rto, rfrom);
        } else if mv.is_queen_castle() {
            let (rfrom, rto) = match us {
                Color::White => (A1, D1),
                Color::Black => (A8, D8),
            };
            self.move_piece_raw(Piece::new(us, PieceType::Rook), rto, rfrom);
        }

        self.castling = undo.castling;
        self.ep_square = undo.ep_square;
        self.halfmove_clock = undo.halfmove_clock;
        self.hash = undo.hash;
    }

    // raw (no-hash) mutators used by unmake
    #[inline]
    fn put_piece_raw(&mut self, piece: Piece, sq: Square) {
        let bit = Bitboard::from_square(sq);
        self.pieces[piece.piece_type.index()] |= bit;
        self.colors[piece.color.index()] |= bit;
        self.mailbox[sq.index()] = encode_piece(piece);
    }
    #[inline]
    fn remove_piece_raw(&mut self, piece: Piece, sq: Square) {
        let bit = Bitboard::from_square(sq);
        self.pieces[piece.piece_type.index()].0 &= !bit.0;
        self.colors[piece.color.index()].0 &= !bit.0;
        self.mailbox[sq.index()] = 0;
    }
    #[inline]
    fn move_piece_raw(&mut self, piece: Piece, from: Square, to: Square) {
        self.remove_piece_raw(piece, from);
        self.put_piece_raw(piece, to);
    }

    /// Recompute the full Zobrist key from scratch (used after construction and
    /// as a consistency check against the incremental key).
    pub fn recompute_hash(&self) -> u64 {
        let mut h = 0u64;
        for sq in 0..64u8 {
            if let Some(p) = self.piece_at(Square(sq)) {
                h ^= zobrist::piece_key(p, sq);
            }
        }
        h ^= zobrist::castling_key(self.castling);
        if let Some(ep) = self.ep_square {
            h ^= self.ep_hash_contribution(ep, self.side_to_move);
        }
        if self.side_to_move == Color::White {
            h ^= zobrist::turn_key();
        }
        h
    }

    /// Set the stored hash to the freshly recomputed value.
    pub(crate) fn refresh_hash(&mut self) {
        self.hash = self.recompute_hash();
    }

    /// Does `color` have the right and geometry to castle on `side`?
    /// (Squares-empty and not-through-check are validated in move generation.)
    #[inline]
    pub fn has_castling_right(&self, color: Color, side: CastlingSide) -> bool {
        self.castling.has(color, side)
    }

    /// Overwrite the non-piece state in one shot (used by FEN/packed loaders).
    pub(crate) fn set_state(
        &mut self,
        side: Color,
        castling: CastlingRights,
        ep_square: Option<Square>,
        halfmove_clock: u16,
        fullmove_number: u16,
    ) {
        self.side_to_move = side;
        self.castling = castling;
        self.ep_square = ep_square;
        self.halfmove_clock = halfmove_clock;
        self.fullmove_number = fullmove_number;
    }

    /// Whether the position is a dead draw by insufficient material: KvK,
    /// K+single-minor vs K, or only same-colored bishops. (KNN vs K is *not*
    /// considered insufficient, matching FIDE.)
    pub fn is_insufficient_material(&self) -> bool {
        const DARK_SQUARES: u64 = 0xAA55_AA55_AA55_AA55;
        const LIGHT_SQUARES: u64 = 0x55AA_55AA_55AA_55AA;
        let pawns = self.pieces[PieceType::Pawn.index()];
        let rooks = self.pieces[PieceType::Rook.index()];
        let queens = self.pieces[PieceType::Queen.index()];
        if (pawns | rooks | queens).any() {
            return false;
        }
        let knights = self.pieces[PieceType::Knight.index()].count();
        let bishops = self.pieces[PieceType::Bishop.index()];
        if knights + bishops.count() <= 1 {
            return true;
        }
        if knights == 0 {
            let on_dark = bishops.0 & DARK_SQUARES != 0;
            let on_light = bishops.0 & LIGHT_SQUARES != 0;
            return !(on_dark && on_light);
        }
        false
    }
}

use core::fmt;

impl fmt::Display for Board {
    /// ASCII board, rank 8 on top.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for rank in (0..8).rev() {
            for file in 0..8 {
                let sq = Square(rank * 8 + file);
                match self.piece_at(sq) {
                    Some(p) => write!(f, "{} ", p)?,
                    None => f.write_str(". ")?,
                }
            }
            f.write_str("\n")?;
        }
        write!(
            f,
            "{} to move",
            match self.side_to_move {
                Color::White => "White",
                Color::Black => "Black",
            }
        )
    }
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Board({})", self.to_fen())
    }
}
