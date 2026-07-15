# A research program for novel, useful chess theory

This corpus asks a deliberately demanding question: where can exact state
modeling, formal proof, and reproducible computation produce knowledge that is
both plausibly new and useful to serious chess players?

The target is not novelty-by-formalization. A result should make a substantive
claim about chess, opening preparation, or humanly learnable strategy that
could be false before we investigate it.

## Evaluation criteria

Every proposed program will be scored against six criteria:

1. **Chess content:** the output says something about chess rather than only
   about software architecture or proof engineering.
2. **Player value:** a player can change preparation, calculation, or study
   behavior because of the result.
3. **Novelty case:** scoped primary-source research identifies what is already
   known and isolates the remaining claim.
4. **Formal leverage:** Lean can certify a meaningful part of the result,
   instead of merely replaying an external computation.
5. **Empirical leverage:** available game, opening, engine, or tablebase data
   can falsify and refine the conjecture.
6. **Executable first experiment:** we can obtain a discriminating result in a
   small checkpoint before committing to a large formalization.

## Parallel investigations

- [Transposition algebra](transposition-algebra/index.md): classify the
  relations between move histories beyond independent-move commutation, and
  ask whether early opening theory admits a useful finite presentation.
- [Opening decisions](opening-decisions/index.md): formalize move-order risk,
  commitment, repertoire compression, and transposition leverage in terms a
  player can act on.
- [Certified chess knowledge](certified-chess-knowledge/index.md): compare
  opening work with proof-producing tactics, reduced-material classification,
  and the extraction of human theorems from exhaustive state spaces.

## Synthesis questions

The final synthesis will answer:

- Which candidate has the strongest novelty case?
- Which candidate can yield a player-facing result rather than a database
  curiosity?
- What is the smallest experiment that can kill or validate the idea?
- Exactly which claims would be proved in Lean, checked by an external
  computation, or left as empirical chess judgments?
- What dataset and licensing commitments are required?

## Provisional hypotheses

The most promising opening hypothesis is that **move-order value is a property
of the route, not merely its endpoint**. Two routes may transpose eventually
while exposing different opponent deviations along the way. This suggests a
dominance order on routes based on their adversarial deviation languages, and
could turn the familiar advice “this move order avoids X” into an exact,
computable statement.

A second hypothesis is that transpositions have several algebraic mechanisms:
commuting independent moves, route substitution, and reversible detours. If a
small set of such relations generates most curated opening transpositions, it
could provide both a new structural description and a compressed way to teach
move orders.

## Supplementary code

- [Transposition classifier](transposition-algebra/data/classify_transpositions.py)
  computes the rooted graph basis, classifies its 205 chord relations, and
  checks all pinned aggregates.
- [Opening-decision pilot](opening-decisions/data/pilot.py) measures history-
  versus-position study units and route-sensitive deviation sets.
- [Certified-knowledge checks](certified-chess-knowledge/data/check_corpus.py)
  reproduce the route ranking, state-space bounds, citations, and the
  effective-en-passant/Polyglot counterexample.

## Phase-one findings

The first investigations have already narrowed the program substantially:

- The 8,646 distinct histories in the named-opening corpus project to 7,848
  exact repetition positions. A cardinality-minimal rooted path presentation
  needs 205 chord equations; 377 of the 570 non-singleton history fibres merely
  propagate earlier merges rather than introduce a fresh transposition.
- Position-keyed opening cards remove 11.32% of White decision-history units
  and 11.80% of Black units in the pinned taxonomy. Route prefixes must still
  be retained because hundreds of routes to a common endpoint expose different
  pre-transposition deviations.
- The strongest near-term theorem-mining route is precise heuristic repair:
  use exhaustive state spaces to find minimal counterexamples, then prove a
  short corrected rule. Bounded tactical certificates are useful checking
  infrastructure but are not, by themselves, a novel chess result.
- The formal repetition semantics exposed a concrete integration defect: a
  legal-position pair can have the same FIDE repetition identity but distinct
  Polyglot hashes when en passant is geometrically available but king-illegal.
  The Rust engine currently reuses that Polyglot hash for repetition counting.

These are structural and system findings, not yet evidence that one opening is
best or that a particular repertoire improves rating. The next phase tests the
claims by independent review and then on rating-matched games, pinned engine
runs, and delayed human transfer.

## Status

Phase 1 authoring and reproducible pilots are complete. Each investigation has
its own primary-source ledger. Independent review and revision are next; no
personal repertoire recommendation is frozen before that evidence converges.
