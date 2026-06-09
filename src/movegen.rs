//! Move generation.
//!
//! Two generators share one definition of legality:
//!  * [`Board::pseudo_legal_moves`] enumerates moves that are legal except for
//!    possibly leaving one's own king in check (castling, however, is fully
//!    validated here because "king safe afterwards" cannot see a king that
//!    *passes through* an attacked square).
//!  * [`Board::legal_moves`] filters those with a make / king-safety / unmake
//!    pass.
//!
//! [`Board::perft`] is the standard correctness oracle and is validated against
//! large downloaded suites in the integration tests.

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::board::Board;
use crate::moves::{Move, MoveList};
use crate::types::{
    CastlingSide, Color, PieceType, Rank, Square, squares::*,
};

/// Castling descriptor: the king's from/to, the squares that must be empty, and
/// the squares the king touches that must be unattacked.
struct CastleSpec {
    king_from: Square,
    king_to: Square,
    empty: &'static [Square],
    safe: &'static [Square],
    side: CastlingSide,
}

const WHITE_CASTLES: [CastleSpec; 2] = [
    CastleSpec {
        king_from: E1,
        king_to: G1,
        empty: &[F1, G1],
        safe: &[E1, F1, G1],
        side: CastlingSide::King,
    },
    CastleSpec {
        king_from: E1,
        king_to: C1,
        empty: &[B1, C1, D1],
        safe: &[E1, D1, C1],
        side: CastlingSide::Queen,
    },
];

const BLACK_CASTLES: [CastleSpec; 2] = [
    CastleSpec {
        king_from: E8,
        king_to: G8,
        empty: &[F8, G8],
        safe: &[E8, F8, G8],
        side: CastlingSide::King,
    },
    CastleSpec {
        king_from: E8,
        king_to: C8,
        empty: &[B8, C8, D8],
        safe: &[E8, D8, C8],
        side: CastlingSide::Queen,
    },
];

impl Board {
    /// All pseudo-legal moves for the side to move.
    pub fn pseudo_legal_moves(&self) -> MoveList {
        let mut list = MoveList::new();
        self.gen_pawn_moves(&mut list);
        self.gen_knight_moves(&mut list);
        self.gen_slider_moves(&mut list);
        self.gen_king_moves(&mut list);
        self.gen_castling(&mut list);
        list
    }

    /// All fully-legal moves for the side to move (fast pin-aware generator).
    pub fn legal_moves(&self) -> MoveList {
        let mut list = MoveList::new();
        self.generate_legal(&mut list);
        list
    }

    /// Reference legal generator: pseudo-legal moves filtered by make / king-
    /// safety / unmake. Slower but obviously correct; used to cross-check
    /// [`Board::generate_legal`] in tests.
    pub fn legal_moves_filtered(&self) -> MoveList {
        let pseudo = self.pseudo_legal_moves();
        let mut legal = MoveList::new();
        let mut scratch = self.clone();
        let us = self.side_to_move();
        for &mv in pseudo.iter() {
            let undo = scratch.make_move(mv);
            let king = scratch.king_square(us);
            if !scratch.is_attacked(king, us.flip(), scratch.occupied()) {
                legal.push(mv);
            }
            scratch.unmake_move(mv, undo);
        }
        legal
    }

    /// Whether the side to move has at least one legal move.
    pub fn has_legal_move(&self) -> bool {
        !self.legal_moves().is_empty()
    }

    fn enemy(&self) -> Bitboard {
        self.color_bb(self.side_to_move().flip())
    }

    /// Add quiet/capture moves to all `targets` (which must already exclude own
    /// pieces) for a non-pawn piece standing on `from`.
    #[inline]
    fn add_piece_moves(&self, from: Square, targets: Bitboard, list: &mut MoveList) {
        let enemy = self.enemy();
        let mut quiets = targets & !enemy;
        let mut caps = targets & enemy;
        while let Some(to) = quiets.pop_lsb() {
            list.push(Move::quiet(from, to));
        }
        while let Some(to) = caps.pop_lsb() {
            list.push(Move::capture(from, to));
        }
    }

    fn gen_knight_moves(&self, list: &mut MoveList) {
        let us = self.side_to_move();
        let own = self.color_bb(us);
        let mut knights = self.pieces_colored(PieceType::Knight, us);
        while let Some(from) = knights.pop_lsb() {
            let targets = attacks::knight_attacks(from) & !own;
            self.add_piece_moves(from, targets, list);
        }
    }

