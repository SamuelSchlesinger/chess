# Pilot: a minimum relation basis for the pinned opening graph

This pilot asks a falsifiable structural question:

> Can the 798 redundant opening histories be explained by a much smaller set
> of local route equations, and how many of those equations are genuine move
> reorderings rather than substitutions or detours?

The answer on the pinned corpus is unusually crisp: **205 path equations are
both sufficient and cardinality-minimal for the projected graph**. Of those,
201 preserve length and the multiset of UCI plies; three are same-length route
substitutions and one is a length-changing detour relation. This does not yet
show that 201 equations have a compact *human* explanation.

## Data and assurance boundary

The input is the repository's pinned snapshot of the Lichess opening-name
dataset. Upstream describes it as an aggregated, curated dataset of opening
names released under CC0 [lichess-openings][lichess-openings]. It is a taxonomy,
not a sample of played games, so no count below measures popularity or move
quality.

The validator uses `chess==1.11.2`, verifies the input SHA-256, legally replays
every distinct UCI prefix, and groups positions by piece placement, player to
move, castling rights, and legally effective en passant. This matches FIDE's
repetition-position distinction [fide23][fide23] and the Lean
`RepetitionKey`; it does not substitute a collision-prone hash for exact
equality. Zobrist's hashing method is the classical engine mechanism for fast
transposition lookup, but hashing and semantic identity are distinct
[zobrist90][zobrist90].

Reproduce the result with:

```sh
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/transposition-algebra/data/classify_transpositions.py
```

The checked output is preserved in
[`data/classify_transpositions.output.txt`](data/classify_transpositions.output.txt).

## Three sizes that must not be conflated

| Object | Count |
|---|---:|
| Distinct legal UCI histories, including root | 8,646 |
| Exact repetition-position vertices | 7,848 |
| Distinct projected directed edges | 8,052 |

The endpoint map has 570 non-singleton history fibers and total excess
`8,646 - 7,848 = 798`. That counts duplicate *history vertices*. It is not the
number of independent mergers in the graph.

A projected vertex is a **primitive merge node** here when its distinct
projected indegree is greater than one. There are only 193:

| Projected indegree | Merge nodes | Incoming-edge excess |
|---:|---:|---:|
| 2 | 181 | 181 |
| 3 | 12 | 24 |
| **Total** | **193** | **205** |

The other `570 - 193 = 377` non-singleton fibers have indegree one. Their
multiple histories are propagated consequences of a transposition earlier in
the path, not fresh merges. This is the first practically meaningful
compression: teaching every transposed endpoint separately repeats downstream
consequences.

## Why 205 is sufficient

The projected graph is rooted and connected. Select one incoming edge for
every non-root vertex to form a rooted arborescence. Its 7,847 edges give one
canonical root path to each vertex. The remaining

```text
8,052 - 7,847 = 205
```

edges are chords. For a chord `e : u -> v`, record the equation

```text
canonical(u) ; e  ~=  canonical(v).
```

Path induction normalizes every legal root path to the canonical path of its
endpoint. Thus these 205 equations generate every endpoint equality in the
finite projected graph, including legal paths obtained by recombining recorded
edges even when that exact move word was not a catalog row.

The script constructs a genuinely shortest, lexicographically least path in
the projected graph, not merely the shortest catalog history. This matters:
merging vertices can make new edge recombinations available.

## Why 205 is a cardinality lower bound

The underlying connected undirected graph has binary cycle-space rank

```text
|E| - |V| + 1 = 8,052 - 7,848 + 1 = 205.
```

Every sound parallel-path equation has a cycle as its boundary. If a set of
equations generates all root-path endpoint equalities, those boundaries must
span every fundamental chord cycle after linearization over `F_2`. It
therefore contains at least 205 equations. The arborescence basis attains the
bound.

This lower bound is conditional on retaining the graph's move arrows and
counting each parallel-path equation as one generator. A macro language could
describe many equations with one parameterized schema; conversely, a linear
cycle basis by itself does not provide directed, legal path rewrites. The
directed normalization proof is essential.

## How much is ordinary reordering?

Any theory generated only by permuting atomic UCI plies preserves path length
and the multiset of ply labels. The pilot uses that pair as a necessary “trace
signature.” It does not mistake signature equality for proof of trace
equivalence.

| Measurement | Count |
|---|---:|
| Non-singleton fibers with one trace signature | 557 |
| Non-singleton fibers with multiple signatures | 13 |
| Endpoint excess potentially explainable by permutations | 785 |
| Endpoint excess requiring stronger relations | 13 |

In the minimum 205-chord shortlex basis:

| Basis relation kind | Count |
|---|---:|
| Same length and same UCI multiset | 201 |
| Same length but different UCI multiset | 3 |
| Different length | 1 |

