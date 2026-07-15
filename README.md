# Chess in Lean

This project formalizes orthodox chess in Lean 4, validates its executable rules
against established chess data, and proves accurate theorems useful to real
players. `Position` contains every field needed to interpret the next move;
`GameState` retains the complete position history needed for repetition; and
`Game` adds FIDE conclusions such as checkmate, claims, automatic draws,
resignation, and time forfeits.

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

## Validation

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
and exact versus repetition-only opening transpositions. A separately pinned
CC0 Lichess corpus adds 3,803 named opening lines and 36,840 plies. Lean checks
every UCI move for legality, resolves every SAN token in its evolving position,
requires SAN and UCI to denote the same move at every ply, and checks every
effective EPD endpoint. The same run analyzes all 40,643 row-prefix
occurrences, deduplicates their move histories, and checks the exact
transposition-fibre distribution. Source revisions, hashes, licenses,
reproduction details, and the empirical graph report are recorded in
[`data/PROVENANCE.md`](data/PROVENANCE.md).

If Stockfish 18 is installed, an optional external cross-check is available:

```text
python3 scripts/verify_stockfish.py --stockfish stockfish
```

Stockfish is not part of the theorem trust base. The script deliberately checks
only perft, move membership, trace legality, and effective FEN endpoints—not
raw en-passant history, repetition, draw-claim semantics, or phase theorems.
