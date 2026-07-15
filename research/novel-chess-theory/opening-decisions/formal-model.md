# Formal model and theorem program

This document makes familiar opening claims precise enough to prove or refute:
“this move order avoids the line,” “I can reuse this preparation,” “that move
commits to the structure,” and “this repertoire is smaller without being less
safe.”

## 1. State comes before notation

Let `S` be complete FIDE game states, including board, side to move, castling
rights, en-passant status, clocks, and the history needed for draw claims. Let

```text
step : S -> Move -> Option S
```

be the legal transition function. Every theorem about actual games starts here.
The current Lean project already models this level.

Let `q : S -> K` project a state to the exact repetition key: placement, side
to move, castling rights, and legally effective en passant. Two histories with
the same `K` may share a static position annotation, but not automatically an
annotation about move clocks, earlier repetitions, opening name, game
frequency, or the opponent who chose the route. The existing factorization
theorems in `Chess/Theory/OpeningDatabase.lean` are the right gate: merge an
observable only after proving it is endpoint-invariant.

This is stricter than using a Zobrist hash. A hash is an index and may collide;
the canonical state fields and equality decide identity.

## 2. Repertoires are finite policies

Fix a player `P`, a start state `s0`, and a horizon `H`. A deterministic
repertoire policy is a partial function

```text
pi : prepared P-to-move states -> legal move
```

A set-valued or randomized policy may retain several playable choices, but the
first implementation should optimize a deterministic backbone. At opponent
states, an **adversary scope** `A(s)` specifies the replies for which the
repertoire promises coverage.

Useful scopes are nested:

1. `A_all(s)`: every legal reply, the strongest and usually most expensive;
2. `A_supported(s; n, epsilon)`: moves with at least `n` observations or
   probability at least `epsilon` in a pinned cohort;
3. `A_personal(s)`: moves in a particular opponent's history;
4. `A_theory(s)`: moves present in a curated source.

Every reported result must name its scope. “Covered” without a scope is not a
mathematical claim.

The policy and scope induce a finite alternating game, an ordinary extensive-
form object [kuhn53][kuhn53]. Its paths retain complete move-order provenance
even when endpoints are later projected into `K`.

## 3. Prepared corridors, local words, and common scenarios

For each policy `pi`, a **prepared corridor** `C_pi` is a finite,
prefix-closed set of legal histories from `s0`. At our turns, `C_pi` follows
`pi`; at opponent turns it branches over the covered moves in `A`. Corridors
must be indexed by policy because two move orders normally contain different
own-move histories. Let `T` be an acceptable target set, such as a collection
of exact positions with attached plans.

For one fixed policy, projecting a play onto the opponent's moves gives useful
route-local diagnostics:

```text
LegalOppWord(pi, H)       opponent UCI words legal against pi through H
ExitWord(pi, C_pi, H)     local words whose induced play first leaves C_pi
UnsafeWord(pi, tau, H)    local words reaching declared evidence score < tau
```

These are finite languages, but languages from different policies are not
silently compared. Their legal alphabets depend on the states each policy
visits; the same UCI token can occur in different states, and a token can be
legal on only one route.

Cross-policy comparison instead fixes a finite, typed scenario space `Omega`.
One strong and unambiguous choice is a set of complete opponent policies over
the union of nonterminal opponent states reachable through horizon `H`:

```text
sigma : each reachable opponent state -> a legal move at that state
ExitScenario(pi, C_pi, Omega, H)
  = {sigma in Omega | play(pi, sigma) first leaves C_pi by H}
UnsafeScenario(pi, Omega, tau, H)
  = {sigma in Omega | play(pi, sigma) reaches evidence score < tau by H}
```

This convention treats “the opponent could play move `m` in this state” as a
typed event, not as an unqualified move word. Making a reply illegal by move
order can then be a genuine avoidance benefit, while the same global `sigma`
still supplies choices at every state reached on the competing route.

`Exit` measures preparation failure; `Unsafe` measures chess failure. They must
not be conflated: an unfamiliar move can be harmless, and a heavily memorized
line can be bad.

The corpus pilot is weaker. At each recorded opponent node it retains typed
one-deviation events `(opponent decision index, pre-deviation key, move, child
key)` and reports only sums of branch incidences. It does **not** compare their
unions by set inclusion and supplies no evidence of dominance. The production
experiment must construct `Omega` and the bounded scenario failure sets from
game and engine data.

