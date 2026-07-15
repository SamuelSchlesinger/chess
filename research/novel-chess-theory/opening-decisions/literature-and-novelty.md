# Prior work and novelty boundary

This is a scoped search, not an exhaustive priority review. Searches on
2026-07-14 covered combinations of “chess opening book,” “move order,”
“transposition,” “opponent modeling,” “repertoire optimization,” “opening
recommendation,” “set cover,” “robust policy,” and “spaced repetition,” then
followed citations from the strongest chess-specific primary sources.

## What is already established

| Source | Established contribution | What it does not supply here |
|---|---|---|
| Walczak 1996 [walczak96][walczak96] | Predicts a chess opponent's opening choices from game history and argues that opponent knowledge can shrink an engine book. | Exact FIDE-state quotient, route-deviation inclusion, human study cost, or robust coverage. |
| Hyatt 1999 [hyatt99][hyatt99] | Automatically tunes Crafty's opening choices from repeated practical results. | A human repertoire or a route-comparison theorem. |
| Fishburn 2018 [fishburn18][fishburn18] | Constructs a chess opening book as a position DAG with absolute reach probabilities against a specified opponent book. | Cognitive units, commitment, adversarial deviation languages, or delayed recall. |
| Levene and Bar-Ilan 2007 [levene07][levene07] | Compares human and engine move-choice distributions at opening positions. | Policy compression or move-order dominance. |
| De Marzo and Servedio 2023 [demarzo23][demarzo23] | Builds a player-opening bipartite network, measures opening relatedness/complexity, and back-tests future-opening recommendations. | Its nodes are opening labels and player co-occurrences, not exact position states or route risks. |
| Chassy and Gobet 2011 [chassy-gobet11][chassy-gobet11] | Estimates theoretical sequence depth and the large amount of “single-use” opening knowledge held by experts. | Transposition-adjusted knowledge counts or a test of reuse across routes. |
| Bilalić, McLeod, and Gobet 2009 [bilalic09][bilalic09] | Shows that opening specialization substantially improves recall and problem solving within the specialty. | A method for selecting the specialty or compressing its move orders. |
| Chowdhary, Iacopini, and Battiston 2023 [chowdhary23][chowdhary23] | Finds skill-dependent specialization and response diversity in more than 120 million online games. | Exact position-level repertoire coverage and route risk. |
| Munshi 2014 [munshi14][munshi14] | Proposes controlled engine comparisons of openings. | Opponent frequency, memory cost, transposition reuse, or formal certification. |

Generic theory also covers important components. Finite extensive games and
behavioral strategies are classical [kuhn53][kuhn53]. Robust dynamic
programming treats ambiguity in transition measures and relates rectangular
models to perfect-information zero-sum games [iyengar05][iyengar05]. Weighted
set cover and its greedy approximation are classical [chvatal79][chvatal79].
Spacing and retrieval practice have a large experimental literature
[cepeda06][cepeda06] [karpicke08][karpicke08]. None of these generic results is
itself a new chess theorem.

## The gap worth testing

The candidate contribution is the conjunction of five choices:

1. use complete FIDE states for legality and an exact, justified quotient for
   reusable position knowledge;
2. keep route occurrences so alternatives before coalescence are not erased;
3. compare move orders by finite opponent-deviation languages and robust risk;
4. optimize a prefix-consistent human repertoire for coverage, soundness, and
   cognitive cost;
5. validate “compression” through delayed recall and transfer to unseen routes.

The closest chess work optimizes engine opening books, predicts opening labels,
or measures sequence knowledge. The scoped search found no primary source that
defines route dominance by opponent-language inclusion, no transposition-
adjusted empirical estimate of human repertoire burden, and no robust
position-graph repertoire cover tested for human recall.

This is enough to say **candidate novelty**, not “first ever.” Move-order
avoidance is old practical chess knowledge, transpositions are classical, and
commercial tools may implement undocumented heuristics. A publication-quality
priority claim requires broader searches in ICGA archives, chess-instruction
patents and products, non-English chess literature, theses, and direct contact
with tool authors.

## Strong and weak novelty claims

### Defensible after the proposed experiments

- A precise adversarial move-order dominance relation and proved laws.
- The first reported exact-position versus history-card differential for a
  declared repertoire corpus, if priority review holds.
- A held-out demonstration that one move order has lower supported deviation
  risk at equal endpoint value and study cost.
- A randomized result showing that graph-based cards improve transfer per
  study minute.
- A formally checked certificate that a finite personal policy covers every
  demanded scenario under a declared scope.

### Not defensible from current evidence

- “We discovered transpositions.”
- “This opening is objectively best.”
- “An engine centipawn score is a proof.”
- “The pinned opening-name corpus measures popularity.”
- “Fewer database nodes necessarily means less human memory.”
- “This opening program will cause a player to reach 2000.”

## Publication shape

A credible contribution would have three linked artifacts:

1. **Formal paper:** state quotient, deviation languages, dominance,
   commitment, and cover-certificate theorems in Lean.
