# Tablebase mining, repaired rules, and folklore counterexamples

## Research target

Use perfect-play data as a counterexample oracle for a human claim, not as a
replacement for the claim.  The desired output has four parts:

1. a precisely scoped heuristic `H(position)`;
2. the canonical smallest positions where `H` disagrees with the chosen
   tablebase outcome;
3. a short repaired predicate `H*` with named exception mechanisms;
4. a Lean theorem connecting `H*` to the repository's chess semantics.

This is narrower than generic explainable rule extraction.  Inductive logic
programming has already learned KRK rules [bain1994][bain1994], and argument-
based learning has already produced a human-facing KBNK strategy
[guid2010][guid2010].  Novelty must reside in the specific repaired theorem and
its minimal exceptions.

## What the oracle actually says

Retrograde analysis computes game values by working backward from terminal
positions [thompson1986][thompson1986].  Current Lichess infrastructure exposes
complete seven-piece Syzygy data and a partial eight-piece source, with legal
moves and several metrics [lila-tablebase][lila-tablebase].  The partial
eight-piece `op1` tables cover positions with opposing pawns on the same file
and report depth to conversion while explicitly ignoring the 50-move rule
[op1][op1].

The metric is part of every claim:

- **WDL** answers theoretical win/draw/loss under the table's rule convention.
- **DTZ** optimizes distance to a zeroing pawn move or capture and supports
  50-move-aware play; it is not shortest mate.
- **DTM** optimizes mate length and can recommend moves that are unnatural or
  poor under a move-limit objective.
- **DTC** optimizes conversion and, in `op1`, ignores the 50-move rule.

Lichess's API distinguishes categories such as win, cursed win, draw, and
blessed loss and can return `dtz`, `dtm`, and `dtc` when known
[lila-tablebase][lila-tablebase].  FIDE separately makes 50 moves claimable and
75 moves automatic, with checkmate taking precedence on the last move
[fide2023][fide2023].  A theorem or exercise that says only “winning” without
declaring these choices is underspecified.

## Counterexample-and-repair loop

For one material/motif slice:

1. generate structurally valid positions, then filter by the exact legality or
   reachability assumptions used in the theorem;
2. canonicalize board symmetries that preserve the claim;
3. query a pinned tablebase version/endpoint and retain the full response
   provenance;
4. compare labels with `H`;
5. order disagreements by piece count, distance features, then canonical FEN,
   so “smallest” is reproducible;
6. cluster mismatches by interpretable features, repair `H`, and rerun the
   entire slice;
7. reserve symmetry or feature strata for an unseen transfer test;
8. prove the final predicate or check an exhaustive certificate in Lean.

Bulk results should not be redistributed until the table data and API terms
have been checked.  The server code is open source, but that alone does not
establish a license for every underlying table file or a large derived corpus.

## Plumbing control: the rule of the square

The repository already proves the exact geometric rule of the square in
`Chess.Theory.PawnGeometry.ruleOfSquare`.  Its documentation correctly limits
the result to king distance and the promotion deadline; it does not claim a
complete KPK outcome because occupancy, opposition, king protection, and
rook-pawn cases remain [pawn-geometry][pawn-geometry].

That makes it an ideal non-novel control experiment:

```text
H(position) := the defending king is inside the tempo-adjusted pawn square
label(position) := tablebase WDL under the declared clock convention
```

The pipeline should recover minimal mismatches and group them into the missing
mechanisms.  If it cannot rediscover a compact account of this familiar gap,
it is not ready to claim a new theorem.  If it succeeds, move to a narrower
KPKP pawn-race or mutual-zugzwang family where the candidate rule is not already
standard textbook material.

## Publishable candidate

The best next target is not “classify all KPKP.”  It is a motif theorem such as:

> Within a declared two-pawn race corridor, a finite predicate of king
> deadlines, capture order, and opposition exactly characterizes the positions
> whose WDL changes with side to move.