## 4. Three notions of move-order dominance

Suppose `pi1` and `pi2` begin at the same state, are intended to reach the same
target class, and are compared for the same number of opponent decisions.

### Structural deviation dominance

Fix one `Omega`, threshold, and horizon; a corridor assignment taking every
candidate `pi` to its `C_pi`; and a declared preorder `CostLE` on cost records.
Define **weak structural dominance** by

```text
WeakStructural(pi1, pi2) iff
  ExitScenario(pi1, C_pi1, Omega) subseteq ExitScenario(pi2, C_pi2, Omega)
  and UnsafeScenario(pi1, Omega) subseteq UnsafeScenario(pi2, Omega)
  and CostLE(Cost(pi1), Cost(pi2)).
```

The direction matters: fewer common opponent scenarios cause failure. This
weak relation is a preorder. Define its strict part separately:

```text
StrictStructural(pi1, pi2) iff
  WeakStructural(pi1, pi2) and not WeakStructural(pi2, pi1).
```

The strict relation is irreflexive and transitive. Both relations are decidable
on a supplied finite graph. Scenario inclusion is more informative than
comparing branch-incidence counts, because two equally large failure sets can
contain very different dangers.

### Robust value dominance

Let `Sigma` be a class of complete opponent policies, each mapping every
opponent state to a legal move. `pi1` **weakly robust-value dominates** `pi2`
if, for every `sigma` in `Sigma`, the lower-bounded utility of the play under
`(pi1, sigma)` is no worse, and its study cost is no greater. Its strict part is
again weak dominance without the converse.

This definition compares one global opponent policy across routes, so it
remains meaningful when they visit different states. It is strong and will be
rare; the worst-case values

```text
Vmin(pi) = min_sigma Value(pi, sigma)
```

give a coarser preorder.

### Practical or distributionally robust dominance

Let `Q` be an ambiguity set around an empirical opponent model. Define

```text
Risk_Q(pi) = max_{q in Q} E_q[regret or failure loss under pi].
```

Then `pi1` **weakly practically dominates** `pi2` if it has no greater robust
risk, preparation cost, and endpoint regret. Define strict practical dominance
as the weak relation without its converse, rather than building strictness into
the preorder. Ambiguity sets prevent a small sample from being treated as
truth. Rectangular state-action uncertainty gives tractable dynamic programs
and an equivalent perfect-information zero-sum interpretation
[iyengar05][iyengar05], though an opponent model tied across many positions may
be non-rectangular and should not be silently approximated.

Dominance should normally be reported as a Pareto relation, not hidden inside
one arbitrary weighted sum.

## 5. Move-order risk

For each first opponent deviation event `e` before target coalescence, retain:

- cohort probability `p(e | state, rating, time control, date)`;
- sample size and uncertainty interval;
- threshold shortfall or declared failure loss `severity(e)` under a pinned
  protocol;
- whether the repertoire contains a response;
- whether the resulting state later coalesces with prepared material.

A simple descriptive score over these mutually exclusive first events is

```text
expected exposure = sum_e unconditional_first_event_mass(e) * severity(e).
```

The robust companion maximizes the expectation over an uncertainty set and
also reports the worst supported edge separately. Neither a win rate nor a raw
centipawn average is enough: player selection, rating, color, time control, and
survivorship all confound game outcomes.

Regret is reserved for comparing the prescribed player move with the best
engine move at the same player node. It is not used as a synonym for deviation
loss.

Engine scores are evidence, not proved minimax bounds. Use “pinned engine
estimate” unless a tablebase or proof-producing search supplies a certificate.

## 6. Commitment points

Let `Targets(s, h)` be the acceptable target classes that the player can still
force or keep viable from `s` within horizon `h`, against scope `A`. For a legal
player move `s -> t`:

```text
commitment_loss(s -> t) = Targets(s, h) \ Targets(t, h-1).
```

The move is a **commitment point** when this set is nonempty. Useful variants
separate:

- **structural commitment:** loses access to pawn-structure or opening-family
  targets;
- **castling commitment:** permanently loses a castling option;
- **risk commitment:** first exposes a severe opponent deviation;
- **preparation commitment:** enters a region with no covered policy;
- **probabilistic commitment:** makes a target sufficiently unlikely rather
  than impossible.

This gives a player an actionable sentence: “after this move, these destinations
are no longer available under the declared opponent scope.” It does not claim
the lost destinations were better.

