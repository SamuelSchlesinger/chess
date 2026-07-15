# Chess in Lean

This project formalizes orthodox chess in Lean 4, validates its executable rules
against established chess test data, and proves accurate theorems useful to real
players.

The first theory campaign will classify king-and-pawn-versus-king endings and
derive precise versions of opposition, key-square, rule-of-the-square, and
rook-pawn principles from that classification.

The current structural results include exact king distance, a tempo-correct
geometric pawn-square theorem, constructive opposition lemmas, a general
two-deadline Réti pivot theorem, and an irreversible phase grading whose cycle
theorems confine every repetition-returning edge to a quiet kernel of non-pawn,
non-capturing, castling-right-preserving moves. A legal-line trace algebra
separately formalizes exact and repetition-node opening transpositions, with a
proved quotient move graph and equality of their residual legal languages. See
[THEORY.md](THEORY.md).

## Validation

The project has checked UCI parsing, checked raw and effective FEN rendering,
and legality-checked `GameState` replay. Successful replay is proved to produce
a path in the legal position graph; it is not merely a parser that updates
boards.

Run the Lean development and pinned TSV corpus with:

```text
lake build
lake exe chess_validate
```

The corpus exercises perft, individual move legality, complete traces,
history-sensitive repetition and draw thresholds, phase monotonicity, and exact
versus repetition-only opening transpositions. Source revisions, hashes,
licenses, and the planned CC0 Lichess opening expansion are recorded in
[`data/PROVENANCE.md`](data/PROVENANCE.md).

If Stockfish 18 is installed, an optional external cross-check is available:

```text
python3 scripts/verify_stockfish.py --stockfish stockfish
```

Stockfish is not part of the theorem trust base. The script deliberately checks
only perft, move membership, trace legality, and effective FEN endpoints—not
raw en-passant history, repetition, draw-claim semantics, or phase theorems.
