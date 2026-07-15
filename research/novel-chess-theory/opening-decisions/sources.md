# Source ledger

This ledger records the primary sources used by the opening-decisions subtree
and the narrow reason each matters. The novelty assessment remains scoped; a
listed source supports an ingredient, not a claim that the proposed synthesis
has publication priority.

## Chess opening models and empirical studies

- [Walczak, “Improving Opening Book Performance Through Modeling of Chess Opponents” (1996)](https://doi.org/10.1145/228329.228334) — Establishes chess-specific opponent opening prediction and motivates conditioning preparation on the player likely to be faced.
- [Hyatt, “Book Learning—A Methodology to Tune an Opening Book Automatically” (1999)](https://doi.org/10.3233/ICG-1999-22102) — Shows that practical results can tune an engine opening book, an important predecessor to evidence-weighted repertoire choice.
- [Fishburn, “Search-based Opening Book Construction” (2018)](https://doi.org/10.3233/ICG-180039) — Represents an opening book as a position DAG with reach probabilities, the closest structural precedent for the proposed graph.
- [Levene and Bar-Ilan, “Comparing Typical Opening Move Choices Made by Humans and Chess Engines” (2007)](https://doi.org/10.1093/comjnl/bxm025) — Supplies evidence that human and engine opening distributions differ, so engine rank cannot substitute for encounter modeling.
- [De Marzo and Servedio, “Quantifying the Complexity and Similarity of Chess Openings Using Online Chess Community Data” (2023)](https://doi.org/10.1038/s41598-023-31658-w) — Uses a player-opening network for opening similarity, complexity, and recommendation, providing the closest recommendation-system comparison.
- [Chassy and Gobet, “Measuring Chess Experts' Single-Use Sequence Knowledge” (2011)](https://doi.org/10.1371/journal.pone.0026692) — Estimates opening sequence knowledge and motivates testing whether exact transpositions reduce genuinely single-use material.
- [Bilalić, McLeod, and Gobet, “Specialization Effect and Its Influence on Memory and Problem Solving in Expert Chess Players” (2009)](https://doi.org/10.1111/j.1551-6709.2009.01030.x) — Demonstrates opening-specialization effects on chess recall and problem solving, grounding the proposed transfer experiment.
- [Chowdhary, Iacopini, and Battiston, “Quantifying Human Performance in Chess” (2023)](https://doi.org/10.1038/s41598-023-27735-9) — Documents skill-dependent opening specialization and response diversity at large scale, supporting rating-stratified cohorts.
- [Munshi, “A Method for Comparing Chess Openings” (2014)](https://doi.org/10.48550/arXiv.1402.6791) — Provides a controlled engine-comparison precedent while clarifying why engine evidence alone is not a human repertoire objective.

## Formal and optimization ingredients

- [Kuhn, “Extensive Games and the Problem of Information” (1953)](https://doi.org/10.1515/9781400829156-011) — Supplies the classical extensive-form framework for the finite alternating preparation game.
- [Iyengar, “Robust Dynamic Programming” (2005)](https://doi.org/10.1287/moor.1040.0129) — Supplies the ambiguity-set and robust dynamic-programming machinery used to frame uncertain opponent move distributions.
- [Chvátal, “A Greedy Heuristic for the Set-Covering Problem” (1979)](https://doi.org/10.1287/moor.4.3.233) — Supplies the classical weighted-cover baseline that robust, prefix-consistent repertoire cover strictly extends.

## Learning and scheduling

- [Cepeda et al., “Distributed Practice in Verbal Recall Tasks” (2006)](https://doi.org/10.1037/0033-2909.132.3.354) — Reviews the spacing effect and supports testing persistent distributed review rather than session-only repetition.
- [Karpicke and Roediger, “The Critical Importance of Retrieval for Learning” (2008)](https://doi.org/10.1126/science.1152408) — Establishes the value of repeated retrieval, motivating active position recall instead of passive line display.
- [Ye et al., “A Stochastic Shortest Path Algorithm for Optimizing Spaced Repetition Scheduling” (2022)](https://doi.org/10.1145/3534678.3539081) — Provides a modern formal scheduling model while motivating chess-specific calibration rather than blind adoption.

## Data and analysis tools

- [Lichess opening taxonomy, pinned upstream revision](https://github.com/lichess-org/chess-openings/tree/292fd0468068f58bb244f7fe1c3e573e493c3c53) — Supplies the CC0 move sequences used only for the reproducible structural pilot, not for popularity estimates.
- [Lichess Open Database](https://database.lichess.org/) — Supplies CC0 rated-game archives for the proposed held-out, rating- and time-control-stratified experiment.
- [Stockfish official source repository](https://github.com/official-stockfish/Stockfish) — Supplies the engine whose commit, network, options, and node budget must be pinned for reproducible evaluation evidence.

## Local References

- The vendored taxonomy's revision, license, generation environment, and SHA-256 are recorded in [`data/lichess-openings/README.md`](../../../data/lichess-openings/README.md).
- The subtree's quantitative checks are implemented by [`data/pilot.py`](data/pilot.py) and recorded in [`data/output.txt`](data/output.txt).
