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
every finite continuation. Consequently,
`phasePotential_eq_of_mutuallyReachable` proves that every strongly connected
component of the legal position graph has constant phase potential. Collapsing
those components therefore exposes a directed acyclic structure of irreversible
commitments.

The strict theorem `pawn_move_not_on_cycle` proves that no legal pawn move can
lie on a directed cycle. For example, Lean computes that `1. e4` consumes
exactly two units of potential and proves that no legal continuation can return
to the initial position. In contrast, `1. Nf3` preserves the phase grade. Grade
preservation is only a necessary condition for reversibility; it does not claim
that every quiet move can actually be undone.

## Opening lines and transpositions

`lineIsLegal` checks an imported line move by move, while
`reachable_playMoves_of_lineIsLegal` turns a successful check into a proof that
the line is a path in the legal position graph. This lets opening data feed the
same graph theory used by the phase results.

`independent_knight_development_transposes` proves a first exact move-order
diamond. Both

```text
1. Nf3 Nf6 2. Nc3 Nc6
1. Nc3 Nc6 2. Nf3 Nf6
```

are certified legal and reach the identical complete `Position`, including
turn, castling rights, en-passant state, and both clocks. The next general step
is to replace this computed example with sufficient noninterference conditions
under which two same-side plans commute around the opponent's replies.

## Novelty status

The Réti maneuver and its original study are classical. A preliminary search
found many qualitative explanations of a king pursuing two goals, but no
equivalent closed interval-intersection theorem for arbitrary pairs of king
deadlines, nor a machine-checked derivation of the unique `Kg7-f6` prefix from
such a theorem. `retiPivot_exists_iff` should therefore be treated as a
candidate novel formulation and formal result, not as a claim that an
exhaustive historical literature review has been completed.
