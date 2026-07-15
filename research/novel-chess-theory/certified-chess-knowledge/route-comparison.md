# Comparative ranking and kill tests

## Recommendation

Run **tablebase counterexample-and-repair** first, while building **bounded
tactical certificates** as reusable infrastructure.  Keep **opening route
dominance** as the high-upside parallel program.  Do not fund generic
“explain a tablebase” work: interpretable rule induction from KRK and KBNK has
been demonstrated for decades [bain1994][bain1994] [guid2010][guid2010], and a
2025 study transferred machine-discovered chess concepts to elite grandmasters
[schut2025][schut2025].

The sharper hypothesis is falsifiable: a narrowly stated human heuristic has a
small set of structural exceptions; exhaustive labels can expose the minimal
exceptions; and the repaired predicate is short enough to prove and train.
This route leads directly to the rule/exception/exercise pipeline described in
[the trainer integration](training-integration.md).

## Common rubric

Scores are ordinal research judgments from 1 (weak) to 5 (strong), not measured
facts.  “Falsifying pilot” scores speed and decisiveness: a high score means a
small experiment can kill the idea.

| Route | Novelty | Player usefulness | Lean leverage | Data access | Falsifying pilot | Total / 25 |
|---|---:|---:|---:|---:|---:|---:|
| Tablebase counterexample and repair | 4 | 5 | 5 | 5 | 5 | **24** |
| Small-material exact classification | 3 | 4 | 5 | 5 | 4 | **21** |
| Opening route dominance | 5 | 5 | 3 | 4 | 3 | **20** |
| Bounded tactical certificates | 2 | 4 | 5 | 4 | 5 | **20** |
| Generic tablebase rule extraction | 1 | 4 | 3 | 5 | 3 | **16** |

The data and arithmetic are checked by
[`rank_candidates.py`](data/rank_candidates.py).  The checked
[sensitivity report](data/ranking-output.txt) reweights player value, novelty,
and Lean leverage; counterexample-and-repair remains first in every declared
profile.  These totals organize judgment rather than settle it.  In particular,
opening route dominance is the strongest claim of substantive novelty, and its
existing trainer makes deployment unusually direct.

## Why the routes score this way

### Tablebase counterexample and repair

Perfect WDL/DTZ labels, legal moves, and current seven-piece coverage are
available through the Lichess tablebase API [lila-tablebase][lila-tablebase].
Lean can state the heuristic and its repaired boundary against this repository's
rules.  A single counterexample can reject a proposed universal rule, so the
feedback loop is fast.  Novelty survives only if the final result is a new,
specific theorem rather than “machine learning finds endgame patterns.”

### Opening route dominance

The sibling [opening investigation](../opening-decisions/index.md) asks whether
one move order exposes a subset of the opponent deviations exposed by another.
That is new-looking, player-facing, and compatible with the current prefix-based
opening trainer.  Its checked taxonomy pilot finds 350 White and 366 Black
history-card reductions after exact position merging, plus 270 White-controlled
and 265 Black-controlled same-target route pairs
([checked output](../opening-decisions/data/output.txt)).  This establishes
route-sensitive structure, not practical dominance: game frequency, opponent
quality, and engine evaluation are still needed before a formal language
inclusion becomes repertoire advice.

### Bounded tactical certificates

Proof-number search was created to establish game-theoretic values in irregular
AND/OR trees [allis1994][allis1994].  A small checker is excellent Lean work and
can make puzzle answers independently trustworthy.  The certificate mechanism
itself is not new; novelty must come from a distilled tactical theorem or a new
minimal family, not from replaying a principal variation.

### Small-material exact classification

Finite classification offers strong completeness and abundant data, but standard
material classes are crowded.  Four-piece pawnless endgames have already been
formally generated in HOL4 [hurd2005][hurd2005], and a complete executable KRK
strategy was proved in Isabelle/HOL [maric2015][maric2015].  A viable project
must select a motif-level family and produce a compact predicate.

### Generic rule extraction

The data are excellent and the output could teach players, but the novelty case
is weakest.  Prior work learned fixed-depth KRK rules and semi-automatically
derived a KBNK teaching strategy [bain1994][bain1994]
[guid2010][guid2010].  Treat those papers as baselines to beat, not as an open
problem statement.

## Smallest falsifying pilots