    fn gen_slider_moves(&self, list: &mut MoveList) {
        let us = self.side_to_move();
        let own = self.color_bb(us);
        let occ = self.occupied();
        let mut bishops = self.pieces_colored(PieceType::Bishop, us)
            | self.pieces_colored(PieceType::Queen, us);
        while let Some(from) = bishops.pop_lsb() {
            let targets = attacks::bishop_attacks(from, occ) & !own;
            self.add_piece_moves(from, targets, list);
        }
        let mut rooks = self.pieces_colored(PieceType::Rook, us)
            | self.pieces_colored(PieceType::Queen, us);
        while let Some(from) = rooks.pop_lsb() {
            let targets = attacks::rook_attacks(from, occ) & !own;
            self.add_piece_moves(from, targets, list);
        }
    }

    fn gen_king_moves(&self, list: &mut MoveList) {
        let us = self.side_to_move();
        let own = self.color_bb(us);
        let from = self.king_square(us);
        let targets = attacks::king_attacks(from) & !own;
        self.add_piece_moves(from, targets, list);
    }

    fn gen_pawn_moves(&self, list: &mut MoveList) {
        let us = self.side_to_move();
        let them = us.flip();
        let mut pawns = self.pieces_colored(PieceType::Pawn, us);
        let empty = !self.occupied();
        let enemy = self.color_bb(them);
        let (promo_rank, start_rank) = match us {
            Color::White => (7u8, 1u8),
            Color::Black => (0u8, 6u8),
        };
        let fwd = us.forward();

        while let Some(from) = pawns.pop_lsb() {
            let r = from.rank().0;
            // Single push.
            let one = Square::make(from.file(), Rank((r as i8 + fwd) as u8));
            if empty.has(one) {
                if one.rank().0 == promo_rank {
                    self.push_promotions(from, one, false, list);
                } else {
                    list.push(Move::quiet(from, one));
                    // Double push from the starting rank.
                    if r == start_rank {
                        let two = Square::make(from.file(), Rank((r as i8 + 2 * fwd) as u8));
                        if empty.has(two) {
                            list.push(Move::double_push(from, two));
                        }
                    }
                }
            }
            // Captures (including promotion captures).
            let mut caps = attacks::pawn_attacks(us, from) & enemy;
            while let Some(to) = caps.pop_lsb() {
                if to.rank().0 == promo_rank {
                    self.push_promotions(from, to, true, list);
                } else {
                    list.push(Move::capture(from, to));
                }
            }
            // En passant.
            if let Some(ep) = self.en_passant_square()
                && attacks::pawn_attacks(us, from).has(ep)
            {
                list.push(Move::en_passant(from, ep));
            }
        }
    }

    #[inline]
    fn push_promotions(&self, from: Square, to: Square, capture: bool, list: &mut MoveList) {
        for &pt in &PieceType::PROMOTIONS {
            list.push(Move::promotion(from, to, pt, capture));
        }
    }

    fn gen_castling(&self, list: &mut MoveList) {
        let us = self.side_to_move();
        let them = us.flip();
        let occ = self.occupied();
        let specs = match us {
            Color::White => &WHITE_CASTLES,
            Color::Black => &BLACK_CASTLES,
        };
        for spec in specs {
            if !self.has_castling_right(us, spec.side) {
                continue;
            }
            // All squares between king and rook must be empty.
            if spec.empty.iter().any(|&sq| occ.has(sq)) {
                continue;
            }
            // The king must not start in, pass through, or land on attack.
            if spec
                .safe
                .iter()
                .any(|&sq| self.is_attacked(sq, them, occ))
            {
                continue;
            }
            let flag = match spec.side {
                CastlingSide::King => crate::moves::MoveFlag::KingCastle,
                CastlingSide::Queen => crate::moves::MoveFlag::QueenCastle,
            };
            list.push(Move::new(spec.king_from, spec.king_to, flag));
        }
    }

    /// Count leaf nodes of the move tree to `depth` — the standard perft metric.
    /// Uses the fast legal generator with depth-1 bulk counting (the legal move
    /// count *is* the leaf count), and make/unmake for deeper plies.
    pub fn perft(&mut self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let mut moves = MoveList::new();
        self.generate_legal(&mut moves);
        if depth == 1 {
            return moves.len() as u64;
        }
        let mut nodes = 0;
        for &mv in moves.iter() {
            let undo = self.make_move(mv);
            nodes += self.perft(depth - 1);
            self.unmake_move(mv, undo);
        }
        nodes
    }

    /// Perft "divide": leaves below each legal root move (move-gen debugging).
    pub fn perft_divide(&mut self, depth: u32) -> Vec<(Move, u64)> {
        let mut out = Vec::new();
        let moves = self.legal_moves();
        for &mv in moves.iter() {
            let undo = self.make_move(mv);
            let n = if depth <= 1 { 1 } else { self.perft(depth - 1) };
            out.push((mv, n));
            self.unmake_move(mv, undo);
        }
        out
    }

    // ===================== fast pin-aware legal generation =====================

