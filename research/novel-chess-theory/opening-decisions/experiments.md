# Discriminating experiments

The research should advance only when an experiment can make a favored idea
lose. This document gives three gates: structural feasibility, practical chess
value, and cognitive transfer.

## Gate 1: the pinned taxonomy pilot

The [pilot script](data/pilot.py) replays the repository's pinned
`data/lichess-openings/all.tsv` with `chess==1.11.2`. It verifies the input
SHA-256, constructs every unique prefix, groups prefixes by the project's exact
repetition key, and exits nonzero if its expected aggregates drift.

It asks only three questions:

1. Does repetition-key grouping reduce recorded history decision nodes?
2. Do same-repetition-key-endpoint histories with an identical raw opponent UCI
   projection have different summed per-decision branch incidences?
3. Are there transposition nodes with both multiple incoming histories and
   substantial named material downstream?

The answer is yes to all three. Full output is committed in
[data/output.txt](data/output.txt). In particular, position grouping reduces
White's decision nodes from 3,091 to 2,741 and Black's from 3,102 to 2,736.
There are 535 same-repetition-key-endpoint, opponent-projection-matched history
pairs, and their summed curated branch incidences differ in 467 cases.

### What this does not show

The source is an opening-name taxonomy. Its branch count measures editorial
density, not popularity, difficulty, or danger. The pilot's largest contrast
for White involves `1.f3`/`e4` move orders, which is a warning against reading
catalog counts as recommendations. Its “hub score” is deliberately descriptive
and uncalibrated. Gate 1 justifies building Gate 2; it cannot select a personal
repertoire. The matched histories are not yet conditional policies, and their
incidence counts are neither unique moves nor deviation-language inclusion.
Gate 1 therefore supplies no dominance result.

## Gate 2: held-out games plus pinned engine analysis

### Cohort

Use a frozen Lichess standard-rated archive, released under CC0
[lichess-db][lichess-db]. Stream it rather than loading it into a GUI, and
record the archive name and SHA-256. Exclude variants, bots, games with illegal
replay, missing ratings, and abnormal termination where the intended outcome
statistic would be misleading.

For a player targeting approximately 2000, stratify rather than pool:

- both players 1600–1799, 1800–1999, 2000–2199, and 2200–2399;
- rapid/classical, blitz, and bullet in separate reports;
- White and Black decisions separately;
- calendar month or quarter, so fashions and engine discoveries are visible.

The approximate rating target and these band boundaries are initial design
choices, not discovered cutoffs. Freeze them before candidate generation and
publish sensitivity analyses for adjacent bands.

Use three chronological windows: an early training window to fit move
probabilities, an intervening validation window to select routes and freeze all
thresholds, and a later untouched test window used once for the published
comparison. If personalized opponent models are tested, split by player as well
as time so the same player's repeated habits do not leak across any boundary.
Prior large-scale work confirms that chess-opening diversity and response
behavior vary with skill [chowdhary23][chowdhary23].

### Exact graph construction

Replay each game into complete states through a declared horizon, with 20 plies
as an initial engineering default rather than a validated boundary. Store:

- exact canonical repetition key and a collision-checked storage identity;
- complete FIDE state, including clocks and history, for every occurrence;
- incoming history occurrence and game identifier;
- side, rating band, time control, date, and result;
- move counts at each position;
- whether the path belongs to the candidate personal policy.

Compute frequencies by pushing occurrence weights through the exact key. Do
not group only by opening name or raw SAN prefix. Keep route occurrences, since
move-order risk lives before a transposition.

### Candidate generation

Generate route pairs only when:

1. they begin at the same player decision state;
2. the player can choose between the routes through a conditional policy;
3. they reach the same exact target key or the same declared target class;
4. each has sufficient training and validation support under a threshold fixed
   before opening the test window;
5. comparison stops at first coalescence, a common horizon, or a declared
   terminal condition.

For each pair, enumerate opponent replies before coalescence. Report both the
raw move distribution and a smoothed interval; never turn an unobserved move
into probability zero. Generate and rank candidates on training data, tune the
support and smoothing rules on validation data, then freeze the complete list
before evaluating it on the test window.

Represent every reply as a typed `(complete pre-state, move, child state)`
event. For a structural inclusion claim, lift those events to one common finite
scenario set `Omega` of contingent opponent policies over the union of states
visited by both routes, as defined in the formal model. Raw UCI words remain
route-local diagnostics and are never compared across policies by inclusion.