## 7. Transposition hubs and leverage

Raw indegree or history-fibre size is only structural multiplicity. A useful
hub score must reward knowledge the player can actually reuse:

```text
Leverage(v) = incoming covered mass
              * downstream reusable study cost
              * recall-transfer probability
              - pre-hub route risk
              - route-specific exception cost.
```

All terms should also be shown separately. A node reached by eight catalog
routes may have no practical leverage if seven routes never occur, if the
player cannot steer toward them, or if dangerous deviations arise first.

The reusable object is not an opening name. It is a position decision and its
endpoint-invariant plan. Names, game counts, and move-order warnings remain
occurrence metadata attached to incoming routes.

## 8. Repertoire compression as covering

Let `D` be weighted demand scenarios: opponent histories or first-deviation
events the player wants to handle. A study unit `u` has cost `c(u)` and covers
the scenarios in which its response and explanation are reusable. The static
relaxation is weighted set cover:

```text
minimize   sum_u c(u) x_u
subject to sum_{u covers d} x_u >= 1   for every required demand d
           x_u in {0, 1}.
```

Weighted set cover has a classical greedy logarithmic approximation guarantee
[chvatal79][chvatal79]. A real repertoire adds chess constraints:

- prefix consistency from the initial state;
- one selected move at each deterministic policy node;
- coverage of opponent branches above a threshold;
- engine acceptability constraints;
- route-specific exceptions before transposition;
- a daily or total study budget;
- optional stylistic and pawn-structure constraints.

Thus **robust repertoire cover** is not merely “deduplicate FENs.” Ordinary set
cover embeds as the special case with independent study units and no path
constraints, suggesting a clean hardness theorem. The practical solver can use
integer programming or branch-and-bound; Lean can check a finite proposed cover
and, for small instances, a lower-bound certificate.

## 9. Cognitive cost and study units

One node is not automatically one unit of human memory. Separate at least:

```text
PositionDecision  repetition key + accepted move(s) + explanation
                  + complete-state occurrence provenance
PlanCard          endpoint-invariant plans, breaks, and piece placements
DeviationCard     route occurrence + opponent surprise + response
TransferCard      alternate route to a known position
CommitmentCard    choice between routes and the options each loses
```

Position and plan cards may merge across transpositions. Deviation and
commitment cards often cannot. This mixed representation turns the quotient
from an overaggressive database deduplication into a falsifiable cognitive
model.

## 10. Lean theorem backlog

The first theorem tranche should avoid engine or frequency claims:

1. bounded route-local opponent-language generation is finite and contains only
   legal histories;
2. typed scenario failure sets, weak structural dominance, and its strict part
   are decidable for finite scopes;
3. under fixed `Omega`, horizon, threshold, policy-indexed corridors, and cost
   preorder, weak structural dominance is a preorder and its strict part is
   irreflexive and transitive;
4. target-set loss is monotone along a fixed edge and commitment witnesses are
   checkable;
5. endpoint-invariant study units factor through the opening quotient, while
   route occurrences remain separate;
6. quotienting a finite corpus preserves total occurrence weight;
7. a reported repertoire cover really covers every demanded scenario;
8. static weighted set cover reduces to repertoire cover.

The next tranche can accept externally computed finite tables—frequencies,
engine estimates, uncertainty intervals—and prove that a particular route or
cover satisfies the stated finite inequalities. The theorem then means exactly
“given this pinned evidence,” not “this is objectively best chess.”

## Local References

- **[chvatal79]** Vašek Chvátal. “A Greedy Heuristic for the Set-Covering Problem.” *Mathematics of Operations Research* 4(3), 233–235, 1979. https://doi.org/10.1287/moor.4.3.233
- **[iyengar05]** Garud N. Iyengar. “Robust Dynamic Programming.” *Mathematics of Operations Research* 30(2), 257–280, 2005. https://doi.org/10.1287/moor.1040.0129
- **[kuhn53]** Harold W. Kuhn. “Extensive Games and the Problem of Information.” In *Contributions to the Theory of Games II*, 193–216, Princeton University Press, 1953. https://doi.org/10.1515/9781400829156-011

[chvatal79]: https://doi.org/10.1287/moor.4.3.233
[iyengar05]: https://doi.org/10.1287/moor.1040.0129
[kuhn53]: https://doi.org/10.1515/9781400829156-011
