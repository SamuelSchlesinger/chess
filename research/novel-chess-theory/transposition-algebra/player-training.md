# Turning the relation basis into player knowledge

The 205-relation rooted basis can organize duplicated opening study, but it
cannot turn 8,646 catalog prefixes into 205 things to memorize. It compresses
the concrete *proof that root routes merge* in one projected graph, not the
chess content at 7,848 distinct positions. Player value requires three separate
compression claims:

1. **state deduplication:** schedule one next-move card per exact position,
   removing duplicate route occurrences;
2. **relation compression:** explain all route mergers with a basis rather
   than listing every transposed history pair;
3. **schema compression:** teach several basis equations as one reusable chess
   idea, such as “develop either knight around the same waiting move.”

The corpus establishes only the structural opportunity behind (1): for
observed decision prefixes, White has `3,091` history nodes versus `2,741`
repetition-key nodes, while Black has `3,102` versus `2,736`. These are
potential duplicate records, not yet scheduled cards or measured study time.
The conditional 205-chord theorem establishes (2) for concrete root-path
equations. Whether either saves human review after relation and deviation cards
are added—and whether (3) exists—remains empirical.

## What the current opening drill still gets wrong

The trainer in `engine/` embeds 21 SAN main lines for free play. Its
`src/bin/chess-trainer/book.rs` indexes that book by exact UCI sequence prefix
and follows the earliest matching line. Consequently, a legal transposition
into a known position can be reported as “out of book.” A separate six-card
diagnostic pilot now has persistent state keyed by card and semantic content
version, but its cards come from individual game occurrences; it does not yet
merge equal positions reached by two routes.

The replacement should use two indexes, not one:

```text
position index: exact opening key -> repertoire move, explanation, evaluations
route index:    path prefix       -> deviations encountered before a merge
```

The position key must use semantic equality—placement, turn, historical castling
rights, and effective en passant—with equality confirmation. A 64-bit
Polyglot/Zobrist value is an efficient lookup accelerator, not an identity
proof. The complete `Game` remains available for draw-rule history.

Opening books as position DAGs are established in computer-chess work
[fishburn18][fishburn18], and position-keyed transposition-aware training is
already implemented by Chess Position Trainer [cpt14][cpt14] and advertised by
GambitLab [gambitlab26][gambitlab26]. The experimental point here is narrower:
add explicit minimum-basis relation and route-deviation prompts, then test them
against a strong position-keyed baseline.

## What can safely be merged

Once two opening routes reach the same exact repetition position, their legal
moves agree. Any annotation or static evaluation proved to factor through that
key can therefore be shared in one **node record**, regardless of opening name
or incoming move order. Evaluations that depend on the half-move clock,
repetition history, or route-conditioned evidence require a richer key. On the
pinned catalog, the broad quotient removes 798 duplicate prefix occurrences,
but those prefixes are not player cards.

The corpus-relative decision-node opportunity is the more relevant quantity:

| Learner side | History decision nodes | Repetition-key decision nodes | Structural reduction |
|---|---:|---:|---:|
| White | 3,091 | 2,741 | 350 (11.32%) |
| Black | 3,102 | 2,736 | 366 (11.80%) |

Even these are database-node counts. Actual savings must be recomputed after
instantiating node, relation, and deviation review records and measuring their
time cost.

Those figures are properties of a sparse name catalog, not forecasts for a
personal repertoire. A system opening with many interchangeable development
orders may compress more; a forcing repertoire may compress less.

What must not be merged is the knowledge needed *before* the common endpoint.
One route may allow an opponent deviation that the other avoids. That is a
property of the adversarial branches exposed along the route, not of the final
node. The trainer should keep a **deviation card** at the earliest branch where
the distinction matters.

## Four relation-aware exercise types

### 1. Position retrieval

Show a board reached through a randomly chosen incoming route and ask for the
same repertoire move. Schedule by exact position key, so success through one
route strengthens the shared node rather than a duplicate string.

### 2. Transposition recognition

Show two short tails from their last common prefix and ask:

- do they reach the same position?
- if so, which familiar position is it?
- which move completes the transposition?

Use the 205 chord equations as the candidate pool, but keep the buckets
disjoint: begin with 84 direct alternating braids at most three plies per side;
then the two other relations at most three plies per side; then 52 relations
with four or five plies per side. The remaining 67 are longer and should not
enter a player curriculum without a separate explanation.

### 3. Move-order contrast

For a chord, stop before the merge and present the opponent's plausible
deviations on each route. Ask which route admits or avoids the target sideline.
This turns an algebraic equality into the practical statement “same eventual
position, different options on the way.” Engine evaluation and played-game
frequency belong on this card; endpoint equality alone does not rank routes.

### 4. Substitution and detour diagnosis

Reserve the four syntactic non-permutation candidates (`R032`, `R100`, `R142`,
and `R164` in the certificate) for explicit human classification before study.
Once the proposed route-substitution or detour account has been checked by a
strong player, ask the learner to identify:

- which piece took a different route;
- which forcing move made the route coalesce;
- which two plies form a removable detour;
- whether the detour was harmless, useful move-order probing, or a concession.

