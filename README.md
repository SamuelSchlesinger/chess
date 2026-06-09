# chess

A compact, fast, fully-legal chess library in Rust.

Positions are stored in **34 bytes** yet move generation runs at **~450–640 million
nodes/sec** (perft), because the crate keeps two representations and converts
between them:

| Form | Size | Role |
|------|------|------|
| [`Packed`] | **34 B** | canonical storage — 32-byte nibble board + 2 state bytes; O(1) random access, half the size of raw bitboards, cache-dense for processing many positions at once |
| [`Board`] | 144 B | working form — per-piece-type & per-color bitboards + a byte mailbox + an incremental Zobrist key; what move generation runs on |

`board.pack()` / `packed.unpack()` move between them.

## Features

- **Fully-legal move generation** — a pin-aware generator (check masks, pin rays,
  king-danger map) with exact en-passant legality, castling, and promotions.
- **Full rules & outcomes** — check / checkmate / stalemate, the 50- and 75-move
  rules, threefold / fivefold repetition, and insufficient material, surfaced as
  an [`Outcome`] via [`Game`].
- **Interop** — FEN, UCI and SAN moves, and **Polyglot-compatible** Zobrist
  hashing (bit-for-bit identical to the published book format).
- **Fast sliding attacks** — magic bitboards (a single multiply-shift-load),
  with the classical ray scan kept for cross-checking and benchmarking.

## Example

```rust
use chess::{Board, Game, Outcome, Color};

// Parse, generate moves, hash.
let board = Board::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")?;
assert_eq!(board.legal_moves().len(), 20);
assert_eq!(board.hash(), 0x463b96181691fc9c);     // Polyglot start key

// Pack to 34 bytes and back.
let packed = board.pack();
assert_eq!(core::mem::size_of_val(&packed), 34);
let board2 = packed.unpack();
assert_eq!(board.hash(), board2.hash());

// Play a game with SAN; detect the result.
let mut game = Game::new();
for mv in ["e4", "e5", "Bc4", "Nc6", "Qh5", "Nf6", "Qxf7"] {
    game.push_san(mv).expect("legal");
}
assert_eq!(game.outcome(), Outcome::Checkmate { winner: Color::White });
# Ok::<(), chess::FenError>(())
```

## Correctness

Every operation is validated against **publicly-downloaded datasets** (see
`tests/data/`), not just hand-written cases:

| Operation | Oracle | Coverage |
|-----------|--------|----------|
| Move generation | perft suites (vajolet 6 838 positions; the 6 CPW landmarks incl. Kiwipete) | all 6 838 to depth 3; landmarks to depth 6 (119 M / 193 M nodes), exact |
| Legal generator | differential vs. a make/unmake reference | identical move *sets* on pin/check/ep/castle/promo trees |
| Zobrist | Polyglot reference keys | all 9 reference positions (incl. ep & castle-forfeit), incremental == recomputed |
| FEN | the 6 838-position suite | parse → serialize → parse round-trips |
| SAN | real PGN games (Kasparov–Deep Blue, etc.) | 707 half-moves parsed, played, and SAN-round-tripped |
| Packed | the 6 838-position suite | lossless round-trip + random-access agreement |
| Draw rules | constructed positions | stalemate, threefold, 50/75-move, insufficient material |

```sh
cargo test --release                 # fast suite
cargo test --release -- --ignored    # deep perft (full suite to depth 5, landmarks to depth 6)
```

## Benchmarks

```sh
cargo bench                  # all groups
cargo bench -- attacks       # one group
```

See [PERFORMANCE.md](PERFORMANCE.md) for the empirical comparison of board-op
alternatives (classical rays vs. magic bitboards; make/unmake filter vs.
pin-aware legal generation) and how the current design was chosen.

[`Board`]: crate::Board
[`Packed`]: crate::Packed
[`Game`]: crate::Game
[`Outcome`]: crate::Outcome