### Engine protocol

Pin an official Stockfish commit, network, threads, hash, tablebase setting,
MultiPV count, and node budget [stockfish][stockfish]. Nodes are preferable to
wall time for reproducibility. Analyze:

- every prescribed player move;
- every opponent move above the support threshold;
- all legal opponent moves whose shallow scan exceeds a danger trigger;
- both sides of every candidate move-order comparison.

Store the complete UCI transcript, score convention, principal variations, and
search effort. Re-run a sample at a larger node budget and with a second engine
to quantify ranking stability. Call the result an **engine estimate**, never a
mathematical bound.

### Estimands and aggregation

Fix a player perspective `P` and make every engine value `V_P` larger when the
position is better for `P`. Keep two quantities separate:

```text
policy_move_regret(s)
  = V_P(best engine move at player node s)
    - V_P(prescribed repertoire move at s)

deviation_loss_tau(e)
  = max(0, tau - V_P(position after opponent event e and prescribed response))
```

The score convention, response horizon, threshold `tau`, and treatment of an
unprepared response must be frozen before the test run. A missing response is a
preparation exit and receives a separately declared failure loss; it is not
silently evaluated as if the engine supplied the repertoire.

Let `e` range over **first** opponent deviations before coalescence. Its
unconditional reach mass is

```text
Pr(reach the pre-deviation state with no earlier deviation)
  * Pr(the opponent chooses e.move | that state).
```

Then expected supported deviation loss is the sum of this mass times
`deviation_loss_tau(e)`. Restricting to first deviations makes the events
disjoint and prevents double counting after preparation has already been left.
Report preparation-exit probability, expected deviation loss, and worst
supported deviation separately. Do not call deviation loss “regret”; reserve
that word for the same-node prescribed-move comparison above.

### Outputs

For each route and rating/time cohort, publish:

- unique position decisions and route-specific exceptions;
- empirical coverage mass with confidence or credible intervals;
- probability of leaving preparation before coalescence;
- expected supported deviation loss under the declared `tau` and first-event
  convention;
- worst supported deviation and its sample size;
- maximum policy-move regret among prescribed moves;
- transposition leverage decomposed into incoming mass, reusable downstream
  study units, and pre-hub risk;
- commitment points with the target classes lost.

Then compute Pareto-optimal routes. A single weighted score may be offered only
as a user-configurable view over these quantities.

### Gate-2 falsifiers

Before processing the test window, preregister the horizon, support count or
probability threshold, danger trigger, acceptable-value threshold `tau`,
maximum prescribed-move regret, engine-ranking stability tolerance, minimum
study-unit saving, candidate-count cap, and all engine settings. The taxonomy
pilot does not supply evidence-based values for these parameters. They are
design choices whose values and sensitivity ranges must be published.

Abandon or weaken the move-order theory claim if any of these hold:

- candidate rankings reverse across preregistered adjacent test subperiods or
  modest engine budgets without an intelligible chess reason;
- route-risk differences vanish after conditioning on rating and time control;
- all apparent compression comes from rare or unreachable routes;
- the optimized policy saves fewer study units than the preregistered minimum
  at equal coverage and soundness;
- expert review judges the highest-ranked “risks” strategically meaningless.

## Gate 3: does graph compression help a human?

Database compression is not cognitive compression. Separate a product
feasibility pilot from an inferential learning study.

### Gate 3a: N-of-1 feasibility pilot

For one player's initial four-week trial, instantiate 40–80 total study units
as a capacity range, not as a powered sample size. Match several unfamiliar,
disjoint transposition components by baseline difficulty and randomly assign
each component to one condition:

- **line condition:** drill canonical move sequences as separate items;
- **graph condition:** drill one position card plus route-specific deviation
   cards, including alternate routes into the same position.

Match conditions for total study time and engine quality. Never move a position
component from one condition to the other, because graph training would
directly contaminate a later line condition. Test immediately, after one week,
and after four weeks. Use the result to debug cards, timing, and instrumentation
only; an N-of-1 result supports no population-level novelty claim.

### Gate 3b: inferential study

For a publishable randomized study, determine participant count by a
preregistered power analysis or simulation for one primary outcome:

> accuracy on unseen transposing move orders after four weeks, with study time
> held equal between conditions.

Randomize disjoint transposition components as clusters, counterbalance which
families receive graph versus line training across participants, and prohibit
cross-condition position overlap. Analyze the binary primary outcome with a
declared repeated-measures model containing condition and order effects plus
participant and component effects. Prespecify exclusions, missing-session
handling, the estimand, confidence interval, and multiplicity correction for
secondary outcomes before enrollment.

