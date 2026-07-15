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

1. Does exact-position grouping reduce recorded decision nodes?
2. Do same-target routes controlled by one side expose different opponent
   branches even when the recorded replies are identical?
3. Are there transposition nodes with both multiple incoming histories and
   substantial named material downstream?

The answer is yes to all three. Full output is committed in
[data/output.txt](data/output.txt). In particular, position grouping reduces
White's decision nodes from 3,091 to 2,741 and Black's from 3,102 to 2,736.
There are 535 same-target controllable route pairs, and their curated
opponent-alternative counts differ in 467 cases.

### What this does not show

The source is an opening-name taxonomy. Its branch count measures editorial
density, not popularity, difficulty, or danger. The pilot's largest contrast
for White involves `1.f3`/`e4` move orders, which is a warning against reading
catalog counts as recommendations. Its “hub score” is deliberately descriptive
and uncalibrated. Gate 1 justifies building Gate 2; it cannot select a personal
repertoire.

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

Use an earlier chronological window to fit move probabilities and a later
window for evaluation. If personalized opponent models are tested, split by
player as well as time so the same player's repeated habits do not leak across
the boundary. Prior large-scale work confirms that chess-opening diversity and
response behavior vary with skill [chowdhary23][chowdhary23].

### Exact graph construction

Replay each game into complete states through a declared horizon, initially 20
plies. Store:

- exact canonical repetition key and a collision-checked storage identity;
- complete FIDE state where clock or repetition claims could matter;
- incoming history occurrence and game identifier;
- side, rating band, time control, date, and result;
- move counts at each position;
- whether the path belongs to the provisional personal policy.

Compute frequencies by pushing occurrence weights through the exact key. Do
not group only by opening name or raw SAN prefix. Keep route occurrences, since
move-order risk lives before a transposition.

### Candidate generation

Generate route pairs only when:

1. they begin at the same player decision state;
2. the player can choose between the routes through a conditional policy;
3. they reach the same exact target key or the same declared target class;
4. each has sufficient held-out support;
5. comparison stops at first coalescence, a common horizon, or a declared
   terminal condition.

For each pair, enumerate opponent replies before coalescence. Report both the
raw move distribution and a smoothed interval; never turn an unobserved move
into probability zero.

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

### Outputs

For each route and rating/time cohort, publish:

- unique position decisions and route-specific exceptions;
- empirical coverage mass with confidence or credible intervals;
- probability of leaving preparation before coalescence;
- expected engine regret from supported deviations;
- worst supported deviation and its sample size;
- max regret among prescribed moves;
- transposition leverage decomposed into incoming mass, reusable downstream
  cards, and pre-hub risk;
- commitment points with the target classes lost.

Then compute Pareto-optimal routes. A single weighted score may be offered only
as a user-configurable view over these quantities.

### Gate-2 falsifiers

Abandon or weaken the move-order theory claim if any of these hold:

- candidate rankings reverse across adjacent held-out periods or modest engine
  budgets without an intelligible chess reason;
- route-risk differences vanish after conditioning on rating and time control;
- all apparent compression comes from rare or unreachable routes;
- the optimized policy saves fewer than a practically chosen minimum number of
  cards at equal coverage and soundness;
- expert review judges the highest-ranked “risks” strategically meaningless.

## Gate 3: does graph compression help a human?

Database compression is not cognitive compression. Run a randomized,
within-player crossover study in the trainer:

- **line condition:** drill canonical move sequences as separate items;
- **graph condition:** drill one position card plus route-specific deviation
  cards, including alternate routes into the same position.

Match the conditions for total study time and engine quality. Use opening
families the participant does not already know, counterbalanced across
conditions. Test immediately, after one week, and after four weeks.

Primary outcomes:

1. correct repertoire move from a delayed exact position;
2. response latency without a hint;
3. transfer to an unseen transposing move order;
4. correct response to a plausible off-main-line deviation;
5. number of reviews required to maintain a target recall rate.

Secondary outcomes are confidence calibration, perceived workload, and
performance in practice games. Spacing and retrieval practice are well
supported in general memory research [cepeda06][cepeda06]
[karpicke08][karpicke08], while chess research shows a large benefit from
opening specialization [bilalic09][bilalic09]. Neither result establishes that
our merged chess cards work; that is exactly what Gate 3 tests.

### Gate-3 falsifiers

The cognitive novelty fails if graph cards merely reduce item count while
worsening delayed recall, if transfer to unseen routes does not improve, or if
route exceptions erase the saved study time. In that case, retain the exact
graph for analysis but schedule history-specific cards.

## Minimum executable sequence

1. Run the committed taxonomy pilot.
2. Select two high-support move-order families, initially one `d4/c4/Nf3`
   complex and one `e4` complex.
3. Process one frozen game month and one chronological holdout month.
4. Analyze only the top five route pairs at a fixed Stockfish node budget.
5. Have a strong player blind-review the resulting risk explanations.
6. Put 40–80 cards into a four-week crossover trial.

This is enough to decide whether to scale to a complete personal repertoire.
It avoids spending months proving definitions around a metric that players do
not recognize as useful.

## Reproduction checklist

Every published quantitative result should include:

- exact input archive URL, date, license, size, and SHA-256;
- parser and chess-rules implementation versions;
- all cohort filters and excluded-game counts;
- exact state-key definition;
- source commit and command line for graph construction;
- engine commit, network hash, UCI options, and node budget;
- train/validation/test dates and sampling seed;
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
