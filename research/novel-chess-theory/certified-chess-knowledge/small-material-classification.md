# Small-material exhaustive classification

## The right unit is a motif, not a material signature

“Solve another endgame” is too broad and often not novel.  Four-piece pawnless
endgames were already generated with formal assurance in HOL4
[hurd2005][hurd2005], and a complete executable KRK strategy has already been
proved in Isabelle/HOL [maric2015][maric2015].  The viable target is a
symmetry-reduced family defined by a human motif and a compact target
predicate—for example, a two-pawn race corridor whose outcome flips with side
to move.

The output must be an exact classifier that a player can apply, plus a proof of
its domain and completeness.  A compressed lookup table is useful software but
not by itself a chess theorem.

## Honest state-space bounds

For `k` distinct labeled pieces, merely placing them on distinct squares and
choosing the side to move gives the crude ceiling `2 * P(64, k)`:

| Labeled pieces | Placement-plus-turn ceiling |
|---:|---:|
| 3 | 499,968 |
| 4 | 30,498,048 |
| 5 | 1,829,882,880 |

[`state_space_bounds.py`](data/state_space_bounds.py) checks the arithmetic and
the exact run is recorded in
[`state-space-output.txt`](data/state-space-output.txt).  These are not counts
of legal chess states: they exclude clocks, castling, en passant, and history
and have not removed illegal king placements or symmetries.  They explain why a
four-piece motif can be feasible while a naive five-piece Lean enumeration can
become the wrong first experiment.

## Candidate pilot: tempo-sensitive KPKP corridor

Start with exactly two kings and one pawn per side, no castling or en-passant
right, halfmove clock zero, and a declared pair of pawn files.  Remove positions
with adjacent kings, impossible checks, immediate promotion, or captures that
leave the declared family.  Quotient only by symmetries proved to preserve the
target predicate.

Classify:

```text
TempoFlip(p) := WDL(p with White to move) != WDL(p with Black to move)
```

Then search for a short equivalent formula using only player-facing features:
king distances to capture and promotion squares, opposition/parity, pawn
protection, and move order.  The first checkpoint is one fixed-file corridor,
not all KPKP.

This target is intentionally provisional.  Six-man zugzwangs have already
been systematically mined [bleicher-haworth2010][bleicher-haworth2010], and
Elkies constructed novel mutual-zugzwang pawn positions using combinatorial
game theory [elkies1999][elkies1999].  Literature review must establish that the
chosen corridor and closed predicate are new.

## Certification choices

There are three increasing levels of value:

1. **Recomputed enumeration.** Lean evaluates the finite classifier.  This is
   simple but may rely on native compilation and produces little explanation.
2. **Checked classification certificate.** An external generator emits labels
   or a retrograde witness; Lean checks terminal states, legal successors, and
   the fixed-point conditions.  Hurd's work is the historical benchmark, and
   recent capture/quiet decomposition work shows that self-consistency alone
   must be anchored to verified smaller-material cases
   [pavlov2026][pavlov2026].
3. **Structural theorem.** Prove the learned closed predicate using geometric
   lemmas and a well-founded strategy.  This is the best player-facing result
   and can be much smaller than the exhaustive certificate.

Use level 2 to secure the labels and level 3 for the final claim.  Keep the
external tablebase as an independent cross-check rather than the sole premise.

## Falsification protocol

The pilot succeeds only if:

- independent enumeration and tablebase labels agree under the same WDL/clock
  semantics;
- a deterministic canonical order reproduces the first counterexample to each
  candidate formula;
- the final predicate has no exceptions in the entire declared corridor;
- held-out symmetry/feature strata can be solved by players from the rule;
- Lean checks both the family boundary and the classifier.

Stop if the rule grows by one ad hoc clause per counterexample, if the family
boundary excludes the positions players care about, or if the result is already
implicit in a published strategy.  Also stop if “proof” means trusting one
opaque label file without verifying its recurrence conditions; opaque large
computations are known to require explicit data assurance
[hurd-haworth2010][hurd-haworth2010].

## Training export

An exact classification naturally generates contrastive practice:

- pair the same placement with opposite sides to move;
- pair positions just inside and outside one distance inequality;
- ask for the classifier verdict before asking for a move;
- schedule exceptions more often after false generalization;
- reserve unseen placements for transfer tests.

The card should show the compact predicate and its scope, not the enumeration
index.  Every answer carries the classifier version and certificate digest
described in [the trainer integration](training-integration.md).

## Local References

- **hurd2005** — Joe Hurd, “Formal Verification of Chess Endgame Databases,” in *Theorem Proving in Higher Order Logics: Emerging Trends Proceedings*, Oxford University Computing Laboratory, 2005, 85–100.
- **maric2015** — Filip Marić, Predrag Janičić, and Marko Maliković, “Proving Correctness of a KRK Chess Endgame Strategy by Using Isabelle/HOL and Z3,” in *Automated Deduction—CADE-25*, LNCS 9195, Springer, 2015, 256–271. DOI 10.1007/978-3-319-21401-6_17.
- **bleicher-haworth2010** — Eiko Bleicher and Guy McC. Haworth, “6-Man Chess and Zugzwangs,” in *Advances in Computer Games*, LNCS 6048, Springer, 2010, 123–135. DOI 10.1007/978-3-642-12993-3_12.
- **elkies1999** — Noam D. Elkies, “On Numbers and Endgames: Combinatorial Game Theory in Chess Endgames,” arXiv:math/9905198, 1999; version of the chapter in *Games of No Chance*, MSRI Publications 29, 135–150.
- **pavlov2026** — Alexander Pavlov, “Capture-Quiet Decomposition: A Verification Theorem for Chess Endgame Tablebases,” arXiv:2604.07907, 2026.
- **hurd-haworth2010** — Joe Hurd and Guy McC. Haworth, “Data Assurance in Opaque Computations,” in *Advances in Computer Games*, LNCS 6048, Springer, 2010, 221–231. DOI 10.1007/978-3-642-12993-3_20.

[hurd2005]: https://www.gilith.com/papers/chess.pdf
[maric2015]: https://doi.org/10.1007/978-3-319-21401-6_17
[bleicher-haworth2010]: https://doi.org/10.1007/978-3-642-12993-3_12
[elkies1999]: https://arxiv.org/abs/math/9905198
[pavlov2026]: https://arxiv.org/abs/2604.07907
[hurd-haworth2010]: https://doi.org/10.1007/978-3-642-12993-3_20
