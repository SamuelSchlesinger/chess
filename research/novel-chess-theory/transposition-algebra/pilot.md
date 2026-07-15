# Pilot: a minimum concrete rooted relation basis for the pinned opening graph

This pilot asks a falsifiable structural question:

> Can the 798 duplicate opening-prefix occurrences be normalized by a much
> smaller set of concrete rooted route equations, and what syntactic tests can
> separate plausible reorderings from relations needing stronger explanations?

The answer on the pinned corpus is unusually crisp but conditional: **205
concrete path equations are sufficient and cardinality-minimal for endpoint
equality on all root-originating paths of the fixed, edge-retaining projected
graph**. Of the selected shortest-shortlex basis, 201 preserve length and the
multiset of UCI plies, three have equal length but different multisets, and one
has unequal lengths. Those are syntactic classes, not yet a complete
braid/substitution/detour derivation or a compact *human* explanation.

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
[`data/classify_transpositions.output.txt`](data/classify_transpositions.output.txt),
and the complete deterministic basis is
[`data/rooted_path_basis.json`](data/rooted_path_basis.json). A default run
reconstructs both artifacts and fails on byte-level drift. After reviewing an
intentional code or data change, refresh them explicitly with
`--write-artifacts`.

## Three sizes that must not be conflated

| Object | Count |
|---|---:|
| Distinct legal UCI histories, including root | 8,646 |
| Exact repetition-position vertices | 7,848 |
| Distinct projected directed edges | 8,052 |

The endpoint map has 570 non-singleton history fibers and total excess
`8,646 - 7,848 = 798`. That counts duplicate *prefix occurrences*, including
the root, both sides to move, and terminal prefixes. It is neither the number
of independent mergers nor the number of scheduled player cards.

Restricting to observed prefixes with a recorded continuation on the learner's
side gives a more relevant, still corpus-relative structural opportunity:

| Learner side | History decision nodes | Repetition-key decision nodes | Reduction |
|---|---:|---:|---:|
| White | 3,091 | 2,741 | 350 (11.32%) |
| Black | 3,102 | 2,736 | 366 (11.80%) |

These remain database nodes, not instantiated node/relation/deviation cards or
evidence of reduced study time.

A projected vertex is a **primitive merge node** here when its distinct
projected indegree is greater than one. There are only 193:

| Projected indegree | Merge nodes | Incoming-edge excess |
|---:|---:|---:|
| 2 | 181 | 181 |
| 3 | 12 | 24 |
| **Total** | **193** | **205** |

The other `570 - 193 = 377` non-singleton fibers have indegree one. Their
multiple histories are propagated consequences of a transposition earlier in
the path, not fresh merges. This is the first structurally relevant compression
signal: a position-keyed representation can share those downstream
consequences, although no player-training saving follows without instantiated
review records.

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
endpoint. Thus these 205 equations generate every endpoint equality among
root-originating paths in the finite projected graph, including legal paths
obtained by recombining recorded edges even when that exact move word was not a
catalog row. They do not by themselves identify arbitrary parallel paths whose
source is a non-root vertex.

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

This lower bound is conditional on retaining every concrete graph move arrow,
generating every root-path endpoint equality, and counting each concrete
parallel-path equation as one generator. It is not a lower bound on hash-table
deduplication of the 798 observed prefix occurrences, a full path-category
presentation, or a macro language in which one parameterized schema describes
many instances. Conversely, a linear cycle basis by itself does not provide
directed, legal path rewrites. The directed rooted normalization proof is
essential.

## Executable certificate

The certificate records all 205 chord equations, not only their count. Each
record contains stable exact source and target keys, the chord move, complete
left and right UCI paths, divergent tails, both trace signatures, the syntactic
classification, and the direct-braid flag. The validator now:

1. replays every recombined canonical path and checks its key;
2. replays both sides of every relation and checks exact endpoint equality;
3. checks that the arborescence reaches all 7,848 vertices;
4. verifies that every mod-2 equation boundary contains exactly its own
   non-tree edge, giving a triangular independence certificate; and
5. compares the reconstructed JSON and output byte-for-byte with the committed
   artifacts.

The current certificate SHA-256 is recorded in the checked output. The generic
rooted theorem and its connection to this certificate remain prose-checked;
formalizing that checker in Lean is future work, not a completed claim.

## Which relations pass a necessary reordering test?

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

| Syntactic basis class | Count |
|---|---:|
| Same length and same UCI multiset; derivation unclassified | 201 |
| Same length but different UCI multiset | 3 |
| Different length | 1 |

Thus four selected chord equations cannot be generated solely by permutations
of their UCI tokens and propagate to thirteen mixed-signature fibers. The four
certificate records make those claims auditable:

| ID | Syntactic class | Divergent UCI tails |
|---|---|---|
| `R032` | unequal length | `f8b4 c1d2 b4e7 f1g2 b8d7 e1g1 e8g8 d1c2 c7c6 d2f4` versus `f8e7 f1g2 e8g8 e1g1 b8d7 d1c2 c7c6 c1f4` |
| `R100` | same length, different multiset | `b1d2 d5e4 d2e4` versus `b1c3 d5e4 c3e4` |
| `R142` | same length, different multiset | `b1d2 d5e4 d2e4` versus `b1c3 d5e4 c3e4` |
| `R164` | same length, different multiset | `b4c5 d2d4 e5d4 e1g1 d7d6 c3d4 c5b6` versus `b4a5 d2d4 e5d4 e1g1 d7d6 c3d4 a5b6` |