2. **Empirical paper/tool report:** frozen game cohorts, pinned engine protocol,
   route-risk tables, repertoire-cover ablations, and expert review.
3. **Learning study:** line cards versus position-graph cards, with delayed
   recall and unseen-route transfer as primary outcomes.

The player-facing deliverable can precede publication, but should expose every
assumption: cohort, horizon, engine protocol, probability threshold, study-cost
model, and uncertainty.

## Local References

- **[bilalic09]** Merim Bilalić, Peter McLeod, and Fernand Gobet. “Specialization Effect and Its Influence on Memory and Problem Solving in Expert Chess Players.” *Cognitive Science* 33(6), 1117–1143, 2009. https://doi.org/10.1111/j.1551-6709.2009.01030.x
- **[cepeda06]** Nicholas J. Cepeda, Harold Pashler, Edward Vul, John T. Wixted, and Doug Rohrer. “Distributed Practice in Verbal Recall Tasks: A Review and Quantitative Synthesis.” *Psychological Bulletin* 132(3), 354–380, 2006. https://doi.org/10.1037/0033-2909.132.3.354
- **[chassy-gobet11]** Philippe Chassy and Fernand Gobet. “Measuring Chess Experts' Single-Use Sequence Knowledge: An Archival Study of Departure from ‘Theoretical’ Openings.” *PLOS ONE* 6(11), e26692, 2011. https://doi.org/10.1371/journal.pone.0026692
- **[chowdhary23]** Sandeep Chowdhary, Iacopo Iacopini, and Federico Battiston. “Quantifying Human Performance in Chess.” *Scientific Reports* 13, article 2113, 2023. https://doi.org/10.1038/s41598-023-27735-9
- **[chvatal79]** Vašek Chvátal. “A Greedy Heuristic for the Set-Covering Problem.” *Mathematics of Operations Research* 4(3), 233–235, 1979. https://doi.org/10.1287/moor.4.3.233
- **[demarzo23]** Giordano De Marzo and Vito D. P. Servedio. “Quantifying the Complexity and Similarity of Chess Openings Using Online Chess Community Data.” *Scientific Reports* 13, article 5327, 2023. https://doi.org/10.1038/s41598-023-31658-w
- **[fishburn18]** John P. Fishburn. “Search-based Opening Book Construction.” *ICGA Journal* 40(1), 2–14, 2018. https://doi.org/10.3233/ICG-180039
- **[hyatt99]** Robert M. Hyatt. “Book Learning—A Methodology to Tune an Opening Book Automatically.” *ICGA Journal* 22(1), 3–12, 1999. https://doi.org/10.3233/ICG-1999-22102
- **[iyengar05]** Garud N. Iyengar. “Robust Dynamic Programming.” *Mathematics of Operations Research* 30(2), 257–280, 2005. https://doi.org/10.1287/moor.1040.0129
- **[karpicke08]** Jeffrey D. Karpicke and Henry L. Roediger III. “The Critical Importance of Retrieval for Learning.” *Science* 319(5865), 966–968, 2008. https://doi.org/10.1126/science.1152408
- **[kuhn53]** Harold W. Kuhn. “Extensive Games and the Problem of Information.” In *Contributions to the Theory of Games II*, 193–216, Princeton University Press, 1953. https://doi.org/10.1515/9781400829156-011
- **[levene07]** Mark Levene and Judit Bar-Ilan. “Comparing Typical Opening Move Choices Made by Humans and Chess Engines.” *The Computer Journal* 50(5), 567–573, 2007. https://doi.org/10.1093/comjnl/bxm025
- **[munshi14]** Jamal Munshi. “A Method for Comparing Chess Openings.” arXiv:1402.6791, 2014. https://doi.org/10.48550/arXiv.1402.6791
- **[walczak96]** Steven Walczak. “Improving Opening Book Performance Through Modeling of Chess Opponents.” *Proceedings of the 1996 ACM 24th Annual Conference on Computer Science*, 53–57, 1996. https://doi.org/10.1145/228329.228334

[bilalic09]: https://doi.org/10.1111/j.1551-6709.2009.01030.x
[cepeda06]: https://doi.org/10.1037/0033-2909.132.3.354
[chassy-gobet11]: https://doi.org/10.1371/journal.pone.0026692
[chowdhary23]: https://doi.org/10.1038/s41598-023-27735-9
[chvatal79]: https://doi.org/10.1287/moor.4.3.233
[demarzo23]: https://doi.org/10.1038/s41598-023-31658-w
[fishburn18]: https://doi.org/10.3233/ICG-180039
[hyatt99]: https://doi.org/10.3233/ICG-1999-22102
[iyengar05]: https://doi.org/10.1287/moor.1040.0129
[karpicke08]: https://doi.org/10.1126/science.1152408
[kuhn53]: https://doi.org/10.1515/9781400829156-011
[levene07]: https://doi.org/10.1093/comjnl/bxm025
[munshi14]: https://doi.org/10.48550/arXiv.1402.6791
[walczak96]: https://doi.org/10.1145/228329.228334
