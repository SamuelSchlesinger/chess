# Source index

This branch prefers original papers, official rules, maintained project
documentation, and the two local codebases. Each note records why the source
constrains the research decision; it is not an endorsement of every claim made
by the source.

## Local References

### Rules, retrograde analysis, and tablebases

- [International Chess Federation, *FIDE Laws of Chess Taking Effect from 1 January 2023*](https://handbook.fide.com/chapter/E012023), approved 7 August 2022 — Defines legal-position, dead-position, repetition, claimable 50-move, and automatic 75-move semantics that certificates and tablebase claims must name.
- [Ken Thompson, “Retrograde Analysis of Certain Endgames,” *ICCA Journal* 9(3), 1986, 131–139](https://doi.org/10.3233/ICG-1986-9302) — Primary account of backward endgame computation and the historical baseline for exhaustive chess analysis.
- [Joe Hurd, “Formal Verification of Chess Endgame Databases,” TPHOLs Emerging Trends, 2005, 85–100](https://www.gilith.com/papers/chess.pdf) — Demonstrates HOL4-checked construction of all four-piece pawnless endgames.
- [Joe Hurd and Guy McC. Haworth, “Data Assurance in Opaque Computations,” *Advances in Computer Games*, LNCS 6048, 2010, 221–231](https://doi.org/10.1007/978-3-642-12993-3_20) — Frames the assurance problem for large computations whose outputs are otherwise difficult to inspect.
- [Guy McC. Haworth, “Strategies for Constrained Optimisation,” *ICGA Journal* 23(1), 2000, 9–20](https://doi.org/10.3233/ICG-2000-23103) — Shows why shortest-mate and move-limit-aware practical objectives can recommend different strategies.
- [Eiko Bleicher and Guy McC. Haworth, “6-Man Chess and Zugzwangs,” *Advances in Computer Games*, LNCS 6048, 2010, 123–135](https://doi.org/10.1007/978-3-642-12993-3_12) — Prior systematic mining of six-man tablebases for zugzwang structure.
- [Noam D. Elkies, “On Numbers and Endgames: Combinatorial Game Theory in Chess Endgames,” arXiv:math/9905198, 1999](https://arxiv.org/abs/math/9905198) — Constructs novel mutual zugzwangs and limits any broad novelty claim about pawn-race or tempo phenomena.
- [Alexander Pavlov, “Capture-Quiet Decomposition: A Verification Theorem for Chess Endgame Tablebases,” arXiv:2604.07907, 2026](https://arxiv.org/abs/2604.07907) — Current verification proposal explaining why internal WDL self-consistency needs anchors to smaller material classes.
- [Lichess, `lila-tablebase` README and HTTP API](https://github.com/lichess-org/lila-tablebase/blob/main/README.md), snapshot inspected 14 July 2026 — Documents current legal-move, WDL-category, DTZ, DTM, and DTC fields and seven-/partial-eight-piece access.
- [Lichess, `op1` partial eight-piece tablebase](https://github.com/lichess-org/op1), snapshot inspected 14 July 2026 — Defines the covered opposing-pawn family and its DTC metric that ignores the 50-move rule.

### Explainable chess knowledge and formal strategy

- [Michael Bain and Stephen Muggleton, “Learning Optimal Chess Strategies,” *Machine Intelligence 13*, 1994, 291–309](https://doi.org/10.1093/oso/9780198538509.003.0012) — Establishes inductive rule learning from move-perfect KRK data, so generic rule extraction is not novel.
- [Matej Guid, Martin Možina, Aleksander Sadikov, and Ivan Bratko, “Deriving Concepts and Strategies from Chess Tablebases,” *Advances in Computer Games*, LNCS 6048, 2010, 195–207](https://doi.org/10.1007/978-3-642-12993-3_18) — Derives a human-facing KBNK strategy and supplies the closest baseline for tablebase-to-teaching work.
- [Filip Marić, Predrag Janičić, and Marko Maliković, “Proving Correctness of a KRK Chess Endgame Strategy by Using Isabelle/HOL and Z3,” CADE-25, 2015, 256–271](https://doi.org/10.1007/978-3-319-21401-6_17) — Proves a complete executable KRK strategy, ruling out standard KRK formalization as a novelty claim.
- [Marko Maliković, “A Formal System for Automated Reasoning about Retrograde Chess Problems Using Coq,” CECIIS, 2008](https://archive.ceciis.foi.hr/index.php/ceciis/2008/paper/view/174.html) — Early chess-specific proof-assistant precedent for retrograde problems.
- [Lisa Schut et al., “Bridging the Human–AI Knowledge Gap through Concept Discovery and Transfer in AlphaZero,” *PNAS* 122(13), 2025, e2406675122](https://doi.org/10.1073/pnas.2406675122) — Demonstrates machine-discovered chess concept transfer to elite human players, but not an exact formal theorem pipeline.

### Search and certificates

- [L. Victor Allis, Maarten van der Meulen, and H. Jaap van den Herik, “Proof-Number Search,” *Artificial Intelligence* 66(1), 1994, 91–124](https://doi.org/10.1016/0004-3702(94)90004-3) — Primary AND/OR search method for proving game-theoretic values.
- [Ashish Darbari, Bernd Fischer, and João Marques-Silva, “Industrial-Strength Formally Certified SAT Solving,” arXiv:0911.1678, 2009](https://arxiv.org/abs/0911.1678) — Supports the untrusted-producer/verified-checker architecture.
- [Peter Lammich, “Efficient Verified (UN)SAT Certificate Checking,” *Journal of Automated Reasoning* 64, 2020, 513–532](https://doi.org/10.1007/s10817-019-09525-z) — Gives a mature example of a small formal checker validating large externally generated certificates.

### Learning and delivery

- [Henry L. Roediger III and Jeffrey D. Karpicke, “Test-Enhanced Learning: Taking Memory Tests Improves Long-Term Retention,” *Psychological Science* 17(3), 2006, 249–255](https://doi.org/10.1111/j.1467-9280.2006.01693.x) — Primary evidence for delayed retrieval practice rather than passive restudy.
- [Nicholas J. Cepeda et al., “Spacing Effects in Learning: A Temporal Ridgeline of Optimal Retention,” *Psychological Science* 19(11), 2008, 1095–1102](https://doi.org/10.1111/j.1467-9280.2008.02209.x) — Shows that useful spacing depends on the final retention horizon, cautioning against a universal fixed interval sequence.

### Local primary artifacts

- [`Chess/RepetitionKey.lean`](../../../Chess/RepetitionKey.lean), local formalization snapshot inspected 14 July 2026 — Proves exact executable key equality equivalent to the modeled FIDE repetition relation.
- [`Chess/Theory/PawnGeometry.lean`](../../../Chess/Theory/PawnGeometry.lean), local formalization snapshot inspected 14 July 2026 — Supplies the geometric pawn-square control theorem and explicitly states what a later KPK classification must add.
- [`engine/src/board.rs`](../../../engine/src/board.rs), imported Rust snapshot inspected 14 July 2026 — Defines the Polyglot-compatible board hash and adjacent-pawn en-passant contribution.
- [`engine/src/game.rs`](../../../engine/src/game.rs), imported Rust snapshot inspected 14 July 2026 — Reuses board hashes for repetition counting and game outcomes.
- [`engine/src/search.rs`](../../../engine/src/search.rs), imported Rust snapshot inspected 14 July 2026 — Reuses the same keys for search-path repetition detection.
- [`engine/src/bin/chess-trainer`](../../../engine/src/bin/chess-trainer), imported Rust snapshot inspected 14 July 2026 — Provides the current opening-book, UCI grading, short-rep, and session-statistics delivery layer.