Manual chess inspection supports “route substitution” for `R100`, `R142`, and
`R164`, and a Bogo-style bishop detour for `R032`, but the executable classifier
deliberately retains only the syntactic labels. The existing Lean Caro-Kann
theorem proves the same knight-route mechanism as `R142` after a shared
`...Nd7` extension; it is not an exact theorem about certificate record
`R142`. The Lean Catalan/Bogo theorem proves an analogous unequal-length bishop
detour under a different canonical prefix; it is not an exact theorem about
`R032`. No current Lean theorem certifies `R100` or `R164`. These are therefore
mechanism witnesses, not formal certification of the four-record decomposition.
The `201/3/1` split can also change when the arborescence changes even though
the minimum total 205 does not.

## The alternating-braid under-approximation

Because adjacent single plies cannot commute across the turn token, the pilot
also looks for the parity-preserving local rule `abc <-> cba`. It records a
contextual application only when both complete histories are catalog prefixes
and the two local three-ply prefixes have equal repetition endpoints. The
certificate additionally deduplicates those applications at their local prefix
equation.

| Catalog-visible braid statistic | Count |
|---|---:|
| Distinct local prefix relations | 87 |
| Complete-history contextual applications | 296 |
| Suffix-propagated applications beyond the local count | 209 |
| Maximum contexts for one local relation | 16 |
| Endpoint fibers touched | 241 |
| History excess collapsed by their closure | 296 |

This closure connects 296 of 798 duplicate prefix occurrences using only
intermediates visible in the sparse name catalog. The 296 is an application
count, not a generator count: one of the 87 local equations may act in several
shared suffix contexts. It remains a lower bound on legal braid connectivity:
an absent intermediate line is not an illegal line.

For the 205 shortest-basis relations, 84 are themselves one alternating braid.
After canceling the longest common prefix, 86 relations use at most three
plies per side and 138 use at most five. The two tails contain 2,112 UCI tokens
in total (10.30 per relation on average); the longest one-sided tail is 17
plies. A minimum-cardinality basis is therefore not automatically a minimum
cognitive-load basis.

## The next, smallest classification experiment

The next pilot should work only on the 205 basis pairs:

1. enumerate legal alternating-braid neighbors, allowing intermediate
   histories absent from the catalog;
2. compute the shortest braid derivation for each of the 201
   signature-compatible pairs;
3. report how many need 0–1, 2–4, 5–8, or more than 8 braids;
4. preserve a machine-checkable witness for each successful derivation;
5. send the four syntactic non-permutation candidates directly to the
   substitution/detour classifier.

This can reject the proposed local algebra cheaply under a preregistered pilot
rule. A provisional rule is to stop if fewer than 70% of the 201 compatible
chords have a derivation of at most four recognizable braids. The `70%` and
four-braid cutoffs are design choices, not evidence-derived constants, so the
report must show sensitivity to nearby thresholds. If coverage is high,
cluster the braid instances by piece plan and opponent waiting move, then test
whether those schemas recur in played-game data.

## Relation to prior work and novelty posture

Position-indexed transposition tables are classical [zobrist90][zobrist90],
and Fishburn explicitly represents a chess opening book as a position DAG
[fishburn18][fishburn18]. Search research also warns that merging a game graph
can be unsound when values depend on path history—the graph-history interaction
problem [kishimoto04][kishimoto04]. None of those facts is novel here.

Nor is position-keyed opening training new. Chess Position Trainer documents a
position database, cross-opening transposition detection, flash-card training,
and scheduled review [cpt14][cpt14]. GambitLab currently advertises scheduled
cards keyed by board position specifically to avoid duplicate work at
transpositions [gambitlab26][gambitlab26]. These systems must be treated as the
player-product baseline, not rediscovered as a contribution.

A scoped search found no prior source that gives a cardinality-minimal concrete
rooted-path presentation of a named chess-opening graph, publishes its complete
certificate, and tests explicit relation/deviation prompts against an existing
position-keyed training baseline. That narrower combination is a plausible
research contribution, not a priority claim. The exploratory queries, included
sources, exclusions, and unresolved search space are recorded in
[`data/novelty_search.md`](data/novelty_search.md); an actual priority claim
would still require a systematic search with database result counts and broader
patent, thesis, product-history, and non-English coverage. The decisive
practical test is whether the 205 equations admit reusable chess explanations
and improve transfer beyond ordinary position-keyed review.

## Local References

[cpt14]: sources.md#cpt14
[fide23]: sources.md#fide23
[fishburn18]: sources.md#fishburn18
[gambitlab26]: sources.md#gambitlab26
[kishimoto04]: sources.md#kishimoto04
[lichess-openings]: sources.md#lichess-openings
[zobrist90]: sources.md#zobrist90
