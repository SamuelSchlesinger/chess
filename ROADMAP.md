# Roadmap

The destination is a substantial, evidence-backed chess project that helps one
player improve toward 2000-level practical strength. Rating improvement cannot
be certified, so every stage has an executable product gate and a human outcome
gate.

## 1. Semantic convergence

- Give Lean and Rust the same canonical effective four-field EPD `PositionId`.
- Keep Polyglot/Zobrist hashing for caches and books, but use exact FIDE identity
  for repetition, cards, and persistent graph keys.
- Differentially replay shared castling, en-passant, promotion, clock, and
  transposition fixtures through both implementations.

Gate: every shared fixture has identical position identity and legal moves in
Lean and Rust; the pinned illegal-en-passant repetition counterexample passes.

Current status: structural FIDE repetition identity, the full legal-history
counterexample, shared perft positions, selected move legality, trace endpoints,
and opening-pair projections are implemented in both stacks. The gate remains
open because the fixture runner does not yet compare the complete legal-move set
from every shared position across Lean and Rust.

## 2. Player baseline and concise guide

- Import the player's recent rapid/classical games and establish a reproducible
  error profile: tactical losses, time use, opening exits, conversion, and
  endgame gaps.
- Reduce the guide to one thinking protocol, a small evaluation vocabulary, and
  explicit exception cards rather than a long list of slogans.
- Keep certified, measured, and pedagogical claims visibly distinct.

Gate: the player can execute the protocol under time pressure, and held-out
games show which rules transfer rather than merely being recalled.

## 3. Repertoire as a robust graph

- Compare candidate repertoires on soundness, encounter coverage, surprise
  severity, unique position decisions, route exceptions, and player enjoyment.
- Analyze rating- and time-control-matched games with pinned Stockfish settings.
- Preserve opponent-route provenance while merging exact transpositions.

Gate: freeze a compact White repertoire and Black answers only after held-out
coverage and engine checks; publish every choice with its plan and stop rule.

## 4. Structured repetition

- Replace deterministic main-line playback with due position, deviation,
  transposition-transfer, concept, tactical, and exact-endgame cards.
- Persist content-versioned review state and import mistakes from the player's
  own games.
- Optimize delayed first-attempt accuracy and unseen transfer, not engine-match
  percentage or session XP.

Gate: progress survives restart, transposed routes share position mastery,
route-specific dangers remain separate, and delayed transfer improves against a
sequence-only control.

## 5. New chess theory

- Test adversarial move-order dominance and transposition-adjusted repertoire
  complexity on real game samples.
- Mine tablebases for minimal counterexamples to precisely stated heuristics,
  then prove short repaired rules in Lean.
- Export bounded tactical certificates and theorem-backed exception drills.

Gate: retain a result as a contribution only when it is novel after primary-
source review, reproducible, shorter than its exception table, and useful on
unseen positions.