| Route | First pilot | Kill condition |
|---|---|---|
| Counterexample and repair | Use the already formalized geometric pawn-square predicate as a plumbing control; enumerate a KPK slice, order mismatches canonically, and see whether familiar exception features repair it. | The loop cannot rediscover a short known repair, or the publishable family needs a lookup-sized exception list. |
| Opening route dominance | On a pinned game sample, compare transposing routes by opponent deviation language and downstream evaluation, stratified by rating and time control. | Apparent dominance disappears out of the opening-name catalog or changes sign under reasonable sampling choices. |
| Tactical certificates | Check a mate-in-two with at least two legal defenses; delete one defense and require rejection. | The checker accepts an incomplete defense set, cannot state draw semantics, or certificates become too large for training artifacts. |
| Small-material classification | Enumerate a symmetry-reduced KPKP corridor and classify placements whose WDL flips with side to move. | The result is merely a table, duplicates published classifications, or no short feature predicate predicts the labels exactly. |
| Generic extraction | Reproduce one published KRK/KBNK baseline before attempting a new class. | The reproduction is less compact, less correct, or less teachable than the prior result. |

The KPK control is deliberately not a novelty claim.  Its purpose is to test the
whole pipeline cheaply: legal-state generation, metric semantics, deterministic
minimal counterexamples, conjecture repair, Lean statement, and exception-card
export.

## Decision gates

Advance a route only when all of these are true:

1. the target proposition names its game semantics and metric;
2. a primary-source search isolates the claim from prior endgame and formal-
   chess work;
3. a deterministic script can reproduce the smallest counterexample or the
   exhaustive count;
4. the result compresses into a rule plus a small exception family;
5. an unseen-position exercise tests transfer rather than diagram memory.

The strongest immediate end-to-end demonstration is already present: the
formal FIDE repetition key exposes a concrete Polyglot-key mismatch in the Rust
game/search workflow, and the same counterexample becomes a compact rules drill.
That example validates the research method even before a novel chess theorem is
found.

## Local References

- **bain1994** — Michael Bain and Stephen Muggleton, “Learning Optimal Chess Strategies,” in *Machine Intelligence 13: Machine Intelligence and Inductive Learning*, Oxford University Press, 1994, 291–309. DOI 10.1093/oso/9780198538509.003.0012.
- **guid2010** — Matej Guid, Martin Možina, Aleksander Sadikov, and Ivan Bratko, “Deriving Concepts and Strategies from Chess Tablebases,” in *Advances in Computer Games*, LNCS 6048, Springer, 2010, 195–207. DOI 10.1007/978-3-642-12993-3_18.
- **schut2025** — Lisa Schut, Nenad Tomašev, Thomas McGrath, Demis Hassabis, Ulrich Paquet, and Been Kim, “Bridging the Human–AI Knowledge Gap through Concept Discovery and Transfer in AlphaZero,” *Proceedings of the National Academy of Sciences* 122(13), 2025, e2406675122. DOI 10.1073/pnas.2406675122.
- **lila-tablebase** — Lichess, `lila-tablebase` README and HTTP API documentation, GitHub repository, current snapshot inspected 14 July 2026.
- **allis1994** — L. Victor Allis, Maarten van der Meulen, and H. Jaap van den Herik, “Proof-Number Search,” *Artificial Intelligence* 66(1), 1994, 91–124. DOI 10.1016/0004-3702(94)90004-3.
- **hurd2005** — Joe Hurd, “Formal Verification of Chess Endgame Databases,” in *Theorem Proving in Higher Order Logics: Emerging Trends Proceedings*, Oxford University Computing Laboratory, 2005, 85–100.
- **maric2015** — Filip Marić, Predrag Janičić, and Marko Maliković, “Proving Correctness of a KRK Chess Endgame Strategy by Using Isabelle/HOL and Z3,” in *Automated Deduction—CADE-25*, LNCS 9195, Springer, 2015, 256–271. DOI 10.1007/978-3-319-21401-6_17.

[bain1994]: https://doi.org/10.1093/oso/9780198538509.003.0012
[guid2010]: https://doi.org/10.1007/978-3-642-12993-3_18
[schut2025]: https://doi.org/10.1073/pnas.2406675122
[lila-tablebase]: https://github.com/lichess-org/lila-tablebase/blob/main/README.md
[allis1994]: https://doi.org/10.1016/0004-3702(94)90004-3
[hurd2005]: https://www.gilith.com/papers/chess.pdf
[maric2015]: https://doi.org/10.1007/978-3-319-21401-6_17
