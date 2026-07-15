# Structure of the pinned opening graph

This report analyzes the exact `all.tsv` snapshot documented in this directory.
It separates three objects that an opening explorer can otherwise conflate:

1. a **row-prefix occurrence** is one prefix as it occurs in one named row;
2. a **history** is one distinct UCI move word in the prefix trie;
3. a **node** is one modeled repetition state, merging transposed histories.

The first two retain move order. The third retains exactly board placement,
side to move, castling rights, and legally effective en passant. It omits move
clocks and earlier repetitions.

## Exact size and quotient fibres

| Quantity | Count |
|---|---:|
| Named rows | 3,803 |
| Plies across rows | 36,840 |
| Row-prefix occurrences, including one root per row | 40,643 |
| Distinct move-word histories, including the root | 8,646 |
| Prefix-trie edges | 8,645 |
| Distinct raw-en-passant four-field states | 7,921 |
| Distinct repetition nodes | 7,848 |

The endpoint quotient therefore removes 798 excess history vertices. There are
570 non-singleton fibres, containing 1,368 histories in total:

| Histories in one node | Nodes |
|---:|---:|
| 2 | 445 |
| 3 | 69 |
| 4 | 31 |
| 5 | 15 |
| 6 | 1 |
| 7 | 6 |
| 8 | 3 |

The maximum observed multiplicity is eight. One such node is the Semi-Slav
structure reached after:

```text
1. d4 d5 2. c4 c6 3. Nc3 Nf6 4. e3 e6 5. Nf3
```

Seven other corpus move orders reach the same repetition state.

Among the 3,803 named row endpoints, 259 have an alternate route somewhere
among all corpus prefixes. Their full fibre-size distribution is:

```text
1 -> 3544    2 -> 193    3 -> 34    4 -> 17
5 -> 7       6 -> 1      7 -> 4     8 -> 3
```

## More than independent-move commutation

Many transpositions are familiar move-order diamonds. For example:

```text
1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. O-O Nf6
1. e4 e5 2. Nf3 Nc6 3. Bc4 Nf6 4. O-O Bc5
```

The corpus also contains coalescence that cannot be generated merely by
permuting the same moves:

```text
1. e4 c6 2. d4 d5 3. Nc3 dxe4 4. Nxe4 Nd7
1. e4 c6 2. d4 d5 3. Nd2 dxe4 4. Nxe4 Nd7
```

The white knight travels through different squares before arriving on e4, so
the raw move lists are not permutations. Lean checks both legality, complete
endpoint equality, and non-permutation in
`Chess/Theory/OpeningCorpusExamples.lean`.

Exactly three observed repetition nodes occur at more than one ply depth. One
pair is:

```text
1. d4 Nf6 2. c4 e6 3. Nf3 d5 4. g3 Be7 5. Bg2 O-O
6. O-O Nbd7 7. Qc2 c6 8. Bf4

1. d4 Nf6 2. c4 e6 3. Nf3 d5 4. g3 Bb4+ 5. Bd2 Be7
6. Bg2 O-O 7. O-O c6 8. Qc2 Nbd7 9. Bf4
```

The Bogo-style bishop detour disappears from the endpoint. The paths have 15
and 17 plies, respectively. Thus even this finite opening quotient is not
naturally graded by ply count.

This suggests two algebraic mechanisms in opening theory:

- **commutation**, where sufficiently independent plans may change order;
- **coalescence**, where genuinely different paths later reach the same state.

A trace monoid generated only by commuting independent moves models the first
but not the second.

## The finite graph versus the chess graph

Deduplicating labelled trie edges by repetition-node endpoints gives:

| Quantity | Count |
|---|---:|
| Vertices | 7,848 |
| Unique directed edges | 8,052 |
| Edge fibres with multiple trie edges | 430 |
| Trie edges in those fibres | 1,023 |
| Maximum edge-fibre size | 8 |
| Self-loops | 0 |
| Directed cycles | 0 |

