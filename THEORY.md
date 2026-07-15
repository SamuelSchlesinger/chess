# Structural Chess Theory

This project separates three kinds of evidence:

1. the orthodox move rules are executable and tested against established perft data;
2. geometric and strategic principles are stated as Lean propositions;
3. player-facing corollaries are proved from those principles rather than copied
   from engine output or an endgame table.

## Exact king distance

For squares `s` and `t`, define

```text
d∞(s,t) = max(|file(s)-file(t)|, |rank(s)-rank(t)|).
```

`kingDistance_is_minimum` proves both directions:

- every geometric king route from `s` to `t` has at least `d∞(s,t)` moves;
- a route with exactly `d∞(s,t)` moves always exists.

The second direction is constructive. The proof repeatedly takes the diagonal
or straight step toward the target and proves that the distance decreases by
exactly one. It does not inspect 64 cases.

## The precise geometric pawn square

`ruleOfSquare` derives the pawn-square mnemonic from exact king distance. Its
deadline accounts for:

- which side moves first;
- the pawn's initial two-square move;
- the pawn's color and promotion rank.

This is deliberately a geometric theorem. It does not falsely claim that the
square alone classifies K+P versus K when the attacking king, opposition,
blocked squares, or rook-pawn exceptions matter.

## The Réti pivot theorem

The ordinary pawn square asks whether one king target is reachable by one
deadline. Réti's idea is multi-objective: choose a common route prefix while
retaining the option to pursue either of two later objectives.

Suppose a king starts on `s`. By a budget of `k` king moves, we want a pivot `p`
such that

```text
d∞(s,p) ≤ k
d∞(p,a) ≤ A-k
d∞(p,b) ≤ B-k
```

where `a` and `b` are target squares with deadlines `A` and `B`.

For one coordinate, each inequality is an interval. Define

```text
lower = max(s-k, a-(A-k), b-(B-k))
upper = min(7, s+k, a+(A-k), b+(B-k)).
```

`retiPivot_exists_iff` proves that a pivot exists exactly when `lower ≤ upper`
holds independently for both file and rank. The reverse proof constructs the
pivot from the two lower endpoints. Thus a branching two-purpose king problem
reduces to two closed inequalities.

This is a reusable multi-objective generalization of the rule of the square,
not a theorem about only one composed position.

### Réti's 1921 study

For the famous position with White's king on h8, the relevant two-move pivot
must preserve four-move access to both d6 and f4. Lean proves:

- f6 satisfies both deadlines;
- h6, reached by committing down the h-file, does not;
- f6 is the unique square satisfying both deadlines after two moves;
- g7 is the unique intermediate square on a two-move king route from h8 to f6.

Consequently the celebrated `1.Kg7!` is derived from the structural theorem.
It is not inserted from the published solution or found by enumerating legal
moves.

## Opposition

`vertical_opposition_odd_gap` proves the player's counting rule: for aligned
kings, even king distance is equivalent to an odd number of intervening
squares.

`lowerVerticalDirect_has_restoring_response` is constructive. From direct
vertical opposition, if the lower king moves sideways or backward, it builds a
one-step reply for the upper king that restores direct opposition and proves
that the reply remains on the board.

## Strategy semantics

`CanForceWithin player goal n position` formalizes a bounded adversarial claim:

- at the player's nodes, one legal continuation must work;
- at the opponent's nodes, every legal continuation must work;
- a terminal non-goal node does not succeed vacuously.

This is the bridge from geometry to future statements such as forced promotion
and exact K+P versus K principles.

## Irreversible phase grading

`Position.phasePotential` assigns a natural number to every position. Its board
component gives every piece a base contribution, gives pawns an additional
identity contribution, and counts each pawn's remaining rank-distance to
promotion. Its historical component counts surviving castling rights. Clocks,
turn, and the ephemeral en-passant target are intentionally excluded.

`phasePotential_applyUnchecked_le` proves from the executable orthodox rules
that every legal move weakly decreases this potential. The proof covers:

- ordinary moves and captures;
- pawn single steps, double steps, captures, en passant, and promotion;
- rook and king movement that revokes castling rights;
- castling, including the simultaneous king and rook relocation.

The path-level theorem `reachable_phasePotential_le` lifts this local result to
every finite continuation. Literal `Position` values contain advancing move
clocks, so cycle statements would be uninteresting on that graph. Instead,
`RepetitionReachable` projects concrete continuations to FIDE repetition
identity: board, turn, castling rights, and effective en-passant availability.
`sameForRepetition_equivalence` proves that this identity is reflexive,
symmetric, and transitive, so quotienting by it is mathematically well-defined
rather than merely an informal grouping. The harder operational theorem is now
also proved: `legal_iff_of_sameForRepetition` says representatives admit the
same legal raw moves, and `sameForRepetition_applyUnchecked` says applying any
raw move preserves the equivalence class. The former proof handles the exact
FIDE subtlety that unequal raw en-passant fields can affect pseudo-legality but
cannot affect full legality unless a genuinely legal en-passant capture exists;
in that case the target is effective and repetition identity forces agreement.
As a search-level consequence, `perft_eq_of_sameForRepetition` proves that
equivalent representatives have identical exhaustive leaf counts at every
finite depth. Checkmate and stalemate are invariant as well, and path lifting
extends this to the semantic `DeadPosition` property: no representative can
acquire a mating continuation absent from another. History-sensitive draw
claims are intentionally properties of `GameState`, not of a repetition node.

