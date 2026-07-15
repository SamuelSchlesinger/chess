# Opening decisions: from move trees to useful preparation

The strongest research direction is not to prove that one opening move is
universally best. It is to make a player's *preparation problem* exact:

> Choose a small, sound policy through the opening graph that covers the moves
> this player is likely to face, remains safe against plausible surprises, and
> reuses knowledge whenever routes transpose.

That formulation can produce genuinely player-facing outputs: “this move order
removes these three dangerous replies,” “these 14 lines are really six decision
positions,” “learn this hub first because it is reached by 18% of your games,”
and “this is the move where you irrevocably allow the Benoni.” The exact legal
graph and its quotient laws belong in Lean; frequencies, engine assessments,
and human recall remain empirical.

## Documents and artifacts

- [Formal model and theorem program](formal-model.md) defines deviation
  languages, move-order dominance, commitment, transposition leverage, and
  repertoire covering.
- [Discriminating experiments](experiments.md) gives the smallest falsifying
  structural, game-database, engine, and human tests.
- [Trainer and repertoire design](trainer-design.md) turns those tests into a
  concrete integration plan for the monorepo's `engine/` crate.
- [Prior work and novelty boundary](literature-and-novelty.md) compares the
  proposal with opening-book learning, opponent modeling, recommendation
  networks, robust control, and chess-memory research.
- [Source ledger](sources.md) records the primary URLs and the specific role
  each source plays in this research program.
- [Pilot script](data/pilot.py) and its [checked output](data/output.txt)
  measure what the pinned opening taxonomy can and cannot establish.

## The central distinction

Two routes that reach the same position share all continuation knowledge that
depends only on that position. They do **not** share the risks encountered
before they meet. A move-order decision therefore has two components:

1. **endpoint value:** how desirable and learnable is the position eventually
   reached?;
2. **route exposure:** which opponent choices are enabled, avoided, or made
   practically likely on the way there?

Existing engine-book work already represents positions as a directed graph and
can weight them by reach probability [fishburn18][fishburn18]. Opponent-specific
opening prediction also has a long chess-specific history
[walczak96][walczak96]. What appears to be missing is the human-repertoire
combination: exact transposition classes, adversarial deviation sets,
frequency-and-severity risk, and explicit study cost.

## What the small pilot establishes

The pinned Lichess opening-name corpus is a curated taxonomy, not a sample of
played games. It cannot tell us which route wins more often or which surprise
is common. It can still test whether route-sensitive structure exists at all.

The reproducible pilot finds:

- White has 3,091 distinct move-history decision nodes in the corpus but only
  2,741 exact repetition-position decision nodes, a structural reduction of
  350 cards (11.32%).
- Black has 3,102 history decision nodes but 2,736 position decision nodes, a
  reduction of 366 cards (11.80%).
- Among routes of equal length that reach the same position while keeping the
  opponent's recorded move sequence fixed, there are 270 White-controlled and
  265 Black-controlled pairs.
- The number of curated opponent alternatives differs between the two routes
  in 247 of the White pairs and 220 of the Black pairs. Even the total number
  of legal opponent alternatives differs in 32 and 98 pairs, respectively.
- A coarse set of first-deviation outcome positions is strictly included in
  the competing route's set for 41 White pairs and 46 Black pairs.

These are corpus-relative counts checked by [the pilot](data/pilot.py), which
pins both the input hash and `chess==1.11.2` and exits nonzero if the aggregates
change. The large differences in *catalog* alternatives partly reflect how the
taxonomy was curated. They are evidence that route matters, not a ranking of
move orders.

The memory reduction is particularly suggestive. Prior work estimated opening
knowledge by expanding theoretical move sequences as a tree and argued that
masters may memorize on the order of 100,000 opening moves
[chassy-gobet11][chassy-gobet11]. Exact transpositions imply that at least some
of this supposedly single-use knowledge can be reused. “Transposition-adjusted
opening complexity” is therefore a concrete, testable candidate contribution.
It must be tested on real repertoires and delayed recall before it can be called
a cognitive saving.