The predicate must be chosen after the control pilot and literature check; the
sentence above is a research template, not a claimed theorem.  Mutual
zugzwangs have already been mined in six-man data [bleicher-haworth2010][bleicher-haworth2010],
and combinatorial-game constructions have produced novel chess zugzwangs
[elkies1999][elkies1999].  The novelty case therefore depends on the exact
family and structural characterization.

## From result to training material

Each repaired predicate exports:

- one rule card containing scope, cue, action, and metric;
- a positive/negative minimal pair for every clause;
- a “folklore trap” card built from the smallest original counterexample;
- unseen positions stratified by the exception feature;
- a digest linking every answer to the enumeration and Lean artifact.

The trainer should ask “does the rule apply?” before “what move wins?”  This
measures whether the player learned the boundary.  Tablebase-optimal moves may
then grade exact execution, while a separate engine score measures practical
robustness.  Haworth's analysis of constrained optimization is a warning that a
single optimal metric need not maximize practical winning chances
[haworth2000][haworth2000].

## Stop conditions

Stop the research route when:

- the repaired rule needs an exception list comparable to the state table;
- canonical counterexamples change under an unacknowledged clock metric;
- the result duplicates a published strategy or classification;
- teachers/players cannot apply it to held-out positions;
- the Lean layer merely asserts imported labels without checking their relation
  to chess rules.

## Local References

- **bain1994** — Michael Bain and Stephen Muggleton, “Learning Optimal Chess Strategies,” in *Machine Intelligence 13: Machine Intelligence and Inductive Learning*, Oxford University Press, 1994, 291–309. DOI 10.1093/oso/9780198538509.003.0012.
- **guid2010** — Matej Guid, Martin Možina, Aleksander Sadikov, and Ivan Bratko, “Deriving Concepts and Strategies from Chess Tablebases,” in *Advances in Computer Games*, LNCS 6048, Springer, 2010, 195–207. DOI 10.1007/978-3-642-12993-3_18.
- **thompson1986** — Ken Thompson, “Retrograde Analysis of Certain Endgames,” *ICCA Journal* 9(3), 1986, 131–139. DOI 10.3233/ICG-1986-9302.
- **lila-tablebase** — Lichess, `lila-tablebase` README and HTTP API documentation, GitHub repository, current snapshot inspected 14 July 2026.
- **op1** — Lichess, `op1`: probe for Marc Bourzutschky's partial eight-piece tablebase, GitHub repository, current snapshot inspected 14 July 2026.
- **fide2023** — International Chess Federation, *FIDE Laws of Chess Taking Effect from 1 January 2023*, Articles 9.3 and 9.6, approved 7 August 2022.
- **pawn-geometry** — `Chess/Theory/PawnGeometry.lean`, geometric pawn-square theorem, local formalization repository snapshot inspected 14 July 2026.
- **bleicher-haworth2010** — Eiko Bleicher and Guy McC. Haworth, “6-Man Chess and Zugzwangs,” in *Advances in Computer Games*, LNCS 6048, Springer, 2010, 123–135. DOI 10.1007/978-3-642-12993-3_12.
- **elkies1999** — Noam D. Elkies, “On Numbers and Endgames: Combinatorial Game Theory in Chess Endgames,” arXiv:math/9905198, 1999.
- **haworth2000** — Guy McC. Haworth, “Strategies for Constrained Optimisation,” *ICGA Journal* 23(1), 2000, 9–20. DOI 10.3233/ICG-2000-23103.

[bain1994]: https://doi.org/10.1093/oso/9780198538509.003.0012
[guid2010]: https://doi.org/10.1007/978-3-642-12993-3_18
[thompson1986]: https://doi.org/10.3233/ICG-1986-9302
[lila-tablebase]: https://github.com/lichess-org/lila-tablebase/blob/main/README.md
[op1]: https://github.com/lichess-org/op1
[fide2023]: https://handbook.fide.com/chapter/E012023
[pawn-geometry]: ../../../Chess/Theory/PawnGeometry.lean
[bleicher-haworth2010]: https://doi.org/10.1007/978-3-642-12993-3_12
[elkies1999]: https://arxiv.org/abs/math/9905198
[haworth2000]: https://doi.org/10.3233/ICG-2000-23103