Thus four primitive non-permutation relations propagate to thirteen mixed
fibers. The Lean Caro-Kann witness exhibits route substitution, and its
Catalan/Bogo witness exhibits detour cancellation.

## The alternating-braid under-approximation

Because adjacent single plies cannot commute across the turn token, the pilot
also looks for the parity-preserving local rule `abc <-> cba`. It currently
counts a braid only when both complete histories are catalog prefixes and the
two three-ply prefixes have equal repetition endpoints.

| Catalog-visible braid statistic | Count |
|---|---:|
| Direct braid relations | 296 |
| Endpoint fibers touched | 241 |
| History excess collapsed by their closure | 296 |

This closure explains 296 of 798 history duplicates using only intermediates
visible in the sparse name catalog. It is deliberately a lower bound on legal
braid connectivity: an absent intermediate line is not an illegal line.

For the 205 shortest-basis relations, 84 are themselves one alternating braid.
After canceling the longest common prefix, 86 relations use at most three
plies per side and 138 use at most five. The two tails contain 2,112 UCI tokens
in total (10.30 per relation on average); the longest one-sided tail is 17
plies. A minimum-cardinality basis is therefore not automatically a minimum
cognitive-load basis.

## The next, smallest falsifying experiment

The next pilot should work only on the 205 basis pairs:

1. enumerate legal alternating-braid neighbors, allowing intermediate
   histories absent from the catalog;
2. compute the shortest braid derivation for each of the 201
   signature-compatible pairs;
3. report how many need 0–1, 2–4, 5–8, or more than 8 braids;
4. preserve a machine-checkable witness for each successful derivation;
5. send the four non-permutation equations directly to the
   substitution/detour classifier.

This can kill the proposed local algebra cheaply. If fewer than 70% of the 201
compatible chords have a derivation of at most four recognizable braids, then
alternating braids are not a useful explanatory basis even though they remain
mathematically sound. If coverage is high, cluster the braid instances by
piece plan and opponent waiting move, then test whether those schemas recur in
played-game data.

## Relation to prior work and novelty posture

Position-indexed transposition tables are classical [zobrist90][zobrist90],
and Fishburn explicitly represents a chess opening book as a position DAG
[fishburn18][fishburn18]. Search research also warns that merging a game graph
can be unsound when values depend on path history—the graph-history interaction
problem [kishimoto04][kishimoto04]. None of those facts is novel here.

A scoped search found no prior source that gives a cardinality-minimal typed
path presentation of a named chess-opening graph, separates its generators
into alternating braids/substitutions/detours, and turns the basis into a
human-training intervention. That combination is a plausible research
contribution, not a priority claim. The decisive novelty test is whether the
205 equations admit reusable chess explanations and improve transfer to unseen
move orders.

## Local References

- <a id="fide23"></a> **fide23** — International Chess Federation, *FIDE Laws of Chess taking effect from 1 January 2023*, Article 9.2.3, approved 7 August 2022. [Official handbook](https://handbook1090.fide.com/chapter/E012023) (accessed 2026-07-14).
- <a id="fishburn18"></a> **fishburn18** — John P. Fishburn, “Search-Based Opening Book Construction,” *ICGA Journal* 40(1), 2018, pp. 2–14. [DOI](https://doi.org/10.3233/ICG-180039).
- <a id="kishimoto04"></a> **kishimoto04** — Akihiro Kishimoto and Martin Müller, “A General Solution to the Graph History Interaction Problem,” *Proceedings of AAAI 2004*, 2004, pp. 644–649. [AAAI paper](https://s.aaai.org/Papers/AAAI/2004/AAAI04-102.pdf).
- <a id="lichess-openings"></a> **lichess-openings** — Lichess contributors, *chess-openings: An Aggregated Data Set of Chess Opening Names*, pinned locally at commit `292fd0468068f58bb244f7fe1c3e573e493c3c53`, CC0. [Official repository](https://github.com/lichess-org/chess-openings) (accessed 2026-07-14).
- <a id="zobrist90"></a> **zobrist90** — Albert L. Zobrist, “A New Hashing Method with Application for Game Playing,” *ICCA Journal* 13(2), 1990, pp. 69–73; reprint of University of Wisconsin Technical Report 88, 1970. [DOI](https://doi.org/10.3233/ICG-1990-13203).

[fide23]: https://handbook1090.fide.com/chapter/E012023
[fishburn18]: https://doi.org/10.3233/ICG-180039
[kishimoto04]: https://s.aaai.org/Papers/AAAI/2004/AAAI04-102.pdf
[lichess-openings]: https://github.com/lichess-org/chess-openings
[zobrist90]: https://doi.org/10.3233/ICG-1990-13203