This is more likely to build transferable pattern knowledge than rehearsing
the entire SAN sequence again.

## A human-readable canonical repertoire

The lexicographically least shortest-path arborescence is a reproducible
mathematical baseline, not automatically the right personal backbone. A future
optimizer should choose the *whole arborescence* to minimize a declared global
review objective; choosing a locally cheap incoming edge at each node need not
minimize the lengths or explanations of all induced chord relations. Candidate
features include:

```text
review cost = personal unfamiliarity
            + expected error consequence
            + explanation length
            - encounter-value benefit.
```

For a fixed backbone, every non-tree edge becomes a relation candidate into a
canonical position. Changing the arborescence leaves the concrete minimum count
205 but changes which equations are short and recognizable. Any optimization
claim must specify the objective, constraints, and algorithm and report
sensitivity to the selected backbone. The target should be measured review
burden, not relation count.

A concise human guide generated from the graph should present:

1. the personal backbone line;
2. the position's pawn structure, piece placement, and plan;
3. incoming transposition arrows, expressed only by divergent tails;
4. route-specific opponent deviations before each arrow rejoins;
5. one model game or tactical motif when it explains the position;
6. a confidence boundary: exact identity, engine evidence, game frequency, or
   human strategic judgment.

The relation basis can choose and deduplicate this material, but it cannot
write good positional explanations without chess analysis.

## Scheduling and prioritization

Use three independent review records:

```text
NodeReview(position_key, repertoire_side)
RelationReview(chord_id, direction)
DeviationReview(route_prefix, opponent_move)
```

This prevents a failed route-recognition exercise from resetting an otherwise
well-known next move. Priority should combine:

```text
personal encounter frequency
  x probability of the opponent branch
  x consequence of an error
  x observed recall weakness.
```

Spacing improves long-term retention across many verbal-learning experiments
[cepeda06][cepeda06], and retrieval practice can improve transfer to new
questions in laboratory tasks [butler10][butler10]. Neither result establishes
that the proposed chess cards outperform ordinary line drilling. Chase and
Simon's chess experiments support the importance of structured positional
chunks [chase73][chase73], but do not validate this particular quotient. Those
sources motivate an experiment rather than substitute for one.

## Two-stage evaluation in the existing workflow

### Stage A: 30-position N-of-1 feasibility pilot

Build a 30-position personal micro-repertoire with at least ten
transpositions. Partition it by **disjoint transposition component**, so two
conditions never expose the same endpoint or a relation that teaches another
condition's held-out route. Counterbalance opening families and assign matched
components to:

- **sequence condition:** ordinary exact-prefix next-move cards;
- **node condition:** position-keyed cards with incoming routes randomized;
- **relation condition:** node cards plus a short relation/deviation prompt.

Use the same scheduler and review-time budget for four weeks. Preregister one
primary feasibility outcome—successful unseen-route presentation without key,
scheduling, or content errors—and report next-move recall, deviation
recognition, review count, seconds, and route mistakes descriptively. Subsequent
game quality is exploratory. With roughly ten items per condition and one
learner, absence of a performance difference is **not** a falsifier and supports
no population-level claim. This stage can reject unusable plumbing or exercises,
not the cognitive hypothesis.

### Stage B: controlled effect study

If Stage A works, power a repeated-measures study from a declared smallest
effect on one primary outcome: delayed next-move recall through an unseen legal
transposition. Use transposition components as assignment clusters, prevent
cross-condition endpoint overlap, counterbalance condition order and opening
family, preserve an untouched unseen-route test, and preregister the participant
count, analysis model, multiplicity policy, and missing-data handling.

The main comparison is **position-keyed node review versus node review plus
relation/deviation prompts**, because position-keyed training already exists.
Exact-prefix drilling may remain a secondary implementation baseline. A
relation-aware trainer is useful only if it reduces review burden or improves
transfer without increasing route mistakes. If it only makes the backend
cleaner, keep the graph representation and hide the algebra from the player.

## Practical recommendation toward a 2000-level repertoire

Use the algebra as infrastructure, not as the syllabus. For a player working
toward roughly 2000:

- index endpoint-invariant node records by exact repetition key immediately;
- expose only short, frequent, high-consequence relation cards;
- prioritize deviations found in the player's own games over obscure named
  catalog routes;
- attach plans and tactical motifs to canonical positions;
- let the engine grade moves and consequences, but require human-readable
  explanations for every retained relation;
- measure whether relation cards transfer before expanding beyond the
  micro-repertoire.

This is the most defensible hypothesis connecting the rooted path theorem to
stronger chess: potentially less duplicate review, more recognition of shared
positions, and exact warnings about where move orders stop being
interchangeable. Only the staged evaluation can establish the player benefit.

## Local References

[butler10]: sources.md#butler10
[cepeda06]: sources.md#cepeda06
[chase73]: sources.md#chase73
[cpt14]: sources.md#cpt14
[fishburn18]: sources.md#fishburn18
[gambitlab26]: sources.md#gambitlab26
