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
