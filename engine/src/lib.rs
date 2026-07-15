//! A compact, fast, fully-legal chess library.
//!
//! # Design
//!
//! Positions have two representations:
//!  * [`Board`] — the *working* form: per-piece-type and per-color
//!    [`Bitboard`]s plus a byte mailbox, an incremental Polyglot-compatible
//!    Zobrist [`Board::hash`], and make/unmake. This is what move generation
//!    runs on.
//!  * [`Packed`] — the *canonical* form: 34 bytes (a 32-byte nibble board plus
//!    two state bytes). Half the size of raw bitboards with O(1) random access,
//!    so large batches of positions stay cache-dense. Convert with
//!    [`Board::pack`] / [`Packed::unpack`].
//!
//! # Rules
//!
//! Fully-legal move generation ([`Board::legal_moves`]) with castling, en
//! passant, and promotion; check / checkmate / stalemate; and — via [`Game`] —
//! the 50/75-move rules, threefold/fivefold repetition, and insufficient
//! material, surfaced as an [`Outcome`].
//!
//! # Interop
//!
//! FEN ([`Board::from_fen`] / [`Board::to_fen`]), UCI and SAN moves, and
//! Polyglot Zobrist hashing — each validated against downloaded reference data
//! in the integration tests.
//!
//! # Example
//!
//! ```
//! use chess::{Board, Game, Outcome, Color};
//!
//! // Parse a position, generate legal moves, read its Polyglot hash.
//! let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
//! assert_eq!(board.legal_moves().len(), 20);
//! assert_eq!(board.hash(), 0x463b96181691fc9c);
//!
//! // Pack to 34 bytes and back without loss (of placement/side/castling/ep).
//! let packed = board.pack();
//! assert_eq!(core::mem::size_of_val(&packed), 34);
//! assert_eq!(board.hash(), packed.unpack().hash());
//!
//! // Play a game in SAN and detect the result.
//! let mut game = Game::new();
//! for mv in ["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7"] {
//!     game.push_san(mv).expect("legal move");
//! }
//! assert_eq!(game.outcome(), Outcome::Checkmate { winner: Color::White });
//! ```

pub mod attacks;
pub mod bitboard;
pub mod board;
pub mod eval;
pub mod fen;
pub mod game;
pub mod magic;
pub mod mcts;
pub mod movegen;
pub mod moves;
pub mod packed;
pub mod san;
pub mod search;
pub mod tt;
pub mod types;
pub mod uci;
pub mod zobrist;
mod zobrist_table;

pub use bitboard::Bitboard;
pub use board::{Board, NullUndo, Undo};
pub use eval::{Evaluator, HandcraftedEval, Nnue, NnueEval, PolicyValueNet};
pub use fen::FenError;
pub use game::{DrawReason, Game, Outcome};
pub use mcts::{Guide, Mcts, RemoteGuide};
pub use moves::{Move, MoveFlag, MoveList};
pub use packed::Packed;
pub use search::{Analysis, Engine, Limits, SearchInfo};
pub use types::{
    CastlingRights, CastlingSide, Color, File, Piece, PieceType, Rank, Square, squares,
};

#[cfg(test)]
mod _size_probe {
    #[test]
    fn print_sizes() {
        eprintln!("Board = {}", core::mem::size_of::<crate::board::Board>());
        eprintln!("Undo  = {}", core::mem::size_of::<crate::board::Undo>());
        eprintln!("Packed= {}", core::mem::size_of::<crate::packed::Packed>());
        eprintln!("Option<Square> = {}", core::mem::size_of::<Option<crate::types::Square>>());
        eprintln!("Move  = {}", core::mem::size_of::<crate::moves::Move>());
    }
}
