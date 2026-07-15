# Certified chess knowledge beyond openings

This branch compares opening-theory research with four routes to genuinely new,
player-useful chess theorems. The target is not merely a verified engine answer.
A successful program must end in a compact claim that a player can understand,
test at the board, and reuse in calculation.

The recommendation is to start with **tablebase counterexample-and-repair**,
build **bounded tactical certificates** as shared infrastructure, and keep
**opening route dominance** as the highest-novelty parallel bet. Generic
tablebase explanation is prior art: KRK and KBNK rule extraction already exist
[bain1994][bain1994] [guid2010][guid2010].

## Questions

- Can a tactical solver emit a small proof object that Lean checks independently?
- Can tablebases yield interpretable rules rather than isolated perfect moves?
- Which familiar rules admit instructive, minimal counterexamples?
- Which reduced-material state spaces are small enough for exhaustive Lean-backed
  classification?
- Does any of these routes dominate opening work on novelty, usefulness, formal
  leverage, data access, and speed of falsification?

## Investigations

- [Primary source index](sources.md): original papers, official rules, project
  documentation, and local code artifacts with one-line relevance notes.
- [Comparative ranking](route-comparison.md): common rubric, candidate ranking,
  and the smallest experiment that could falsify each program.
- [Proof-producing tactical certificates](tactical-certificates.md): bounded
  forced claims, AND/OR proof trees, independent checking, and human-readable
  tactical lemmas.
- [Tablebase mining and folklore counterexamples](tablebase-rule-mining.md):
  retrograde-analysis evidence, rule extraction, clock-sensitive semantics, and
  minimal exceptions to familiar endgame advice.
- [Small-material exhaustive classification](small-material-classification.md):
  finite domains in which a complete classification can become an interpretable
  theorem rather than a lookup table.
- [Theorem-to-trainer integration](training-integration.md): versioned rule and
  exception cards, certificate-backed exercises, spaced review, and the bridge
  to the monorepo's `engine/` crate.

## Planned pilots

1. Use the formal effective-en-passant repetition key to test the Rust engine's
   Polyglot-based repetition workflow. The checked counterexample shows equal
   FIDE keys but unequal Polyglot hashes, exposing a concrete undercount risk and
   producing an exception drill immediately.
2. Specify and check a bounded mate certificate whose opponent nodes enumerate
   every legal reply; deletion of one defense must make checking fail.
3. Use the geometric pawn-square theorem as a non-novel control for tablebase
   counterexample mining and compact rule repair.
4. Exhaustively classify a symmetry-reduced KPKP corridor only if the control
   produces a short exact predicate rather than a lookup-sized exception list.

## Supplementary code

- [`rank_candidates.py`](data/rank_candidates.py) validates the 1–5 scoring
  rubric and reproduces several weighting profiles from
  [`candidate_scores.csv`](data/candidate_scores.csv); the checked report is
  [`ranking-output.txt`](data/ranking-output.txt).
- [`state_space_bounds.py`](data/state_space_bounds.py) checks the crude labeled
  placement ceilings used to scope exhaustive pilots; its output is
  [`state-space-output.txt`](data/state-space-output.txt).
- [`repetition_ep_counterexample.py`](data/repetition_ep_counterexample.py)
  independently compares FIDE-effective and Polyglot en-passant identity using
  pinned `chess==1.11.2`; its checked output is
  [`repetition-ep-output.txt`](data/repetition-ep-output.txt).
- [`check_corpus.py`](data/check_corpus.py) checks local links, reachability from
  this index, inline citation definitions, and full local bibliographies.

## Result

The research branch is complete at pilot-design level. The strongest completed
worked example is the Polyglot-versus-FIDE repetition mismatch: it connects a
proved formal identity, an independent executable counterexample, a real engine
integration hazard, and a compact human rule/exception card. No external engine
repository was modified.

## Local References

- **bain1994** — Michael Bain and Stephen Muggleton, “Learning Optimal Chess Strategies,” in *Machine Intelligence 13: Machine Intelligence and Inductive Learning*, Oxford University Press, 1994, 291–309. DOI 10.1093/oso/9780198538509.003.0012.
- **guid2010** — Matej Guid, Martin Možina, Aleksander Sadikov, and Ivan Bratko, “Deriving Concepts and Strategies from Chess Tablebases,” in *Advances in Computer Games*, LNCS 6048, Springer, 2010, 195–207. DOI 10.1007/978-3-642-12993-3_18.

[bain1994]: https://doi.org/10.1093/oso/9780198538509.003.0012
[guid2010]: https://doi.org/10.1007/978-3-642-12993-3_18