`RepetitionNode` is therefore a real quotient type with well-defined legality,
legal-move lists, move application, successor edges, and finite reachability.
Path lifting proves that `RepetitionReachable` is equivalent to reachability in
this quotient graph, rather than merely an endpoint-grouping heuristic.
`RepetitionNode.phasePotential` descends the irreversible grade to the quotient,
where `phasePotential_eq_of_mutuallyReachable` proves that every strongly
connected component has constant potential and
`no_strict_phase_edge_on_cycle` excludes every strictly descending edge from a
directed cycle.

The strict theorem `pawn_move_not_on_cycle` proves that after any legal pawn
move, no legal continuation can return to the source's repetition class. For
example, Lean computes that `1. e4` consumes exactly two units of potential and
proves that no legal continuation can return to the initial position's FIDE
repetition class. In
contrast, `1. Nf3` preserves the phase grade. Grade preservation is only a
necessary condition for reversibility; it does not claim that every quiet move
can actually be undone.

`move_on_repetition_cycle_is_quiet` gives the structural information available
on any edge whose successor can return to the source's repetition class: it
moves a non-pawn to an empty square, preserves all castling rights, and
increments the halfmove clock.
Ordinary captures are excluded by strict occupied-target accounting; en
passant is excluded as a pawn move; castling and first king or rook moves are
excluded when they consume a right. The legal shuffle

```text
1. Nf3 Nf6 2. Ng1 Ng8
```

is a non-vacuity witness: it returns to the initial FIDE repetition node, and
Lean derives that its first edge satisfies the quiet-kernel conclusion.

## Opening lines and transpositions

`lineIsLegal` checks an imported line move by move, while
`reachable_playMoves_of_lineIsLegal` turns a successful check into a proof that
the line is a path in the legal position graph. This lets opening data feed the
same graph theory used by the phase results.

`playMoves_append` and `lineIsLegal_append` make legal lines into a small trace
algebra: concatenation composes endpoint transformations, and legality splits
into legality of the prefix plus legality of the suffix at the intermediate
position. `LinesTransposeAt` identifies legal lines with the same complete
instantaneous endpoint. It is an equivalence relation and is preserved by a
common legal prefix or suffix.

Operational congruence turns the free monoid of move words into a deterministic
partial action on `RepetitionNode`: `RepetitionNode.playMoves` is the total
state transformation and `RepetitionNode.lineIsLegal` specifies its domain.
`lineIsLegal_eq_of_sameForRepetition` proves equality of the entire residual
legal language at equivalent positions, while
`playMoves_sameForRepetition` proves endpoint congruence for arbitrary words.
`RepetitionTrace` packages a legal labelled path to a quotient-specified target;
its append theorem factors a trace through an intermediate repetition class.

A `ReplyPlan` packages one move and its reply. This is the correct atomic unit
for opening commutation because a single ply changes whose turn it is, whereas
a move/reply pair restores it. `ReplyPlansIndependentAt` states that either
plan is legal first, remains legal after the other, and that their endpoint
transformations commute. `replyPlansCommute_iff_independent` proves that these
semantic independence obligations are exactly a legal four-ply move-order
diamond.

`independent_knight_development_transposes` proves a first exact move-order
diamond. Both

```text
1. Nf3 Nf6 2. Nc3 Nc6
1. Nc3 Nc6 2. Nf3 Nf6
```

are certified legal and reach extensionally identical complete instantaneous
positions, including turn, castling rights, raw en-passant state, and both
clocks. Their `GameState.prior` histories are not identified.

Exact endpoint equality is deliberately stronger than the ordinary
opening-player notion of transposition. Lean also proves that

```text
1. Nf3 d5 2. d4
1. d4 d5 2. Nf3
```

reach the same FIDE repetition node even though the first endpoint has
halfmove clock `0` and a raw but ineffective `d3` en-passant target, while the
second has halfmove clock `1` and no raw target. `LinesRepetitionTransposeAt`
records this quotient notion separately. Lean now proves the player-facing
consequence: the two endpoints admit exactly the same legal continuation words,
produce equal `perft` counts at every depth, and remain transposed after
appending `...Nf6`. More generally, repetition transpositions are preserved by
every legal common suffix and compose by substitution of further quotient
diamonds. Raw clocks, ineffective en-passant fields, and `GameState.prior`
histories remain deliberately outside that conclusion. These quotient classes
are the right nodes for mining real opening databases.

## Novelty status

The Réti maneuver and its original study are classical. A preliminary search
found many qualitative explanations of a king pursuing two goals, but no
equivalent closed interval-intersection theorem for arbitrary pairs of king
deadlines, nor a machine-checked derivation of the unique `Kg7-f6` prefix from
such a theorem. `retiPivot_exists_iff` should therefore be treated as a
candidate novel formulation and formal result, not as a claim that an
exhaustive historical literature review has been completed.