    /// Generate fully-legal moves directly — no make/unmake filtering. Uses a
    /// check mask (block-or-capture squares), pin rays (a pinned piece may only
    /// move along the king–pinner line), and a king-danger map (enemy attacks
    /// with our king removed from the occupancy, so sliders see through it).
    /// En passant, whose horizontal discovered-check case neither the check
    /// mask nor the pin ray can express, is validated exactly in
    /// [`Board::ep_is_legal`].
    pub fn generate_legal(&self, list: &mut MoveList) {
        let us = self.side_to_move();
        let them = us.flip();
        let king = self.king_square(us);
        let occ = self.occupied();
        let us_bb = self.color_bb(us);
        let them_bb = self.color_bb(them);

        // King-danger squares: enemy attacks with our king transparent.
        let danger = self.attack_span(them, Bitboard(occ.0 ^ king.bit()));

        // King moves never enter danger or capture own pieces.
        let king_targets = attacks::king_attacks(king) & !us_bb & !danger;
        self.add_split(king, king_targets, them_bb, list);

        let checkers = self.attackers_to(king, occ) & them_bb;
        // Double check: only the king may move.
        if checkers.count() >= 2 {
            return;
        }
        let check_mask = if let Some(checker) = checkers.lsb() {
            attacks::between(king, checker) | Bitboard::from_square(checker)
        } else {
            Bitboard::FULL
        };

        let pinned = self.compute_pins(king, us, them, occ);

        // Knights — a pinned knight can never move legally.
        let mut knights = self.pieces_colored(PieceType::Knight, us) & !pinned;
        while let Some(from) = knights.pop_lsb() {
            let targets = attacks::knight_attacks(from) & !us_bb & check_mask;
            self.add_split(from, targets, them_bb, list);
        }

        // Diagonal sliders (bishops + queens).
        let mut diag =
            self.pieces_colored(PieceType::Bishop, us) | self.pieces_colored(PieceType::Queen, us);
        while let Some(from) = diag.pop_lsb() {
            let mut targets = attacks::bishop_attacks(from, occ) & !us_bb & check_mask;
            if pinned.has(from) {
                targets &= attacks::line(king, from);
            }
            self.add_split(from, targets, them_bb, list);
        }
        // Orthogonal sliders (rooks + queens).
        let mut orth =
            self.pieces_colored(PieceType::Rook, us) | self.pieces_colored(PieceType::Queen, us);
        while let Some(from) = orth.pop_lsb() {
            let mut targets = attacks::rook_attacks(from, occ) & !us_bb & check_mask;
            if pinned.has(from) {
                targets &= attacks::line(king, from);
            }
            self.add_split(from, targets, them_bb, list);
        }

        // Pawns.
        self.gen_pawn_legal(king, us, them, occ, them_bb, check_mask, pinned, list);

        // Castling only when not in check.
        if checkers.is_empty() {
            self.gen_castling(list);
        }
    }

    /// Push quiet and capture moves from `from` to every square in `targets`
    /// (which must already exclude own pieces); `them_bb` selects captures.
    #[inline]
    fn add_split(&self, from: Square, targets: Bitboard, them_bb: Bitboard, list: &mut MoveList) {
        let mut quiets = targets & !them_bb;
        let mut caps = targets & them_bb;
        while let Some(to) = quiets.pop_lsb() {
            list.push(Move::quiet(from, to));
        }
        while let Some(to) = caps.pop_lsb() {
            list.push(Move::capture(from, to));
        }
    }

    /// Every square attacked by `color` under occupancy `occ` (used for the
    /// king-danger map, where `occ` has the opposing king removed).
    fn attack_span(&self, color: Color, occ: Bitboard) -> Bitboard {
        let pawns = self.pieces_colored(PieceType::Pawn, color);
        let mut span = match color {
            Color::White => pawns.north_east() | pawns.north_west(),
            Color::Black => pawns.south_east() | pawns.south_west(),
        };
        let mut knights = self.pieces_colored(PieceType::Knight, color);
        while let Some(s) = knights.pop_lsb() {
            span |= attacks::knight_attacks(s);
        }
        span |= attacks::king_attacks(self.king_square(color));
        let mut diag = self.pieces_colored(PieceType::Bishop, color)
            | self.pieces_colored(PieceType::Queen, color);
        while let Some(s) = diag.pop_lsb() {
            span |= attacks::bishop_attacks(s, occ);
        }
        let mut orth = self.pieces_colored(PieceType::Rook, color)
            | self.pieces_colored(PieceType::Queen, color);
        while let Some(s) = orth.pop_lsb() {
            span |= attacks::rook_attacks(s, occ);
        }
        span
    }

