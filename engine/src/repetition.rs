//! Exact FIDE repetition identity and its stable textual position identifier.
//!
//! [`Board::hash`](crate::Board::hash) intentionally follows Polyglot's fast
//! Zobrist convention.  That key is useful for opening books and the
//! transposition table, but it is not the rules definition of "the same
//! position": Polyglot retains some en-passant targets for which every capture
//! is illegal (for example because the pawn is pinned), and any `u64` can
//! collide.  This module keeps those two jobs separate.

use crate::board::{Board, decode_piece};
use crate::types::{CastlingRights, Color, Square};

/// A collision-free, executable representation of the position components
/// relevant to FIDE repetition.
///
/// Move clocks are absent.  The en-passant square is present exactly when at
/// least one en-passant capture is legal, not merely pseudo-legal.  For
/// arbitrary FEN input, correspondence with FIDE's notion additionally assumes
/// the ordinary position well-formedness invariants (in particular meaningful
/// castling rights).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RepetitionKey {
    /// A normalized Zobrist prefilter. Equality still checks every field below,
    /// so collisions cannot change the result. Keeping it first makes the
    /// overwhelmingly common unequal-key comparison a single `u64` check.
    fingerprint: u64,
    placement: [u8; 64],
    turn: Color,
    castling_rights: CastlingRights,
    en_passant_target: Option<Square>,
}

impl RepetitionKey {
    pub(crate) fn from_board(board: &Board) -> RepetitionKey {
        let en_passant_target = board.effective_en_passant_square();
        let fingerprint = match (board.ep_square, en_passant_target) {
            // Remove Polyglot's legality-insensitive EP contribution. It is
            // zero already when no adjacent pawn exists.
            (Some(raw), None) => board.hash ^ board.ep_hash_contribution(raw, board.side_to_move),
            _ => board.hash,
        };
        RepetitionKey {
            fingerprint,
            placement: board.mailbox,
            turn: board.side_to_move,
            castling_rights: board.castling,
            en_passant_target,
        }
    }

    /// The legally effective en-passant component of this key.
    #[inline]
    pub fn en_passant_target(self) -> Option<Square> {
        self.en_passant_target
    }

    /// Canonical four-field effective EPD.
    ///
    /// This is the persistent cross-language `PositionId` used by the
    /// monorepo.  Unlike a Zobrist key it is collision-free and inspectable.
    pub fn position_id(self) -> String {
        let mut out = String::with_capacity(72);

        for rank in (0..8).rev() {
            let mut empty = 0u8;
            for file in 0..8 {
                let code = self.placement[rank * 8 + file];
                match decode_piece(code) {
                    Some(piece) => {
                        if empty != 0 {
                            out.push((b'0' + empty) as char);
                            empty = 0;
                        }
                        out.push(piece.to_char());
                    }
                    None => empty += 1,
                }
            }
            if empty != 0 {
                out.push((b'0' + empty) as char);
            }
            if rank != 0 {
                out.push('/');
            }
        }

        out.push(' ');
        out.push(match self.turn {
            Color::White => 'w',
            Color::Black => 'b',
        });

        out.push(' ');
        if self.castling_rights.is_empty() {
            out.push('-');
        } else {
            if self.castling_rights.0 & CastlingRights::WHITE_KING != 0 {
                out.push('K');
            }
            if self.castling_rights.0 & CastlingRights::WHITE_QUEEN != 0 {
                out.push('Q');
            }
            if self.castling_rights.0 & CastlingRights::BLACK_KING != 0 {
                out.push('k');
            }
            if self.castling_rights.0 & CastlingRights::BLACK_QUEEN != 0 {
                out.push('q');
            }
        }

        out.push(' ');
        match self.en_passant_target {
            Some(square) => out.push_str(&square.to_string()),
            None => out.push('-'),
        }
        out
    }
}

impl Board {
    /// The raw FEN en-passant square, retained only when some en-passant
    /// capture is actually legal in this position.
    pub fn effective_en_passant_square(&self) -> Option<Square> {
        let target = self.ep_square?;
        if self.legal_moves().iter().any(|mv| mv.is_en_passant()) {
            Some(target)
        } else {
            None
        }
    }

    /// Exact structural identity for the FIDE repetition rules.
    #[inline]
    pub fn repetition_key(&self) -> RepetitionKey {
        RepetitionKey::from_board(self)
    }

    /// Stable, cross-language identifier: canonical four-field effective EPD.
    #[inline]
    pub fn position_id(&self) -> String {
        self.repetition_key().position_id()
    }
}