Tarjan analysis gives only singleton strongly connected components, and no row
revisits a repetition node. The corpus quotient is therefore a DAG. This is an
empirical property of these curated, truncated lines. It must not be promoted
to a theorem about chess: the full repetition graph contains legal cycles such
as `Nf3, ...Nf6, Ng1, ...Ng8`.

## Why effective en passant is exact

Across distinct histories, 1,418 positions record a raw en-passant target but
only 27 permit a legal en-passant capture. Normalizing ineffective targets
reduces 7,921 raw keys to 7,848 repetition nodes. Seventy-one nodes merge
multiple raw keys: 69 merge two alternatives and two merge three.

A simple valid merge is:

```text
1. c4 e5 2. e3
1. e3 e5 2. c4
```

The second endpoint records raw `c3`, while the first records no target. No
black pawn can legally capture on c3, so their legal futures agree.

Erasing en passant unconditionally would be wrong. The following endpoints
have the same board, turn, and castling rights:

```text
1. Nc3 e5 2. f4 exf4 3. e4
1. e4 e5 2. f4 exf4 3. Nc3
```

Only the first permits `...fxe3 e.p.`. The exact keys must differ, and Lean
checks both the common base fields and the effective-target distinction.

## Relation to prior quantitative work

The idea that move trees have transpositions is classical, not a discovery of
this project. François Labelle's [exact early-ply enumeration][labelle] uses the
same effective-en-passant convention. At ply 11, the published sequences give
2,097,651,003,696,806 legal histories, 726,155,461,002 distinct positions, and
1,994,236,773 positions reached by exactly one history
([histories][a048987], [positions][a083276], [singletons][a089957]). Thus the
mean fixed-depth fibre has about 2,888.71 histories and only 0.275% of position
fibres are singletons.

Those figures are not directly comparable to this report: Labelle exhausts
every legal word of one length, while this artifact is a sparse curated union
of named lines at varying lengths. The contrast is nevertheless informative.
The named corpus retains far fewer redundant move orders than the full legal
tree, but even it is not faithfully represented as a tree of positions.

A scoped literature search also found engine-search graphs and statistical
opening trees, but no source combining a complete fibre histogram for a named
opening corpus, an effective-en-passant differential measurement, and
machine-checked projection laws. That combination is a candidate contribution
of this project, not a claim that an exhaustive priority search is complete.

[labelle]: https://www.wismuth.com/chess/statistics-positions.html
[a048987]: https://oeis.org/A048987
[a083276]: https://oeis.org/A083276
[a089957]: https://oeis.org/A089957

## Names are occurrence metadata

The 3,803 rows contain 3,174 distinct names. There are 293 repeated-name
groups covering 922 rows, with maximum multiplicity fourteen. Of those names,
108 span multiple exact ECO codes and six span different ECO letter families.

All 3,803 named endpoint EPD values are unique. Consequently repeated names in
this artifact denote distinct positions, not duplicate routes to one endpoint:
a name is not a unique identifier for a repetition node. On the finite set of
named endpoints, the labeling happens to define a partial node-to-name function
because each endpoint occurs only once. That accidental factorization does not
make the name an intrinsic chess invariant. The schema attaches names to row
occurrences, and a database that combines other corpora or labels prefixes must
retain that provenance or explicitly aggregate labels over each node fibre.

The same caution applies to counts. This is a curated taxonomy of named lines,
not a sample of played games. Row and edge counts measure catalog density; they
do not measure popularity, winning chances, or move strength.

## Reproduction and assurance boundary

The primary analysis replayed every unique UCI prefix with `chess==1.11.2` and
grouped by:

```text
(board placement, turn, clean castling rights,
 legal en-passant target if one exists)
```

Raw-key analysis retained the nominal en-passant square instead. Directed-edge
and strongly-connected-component counts were then computed from adjacent trie
prefixes.

The Lean validator independently replays every row, and the Lean opening-graph
analyzer recomputes the core history, repetition-node, fibre, depth, and raw-key
counts using the proved exact `RepetitionKey`. The read-only `analyze.py` script
reproduces every count in this report with exactly `chess==1.11.2` and the
pinned input hash. The SCC calculation remains an executable empirical result,
not a Lean theorem.