    /// Bitboard of our pieces pinned against our king.
    fn compute_pins(&self, king: Square, us: Color, them: Color, occ: Bitboard) -> Bitboard {
        let us_bb = self.color_bb(us);
        let them_bb = self.color_bb(them);
        let rq = (self.pieces(PieceType::Rook) | self.pieces(PieceType::Queen)) & them_bb;
        let bq = (self.pieces(PieceType::Bishop) | self.pieces(PieceType::Queen)) & them_bb;
        // Snipers: enemy sliders that would attack the king on an empty board.
        let mut snipers = (attacks::rook_attacks(king, Bitboard::EMPTY) & rq)
            | (attacks::bishop_attacks(king, Bitboard::EMPTY) & bq);
        let mut pinned = Bitboard::EMPTY;
        while let Some(s) = snipers.pop_lsb() {
            let blockers = attacks::between(king, s) & occ;
            if blockers.is_single() && (blockers & us_bb).any() {
                pinned |= blockers;
            }
        }
        pinned
    }

    #[allow(clippy::too_many_arguments)]
    fn gen_pawn_legal(
        &self,
        king: Square,
        us: Color,
        them: Color,
        occ: Bitboard,
        them_bb: Bitboard,
        check_mask: Bitboard,
        pinned: Bitboard,
        list: &mut MoveList,
    ) {
        let mut pawns = self.pieces_colored(PieceType::Pawn, us);
        let (promo_rank, start_rank) = match us {
            Color::White => (7u8, 1u8),
            Color::Black => (0u8, 6u8),
        };
        let fwd = us.forward();
        let empty = !occ;

        while let Some(from) = pawns.pop_lsb() {
            let pin_mask = if pinned.has(from) {
                attacks::line(king, from)
            } else {
                Bitboard::FULL
            };
            let mask = check_mask & pin_mask;
            let r = from.rank().0;

            // Pushes.
            let one = Square::make(from.file(), Rank((r as i8 + fwd) as u8));
            if empty.has(one) {
                if mask.has(one) {
                    if one.rank().0 == promo_rank {
                        self.push_promotions(from, one, false, list);
                    } else {
                        list.push(Move::quiet(from, one));
                    }
                }
                if r == start_rank {
                    let two = Square::make(from.file(), Rank((r as i8 + 2 * fwd) as u8));
                    if empty.has(two) && mask.has(two) {
                        list.push(Move::double_push(from, two));
                    }
                }
            }

            // Captures and promotion-captures.
            let mut caps = attacks::pawn_attacks(us, from) & them_bb & mask;
            while let Some(to) = caps.pop_lsb() {
                if to.rank().0 == promo_rank {
                    self.push_promotions(from, to, true, list);
                } else {
                    list.push(Move::capture(from, to));
                }
            }

            // En passant — validated exactly (its discovered-check case is
            // invisible to the check mask / pin ray).
            if let Some(ep) = self.en_passant_square()
                && attacks::pawn_attacks(us, from).has(ep)
                && self.ep_is_legal(from, ep, king, us, them, occ)
            {
                list.push(Move::en_passant(from, ep));
            }
        }
    }

    /// Exact legality test for an en-passant capture: removes both pawns, places
    /// the capturer on the ep square, and checks for any remaining attack on the
    /// king (covering the rank-discovered-check and the captured-pawn-was-the-
    /// checker cases that the pin/check machinery cannot represent).
    fn ep_is_legal(
        &self,
        from: Square,
        ep: Square,
        king: Square,
        us: Color,
        them: Color,
        occ: Bitboard,
    ) -> bool {
        let cap_sq = Square::make(ep.file(), from.rank());
        let new_occ = Bitboard((occ.0 ^ from.bit() ^ cap_sq.bit()) | ep.bit());
        let them_bb = self.color_bb(them);

        // Remaining non-slider checks (occupancy-independent; captured pawn out).
        let enemy_pawns = Bitboard(self.pieces_colored(PieceType::Pawn, them).0 & !cap_sq.bit());
        if (attacks::pawn_attacks(us, king) & enemy_pawns).any() {
            return false;
        }
        if (attacks::knight_attacks(king) & self.pieces_colored(PieceType::Knight, them)).any() {
            return false;
        }
        if (attacks::king_attacks(king) & self.pieces_colored(PieceType::King, them)).any() {
            return false;
        }
        // Slider checks under the post-capture occupancy.
        let rq = (self.pieces(PieceType::Rook) | self.pieces(PieceType::Queen)) & them_bb;
        let bq = (self.pieces(PieceType::Bishop) | self.pieces(PieceType::Queen)) & them_bb;
        if (attacks::rook_attacks(king, new_occ) & rq).any() {
            return false;
        }
        if (attacks::bishop_attacks(king, new_occ) & bq).any() {
            return false;
        }
        true
    }
}
