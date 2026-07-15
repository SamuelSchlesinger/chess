# Chess: proofs, engines, and human training

This monorepo combines an executable Lean 4 formalization of orthodox chess, a
fast Rust engine and analysis UI, reproducible chess-theory research, and a
personal training system. Its organizing rule is simple: proofs establish exact
meaning, engines and corpora provide measured evidence, and pedagogy turns both
into concise human practice without confusing one kind of claim for another.

| Path | Role |
|---|---|
| [`Chess/`](Chess/) | FIDE state, legal moves, game conclusions, exact keys, and proofs |
| [`engine/`](engine/) | Rust move generation, search, UCI engines, analysis board, and trainer |
| [`data/`](data/) | Pinned public corpora, hashes, licenses, and validation fixtures |
| [`research/`](research/) | Cited theory investigations, pilots, limitations, and checked outputs |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Cross-layer identities, schemas, trust boundaries, and data policy |

The Rust history was imported intact under `engine/`; its former checkout at
`~/projects/games/chess` is no longer a second maintained implementation. Large
game dumps, nets, tablebases, caches, and build products remain external and
content-addressed rather than being copied into Git.

## Start here

Run the complete local validation chain from the repository root:

```text
scripts/check_all.sh
```

Or run one layer directly:

```text
lake build
lake exe chess_validate
cargo test --manifest-path engine/Cargo.toml
cargo run --release --manifest-path engine/Cargo.toml --bin chess-web
cargo run --release --manifest-path engine/Cargo.toml --bin chess-trainer
scripts/run_personal_trainer.sh
```

The browser trainer now has two layers. Free play retains the imported warm-UCI
grading and short, sequence-based opening repetitions. Private diagnostic
review loads replay-checked positions selected from the player's games, hides
the engine reference until the player commits a move and reason, and persists
both the answer release and self-grade in an append-only local log. If the
process stops after revealing, the same answer is restored and must still be
graded. The initial private pilot has six cards and a deliberately simple fixed
schedule; it is a training instrument, not yet the transposition-aware
repertoire graph or evidence of rating gain.
See the [roadmap](ROADMAP.md) and the
[research synthesis](research/novel-chess-theory/index.md).

Personal Chess.com exports can be validated and profiled locally with
`scripts/player_games.py`. Raw exports remain untracked wherever they are
stored; generated profiles are accepted by default only under the Git-ignored
`data/private/`. The privacy and assurance boundary is documented in
[player data](docs/PLAYER_DATA.md).

## Formal theory

`Position` contains every field needed to interpret the next move; `GameState`
retains the complete position history needed for repetition; and `Game` adds
FIDE conclusions such as checkmate, claims, automatic draws, resignation, and
time forfeits.

The first theory campaign will classify king-and-pawn-versus-king endings and
derive precise versions of opposition, key-square, rule-of-the-square, and
rook-pawn principles from that classification.

The current structural results include exact king distance, a tempo-correct
geometric pawn-square theorem, constructive opposition lemmas, a general
two-deadline Réti pivot theorem, and an irreversible phase grading whose cycle
theorems confine every repetition-returning edge to a quiet kernel of non-pawn,
non-capturing, castling-right-preserving moves. A legal-line trace algebra
separately formalizes exact and repetition-node opening transpositions, with a
proved quotient move graph and equality of their residual legal languages.

The opening-database layer now proves a useful design principle. Legal move
words form a prefix trie, while transposition classes form a graph quotient.
An observable can be stored unambiguously on a transposition node exactly when
it is invariant under move order. Legal continuations, perft, and static
evaluations deliberately defined only from repetition state qualify;
clock-aware evaluations, ply number, opening labels, and occurrence records
generally do not.
This is why a sound opening explorer must retain history-level edges alongside
position-level nodes rather than overwriting one move order with another. See
[THEORY.md](THEORY.md).

That specification now has an executable exact key and a finite pushforward
layer. The pinned opening trie contains 8,646 distinct move histories but only
7,848 repetition nodes: 570 nodes have multiple observed histories, with up to
eight move orders in one fibre. Lean recomputes those figures from all prefixes
in CI. It also proves that projected continuation records remain legal graph
edges and that corpus weights add over fibres without forcing occurrence names
or counts to become intrinsic properties of a position.

## Lean validation and datasets

The project has checked UCI parsing, canonical position-dependent SAN parsing
and rendering, checked raw and effective FEN rendering, and legality-checked
`GameState` replay. Successful UCI and SAN replay are each proved to produce a
path in the legal position graph; they are not merely parsers that update
boards.

Run the Lean development and pinned TSV corpus with:

```text
lake build
lake exe chess_validate
```

The small regression corpus exercises perft, individual move legality, complete
traces, history-sensitive repetition and draw thresholds, phase monotonicity,
exact versus repetition-only opening transpositions, and a shared nine-case
`PositionId` contract that Lean and Rust both execute. A separately pinned
CC0 Lichess corpus adds 3,803 named opening lines and 36,840 plies. Lean checks
every UCI move for legality, resolves every SAN token in its evolving position,
requires SAN and UCI to denote the same move at every ply, and checks every
effective EPD endpoint. The same run analyzes all 40,643 row-prefix
occurrences, deduplicates their move histories, and checks the exact
transposition-fibre distribution. Source revisions, hashes, licenses,
reproduction details, and the empirical graph report are recorded in
[`data/PROVENANCE.md`](data/PROVENANCE.md).

Rust's `monorepo_fixtures` integration test consumes these same root
`perft.tsv`, `moves.tsv`, `traces.tsv`, and `opening_pairs.tsv` files. It checks
move-generation counts, individual legality, full trace endpoints and draw
thresholds, and exact versus repetition-only transpositions against the Rust
runtime rather than maintaining a second copy of those cases.

Rust deliberately separates its Polyglot-compatible `Board::hash()` from the
structural FIDE `RepetitionKey`. A reachable-from-start regression includes a
pinned en-passant occurrence whose Polyglot key differs after the target
expires; the exact game history still recognizes the resulting threefold
position, and search tests exercise its distinct one-ancestor draw heuristic.

If Stockfish 18 is installed, an optional external cross-check is available:

```text
python3 scripts/verify_stockfish.py --stockfish stockfish
```

Stockfish is not part of the theorem trust base. The script deliberately checks
only perft, move membership, trace legality, and effective FEN endpoints—not
raw en-passant history, repetition, draw-claim semantics, or phase theorems.