## The proposed player metrics

A useful dashboard should not collapse everything into one opening score. It
should expose a Pareto frontier over five quantities:

| Metric | Player question |
|---|---|
| Soundness | How much evaluation do my prescribed moves concede under a pinned analysis protocol? |
| Encounter coverage | What fraction of opponent moves in my rating and time-control band have a prepared response? |
| Surprise robustness | What is the worst uncovered or dangerous reply above a minimum support threshold? |
| Cognitive cost | How many unique position decisions, plans, and route-specific exceptions must I retain? |
| Transfer | Can I recognize the same position and plan through an unseen move order? |

The last column is essential. Chess specialization can substantially improve
recall and problem solving within the trained opening
[bilalic09][bilalic09], but rigidly drilling only a canonical sequence risks
testing sequence recognition rather than position knowledge. The trainer
should deliberately present transposed routes and near-neighbor deviations.

## Product recommendation

The existing trainer in `engine/` is already a strong execution
base: it has full legality, FEN/PGN/SAN interop, a warm UCI engine, MultiPV
analysis, and a browser drill loop. Its current opening layer is intentionally
small and sequence-based: 21 embedded main lines, an opponent that selects the
earliest matching continuation, session-only scoring, and short repetitions
that restart from move one.

The next version should make four conceptual changes:

1. **A repertoire is a policy, not a list of lines.** At our positions it
   records one or more accepted moves; at opponent positions it records the
   covered reply distribution.
2. **The primary study unit is an exact position decision.** Move histories are
   retained as occurrence provenance and as route-specific deviation context.
3. **Correctness and engine quality are separate.** First ask whether the move
   matches the chosen repertoire; then report its pinned-engine regret. A
   sound personal choice need not be Stockfish's first line on every run.
4. **Scheduling is persistent and item-specific.** Retrieve due position,
   deviation, plan, and transposition-transfer cards instead of repeatedly
   replaying one deterministic main line. Spacing and repeated retrieval have
   broad empirical support [cepeda06][cepeda06] [karpicke08][karpicke08], but
   their effectiveness for these chess-specific card types should be measured,
   not assumed.

The full schema, exercise types, and progression metrics are in
[Trainer and repertoire design](trainer-design.md).

## A repertoire hypothesis, not yet a prescription

The pinned graph makes the `1.d4`/`c4`/`Nf3` complex, QGD, Catalan, Slav, and
Semi-Slav regions look unusually rich in reusable transposition hubs. For
example, the pilot's high-ranking hubs include positions with six to eight
recorded routes and dozens of named descendants. This makes a position-based
`1.d4`/`2.c4` repertoire a sensible *first test bed* for White, and a
QGD/Semi-Slav family a sensible test bed for Black.

That is not yet enough to recommend them to the player. Catalog density is not
encounter frequency, engine soundness, stylistic fit, or recall cost. A strong
personal repertoire should be frozen only after the larger experiment combines:

- the player's own games and preferred structures;
- a rating- and time-control-matched Lichess sample;
- pinned MultiPV engine evidence;
- route-risk and unique-card counts;
- a short human pilot measuring delayed recall and enjoyment.

The product can start with a provisional backbone—White `1.d4`/`2.c4`, Caro-
Kann against `1.e4`, and QGD/Semi-Slav against `1.d4`—but every branch should
be treated as a hypothesis to retain, replace, or deepen. The optimization
target is not the fewest lines at any cost; it is the most reliable chess per
minute of study.

## Claims that could be novel

The following are plausible contributions after validation, stated from
strongest to weakest novelty case:

1. **Adversarial move-order dominance.** A formally defined preorder
   comparing routes by bad-deviation language, preparation exits, endpoint
   value, and study cost.
2. **Transposition-adjusted repertoire complexity.** A position-graph measure
   of human opening burden, plus a randomized recall experiment showing when
   quotienting routes creates real transfer rather than merely fewer database
   rows.
3. **Robust repertoire cover.** A minimum-cost, prefix-consistent policy that
   covers a chosen mass of opponent play while guarding against severe rare
   replies; ordinary weighted set cover is only its static special case.