Secondary outcomes are:

1. correct repertoire move from a delayed exact position;
2. response latency without a hint;
3. correct response to a plausible off-main-line deviation;
4. number of reviews required to maintain a declared recall target;
5. confidence calibration, perceived workload, and performance in practice
   games.

Spacing and retrieval practice are well supported in general memory research
[cepeda06][cepeda06]
[karpicke08][karpicke08], while chess research shows a large benefit from
opening specialization [bilalic09][bilalic09]. Neither result establishes that
our merged chess cards work; that is exactly what Gate 3 tests.

### Gate-3 falsifiers

The cognitive novelty fails if graph cards merely reduce item count while
worsening delayed recall, if transfer to unseen routes does not improve, or if
route exceptions erase the saved study time. Population-level cognitive novelty
requires Gate 3b's powered primary analysis; Gate 3a can only reject an unusable
prototype. If the claim fails, retain the exact graph for analysis but schedule
history-specific cards.

## Minimum executable sequence

1. Run the committed taxonomy pilot.
2. Use the catalog-dense `d4/c4/Nf3` region and one separately selected `e4`
   region as engineering fixtures, not repertoire recommendations.
3. Process frozen training, validation, and untouched chronological test
   windows.
4. Freeze a compute-budget cap (initially five route pairs) and a fixed
   Stockfish node budget in the preregistration; select the pairs without test
   data.
5. Have a strong player blind-review the resulting risk explanations.
6. Put 40–80 instantiated study units into the four-week N-of-1 feasibility
   pilot, then power Gate 3b separately if the prototype survives.

This is enough to decide whether to scale the product experiment to a complete
personal repertoire. It is not enough for a population-level learning claim.
It avoids spending months proving definitions around a metric that the initial
player does not recognize as useful.

## Reproduction checklist

Every published quantitative result should include:

- exact input archive URL, date, license, size, and SHA-256;
- parser and chess-rules implementation versions;
- all cohort filters and excluded-game counts;
- exact state-key definition;
- source commit and command line for graph construction;
- engine commit, network hash, UCI options, and node budget;
- train/validation/test dates and sampling seed;
- preregistered candidate-selection, threshold, primary-outcome, power, and
  missing-data rules;
- raw aggregate tables before ranking;
- expert-review protocol and disagreements;
- a rerunnable command that fails on expected-count drift.

## Local References

- **[bilalic09]** Merim Bilalić, Peter McLeod, and Fernand Gobet. “Specialization Effect and Its Influence on Memory and Problem Solving in Expert Chess Players.” *Cognitive Science* 33(6), 1117–1143, 2009. https://doi.org/10.1111/j.1551-6709.2009.01030.x
- **[cepeda06]** Nicholas J. Cepeda, Harold Pashler, Edward Vul, John T. Wixted, and Doug Rohrer. “Distributed Practice in Verbal Recall Tasks: A Review and Quantitative Synthesis.” *Psychological Bulletin* 132(3), 354–380, 2006. https://doi.org/10.1037/0033-2909.132.3.354
- **[chowdhary23]** Sandeep Chowdhary, Iacopo Iacopini, and Federico Battiston. “Quantifying Human Performance in Chess.” *Scientific Reports* 13, article 2113, 2023. https://doi.org/10.1038/s41598-023-27735-9
- **[karpicke08]** Jeffrey D. Karpicke and Henry L. Roediger III. “The Critical Importance of Retrieval for Learning.” *Science* 319(5865), 966–968, 2008. https://doi.org/10.1126/science.1152408
- **[lichess-db]** Lichess. “Lichess Open Database.” CC0 database exports and format documentation, accessed 2026-07-14. https://database.lichess.org/
- **[stockfish]** Stockfish developers. “Stockfish: a free and strong UCI chess engine.” Official source repository, accessed 2026-07-14. https://github.com/official-stockfish/Stockfish

[bilalic09]: https://doi.org/10.1111/j.1551-6709.2009.01030.x
[cepeda06]: https://doi.org/10.1037/0033-2909.132.3.354
[chowdhary23]: https://doi.org/10.1038/s41598-023-27735-9
[karpicke08]: https://doi.org/10.1126/science.1152408
[lichess-db]: https://database.lichess.org/
[stockfish]: https://github.com/official-stockfish/Stockfish
