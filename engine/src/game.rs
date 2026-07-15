//! [`Game`]: a [`Board`] plus the move/position history needed for the
//! history-dependent draw rules, surfaced through [`Outcome`].
//!
//! `Board` alone answers everything about a single position (legal moves,
//! check, stalemate, insufficient material). Threefold/fivefold repetition and
//! the 50/75-move rules require history, which lives here.

use crate::board::{Board, Undo};
use crate::moves::Move;
use crate::repetition::RepetitionKey;
use crate::types::Color;

/// Why a game is drawn (other than stalemate, which is its own [`Outcome`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawReason {
    /// 50 moves by each side without a pawn move or capture (claimable).
    FiftyMove,
    /// 75 moves by each side without a pawn move or capture (automatic).
    SeventyFiveMove,
    /// The same position has occurred three times (claimable).
    ThreefoldRepetition,
    /// The same position has occurred five times (automatic).
    FivefoldRepetition,
    /// Neither side has the material to deliver checkmate.
    InsufficientMaterial,
}

/// The status of a game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The game is still in progress.
    Ongoing,
    /// `winner` delivered checkmate.
    Checkmate { winner: Color },
    /// The side to move has no legal move but is not in check.
    Stalemate,
    /// A draw for the given reason.
    Draw(DrawReason),
}

impl Outcome {
    #[inline]
    pub fn is_over(self) -> bool {
        !matches!(self, Outcome::Ongoing)
    }

    #[inline]
    pub fn is_draw(self) -> bool {
        matches!(self, Outcome::Stalemate | Outcome::Draw(_))
    }
}

/// A game: the current position plus its full history.
#[derive(Clone)]
pub struct Game {
    board: Board,
    /// Exact repetition keys, one per ply plus the initial position.
    keys: Vec<RepetitionKey>,
    /// Applied moves and their undo records, for [`Game::pop`].
    stack: Vec<(Move, Undo)>,
}

impl Game {
    /// A new game from the standard starting position.
    pub fn new() -> Game {
        Game::from_board(Board::startpos())
    }

    /// A game starting from an arbitrary position.
    pub fn from_board(board: Board) -> Game {
        let keys = vec![board.repetition_key()];
        Game {
            board,
            keys,
            stack: Vec::new(),
        }
    }

    /// Start from a FEN position.
    pub fn from_fen(fen: &str) -> Result<Game, crate::fen::FenError> {
        Ok(Game::from_board(Board::from_fen(fen)?))
    }

    #[inline]
    pub fn board(&self) -> &Board {
        &self.board
    }

    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.board.side_to_move()
    }

    /// Number of plies (half-moves) played.
    #[inline]
    pub fn ply(&self) -> usize {
        self.stack.len()
    }

    pub fn legal_moves(&self) -> crate::moves::MoveList {
        self.board.legal_moves()
    }

    /// Apply a move and record it in the history.
    pub fn push(&mut self, mv: Move) {
        let undo = self.board.make_move(mv);
        self.stack.push((mv, undo));
        self.keys.push(self.board.repetition_key());
    }

    /// Apply a move given in UCI notation; returns the move if legal.
    pub fn push_uci(&mut self, uci: &str) -> Option<Move> {
        let mv = self.board.parse_uci(uci)?;
        self.push(mv);
        Some(mv)
    }

    /// Apply a move given in SAN notation; returns the move if legal.
    pub fn push_san(&mut self, san: &str) -> Option<Move> {
        let mv = self.board.parse_san(san)?;
        self.push(mv);
        Some(mv)
    }

    /// Undo the most recent move, if any.
    pub fn pop(&mut self) -> Option<Move> {
        let (mv, undo) = self.stack.pop()?;
        self.board.unmake_move(mv, undo);
        self.keys.pop();
        Some(mv)
    }

    /// Exact repetition keys of every position so far, including the current
    /// one (one per ply plus the initial position). Useful for seeding an
    /// engine's repetition history during self-play.
    pub fn position_keys(&self) -> &[RepetitionKey] {
        &self.keys
    }

    /// How many times the current position has occurred (including now).
    pub fn repetition_count(&self) -> usize {
        let current = self.board.repetition_key();
        self.keys.iter().filter(|&&key| key == current).count()
    }

    /// Whether the side to move may *claim* a draw (threefold repetition or the
    /// 50-move rule).
    pub fn can_claim_draw(&self) -> bool {
        self.repetition_count() >= 3 || self.board.halfmove_clock() >= 100
    }

    /// The game's outcome.
    ///
    /// Automatic terminations (checkmate, stalemate, 75-move, fivefold
    /// repetition, insufficient material) are always reported. The *claimable*
    /// draws (threefold repetition, 50-move) are also reported here as draws,
    /// which matches how most engines treat them; use [`Game::can_claim_draw`]
    /// if you need to distinguish "claimable" from "automatic".
    pub fn outcome(&self) -> Outcome {
        if self.board.legal_moves().is_empty() {
            return if self.board.in_check() {
                Outcome::Checkmate {
                    winner: self.board.side_to_move().flip(),
                }
            } else {
                Outcome::Stalemate
            };
        }
        if self.board.is_insufficient_material() {
            return Outcome::Draw(DrawReason::InsufficientMaterial);
        }
        let rep = self.repetition_count();
        if rep >= 5 {
            return Outcome::Draw(DrawReason::FivefoldRepetition);
        }
        if self.board.halfmove_clock() >= 150 {
            return Outcome::Draw(DrawReason::SeventyFiveMove);
        }
        if rep >= 3 {
            return Outcome::Draw(DrawReason::ThreefoldRepetition);
        }
        if self.board.halfmove_clock() >= 100 {
            return Outcome::Draw(DrawReason::FiftyMove);
        }
        Outcome::Ongoing
    }

    #[inline]
    pub fn is_checkmate(&self) -> bool {
        matches!(self.outcome(), Outcome::Checkmate { .. })
    }

    #[inline]
    pub fn is_stalemate(&self) -> bool {
        matches!(self.outcome(), Outcome::Stalemate)
    }
}

impl Default for Game {
    fn default() -> Self {
        Game::new()
    }
}