4. **Commitment frontiers.** Exact identification of the move where a route
   loses access to a family of acceptable structures or first exposes a
   high-severity deviation.
5. **Transposition leverage.** A frequency-weighted ranking of positions by
   downstream knowledge reused across distinct reachable routes, discounted
   by pre-hub deviation risk.

A scoped search found the ingredients separately but not this combination.
That supports a candidate novelty claim, not priority. The literature boundary
and search limitations are recorded in
[Prior work and novelty](literature-and-novelty.md).

## Proof and evidence boundary

Lean can certify:

- legality and exact FIDE state transitions;
- when two histories have the same reusable position key;
- finite-language inclusion and dominance properties for a supplied graph;
- correctness of reachability, commitment, and covering definitions;
- factorization theorems saying exactly which annotations may be merged across
  a transposition;
- optimality certificates for small finite covers, if the solver emits a
  checkable witness and lower-bound certificate.

Lean cannot by itself certify that a database frequency predicts this player,
that a centipawn score is true minimax value, that one annotation is memorable,
or that a repertoire causes a rating increase. Those require held-out data,
fixed engine protocols, and human experiments. Exact structure is the spine;
it is not a substitute for chess evidence.

## Decision

This program is worth executing. Within the curated opening catalog, the pilot
already rejects the structural null that route choice and position quotienting
never change any measured quantity. It does not yet establish a practical
playing advantage. The next discriminating result is a held-out, rating-matched
comparison of candidate move orders that reports:

1. unique study positions;
2. covered opponent probability mass;
3. expected and worst supported engine regret before coalescence;
4. delayed recall and transfer through an unseen move order.

If route rankings are unstable, the optimized cover is no smaller than an
expert baseline, or merged cards do not transfer, we should stop claiming a
new theory and retain only the graph-aware trainer engineering.

## Local References

- **[bilalic09]** Merim Bilalić, Peter McLeod, and Fernand Gobet. “Specialization Effect and Its Influence on Memory and Problem Solving in Expert Chess Players.” *Cognitive Science* 33(6), 1117–1143, 2009. https://doi.org/10.1111/j.1551-6709.2009.01030.x
- **[cepeda06]** Nicholas J. Cepeda, Harold Pashler, Edward Vul, John T. Wixted, and Doug Rohrer. “Distributed Practice in Verbal Recall Tasks: A Review and Quantitative Synthesis.” *Psychological Bulletin* 132(3), 354–380, 2006. https://doi.org/10.1037/0033-2909.132.3.354
- **[chassy-gobet11]** Philippe Chassy and Fernand Gobet. “Measuring Chess Experts' Single-Use Sequence Knowledge: An Archival Study of Departure from ‘Theoretical’ Openings.” *PLOS ONE* 6(11), e26692, 2011. https://doi.org/10.1371/journal.pone.0026692
- **[fishburn18]** John P. Fishburn. “Search-based Opening Book Construction.” *ICGA Journal* 40(1), 2–14, 2018. https://doi.org/10.3233/ICG-180039
- **[karpicke08]** Jeffrey D. Karpicke and Henry L. Roediger III. “The Critical Importance of Retrieval for Learning.” *Science* 319(5865), 966–968, 2008. https://doi.org/10.1126/science.1152408
- **[walczak96]** Steven Walczak. “Improving Opening Book Performance Through Modeling of Chess Opponents.” *Proceedings of the 1996 ACM 24th Annual Conference on Computer Science*, 53–57, 1996. https://doi.org/10.1145/228329.228334

[bilalic09]: https://doi.org/10.1111/j.1551-6709.2009.01030.x
[cepeda06]: https://doi.org/10.1037/0033-2909.132.3.354
[chassy-gobet11]: https://doi.org/10.1371/journal.pone.0026692
[fishburn18]: https://doi.org/10.3233/ICG-180039
[karpicke08]: https://doi.org/10.1126/science.1152408
[walczak96]: https://doi.org/10.1145/228329.228334
