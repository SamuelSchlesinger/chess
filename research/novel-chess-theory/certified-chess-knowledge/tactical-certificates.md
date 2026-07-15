# Proof-producing tactical certificates

## Claim worth certifying

The useful first target is not “the engine likes this move.”  It is:

> From this complete game state, the attacker can force checkmate within at
> most `n` plies against every legal defense, under explicitly named draw
> semantics.

Proof-number search is a natural untrusted producer because it searches the
existential/universal structure of game trees [allis1994][allis1994].  Lean
should check a compact strategy object after search, following the established
architecture of a fast untrusted solver plus a small verified certificate
checker [darbari2009][darbari2009] [lammich2020][lammich2020].

A principal variation is not such a certificate.  Replaying one line proves
only that one cooperative continuation mates.  Every defender node must show
that the certificate covers *exactly all* legal replies.

## Force relations before certificates

Fix an attacker color `A` and a remaining ply budget `n`. Define the semantic
relations without mentioning a certificate.

`PositionTreeForce A n p` deliberately forgets `p`'s halfmove and fullmove
clocks and has no prior history. Checkmate is success exactly when the mated
side is `A.other`; stalemate and budget exhaustion are failure. At a
nonterminal node with positive budget, an `A` node succeeds when some legal
move succeeds recursively, while an `A.other` node succeeds only when every
legal move succeeds. Repetition, 50/75-move rules, and claims do not exist in
this puzzle convention. Dead position need not be an executable terminal test:
an accepted legal path to checkmate itself contradicts `DeadPosition`.

`FIDEForce A n s` instead consumes a complete `GameState`. Its terminal order
is fixed:

1. current checkmate succeeds exactly when the mated side is `A.other`; a
   checkmate of `A` fails;
2. otherwise stalemate, semantic dead position, fivefold repetition, or the
   75-move limit fails as an automatic draw;
3. otherwise play continues if the budget is positive.

This gives checkmate precedence over a simultaneous 75-move threshold, matching
the repository definition and FIDE [fide2023][fide2023]. At attacker nodes,
`A` may decline a claim and choose one legal move. At defender nodes, the
universal response type is:

```text
play(move)       where Legal current move
claim-now        where DrawClaimAvailable state
claim-after(move) where DrawClaimAvailableAfter state move
```

Either claim action ends the branch as a draw, so a forced-mate witness cannot
exist when the defender has one. `claim-after` covers both an announced move
that creates a third occurrence and one that reaches the 50-move threshold.
Automatic draws created by an ordinary move are caught at the next recursive
state. This relation distinguishes an exact FIDE force claim from search's
internal repetition heuristics.

## Certificate shape

Only after those relations are fixed, use the inductive strategy object:

```text
mate
attacker(move, child)
defender([(move_1, child_1), ..., (move_k, child_k)])
```

The checker is parameterized by the semantic mode, attacker, remaining ply
budget, and either a `Position` or `GameState`.

- `mate` succeeds exactly for checkmate by the declared attacker.
- `attacker` requires a legal move, decrements the budget, and checks its child.
- `defender` compares the listed moves extensionally with
  `Chess.legalMoves state.current`, rejects duplicates, and checks every child.
- position-tree mode rejects stalemate; FIDE mode additionally checks the
  decidable automatic conditions and rejects any `claim-now` or `claim-after`
  defense action. Semantic deadness is discharged in the soundness proof below.

The crucial soundness theorem says that checker success implies the bounded
force relation defined independently of the certificate.  Completeness is a
later convenience theorem; soundness is the trust boundary.

The current repository has `Legal`, `legalMoves`, `applyUnchecked`,
`GameState.afterMove`, `Checkmate`, both claim forms, and automatic-draw
semantics [local-game][local-game] [local-rules][local-rules]. Its
`DeadPosition` is a semantic proposition, not an executable Boolean. The first
checker therefore does not pretend to decide it: from each accepted subtree,
the soundness proof extracts a legal continuation ending in checkmate and thus
proves that every visited position is not dead. A later supported-domain
dead-position decider would be a separate certified component.

## Identity semantics

A memoized certificate DAG must use an identity adequate for the claim.  The
formal `RepetitionKey` is exact for position-level repetition identity, but two
nodes with the same current position can have different repetition counts.  A
history-sensitive proof therefore cannot merge them merely because their board
keys agree.

This matters immediately for integration. The Rust engine now uses an exact
structural repetition record rather than its Polyglot hash for game and search
history; the legal-history regression and training drill are documented in
[the trainer bridge](training-integration.md). Even so, equal current keys do
not justify merging histories with different claim availability.

## Smallest decisive pilot

1. Encode one legal mate-in-two position with at least two defender replies.
2. Produce a complete certificate with an untrusted depth-first or
   proof-number search.
3. Check it in Lean and prove the checker sound once.
4. Apply five mutations and require rejection: delete one legal defense,
   substitute an illegal move, claim mate one ply early, replace the history by
   one with an at-state defender claim, and set a 99-ply halfmove clock where a
   declared legal non-zeroing defense supports `claim-after`.
5. Run both semantic modes and require the two claim mutations to affect only
   FIDE mode.
6. Export the root FEN, full history when required, accepted first move or
   moves, SAN explanation, semantic mode, and certificate digest as a trainer
   item.

The route fails at this checkpoint if a deleted defense is accepted, the claim
silently ignores draw rights, or the artifact is too large to inspect and cache
for a two-move tactic.  A successful pilot validates infrastructure, not a
novel chess theorem.

## Route to player-useful theorems

Certificates become chess knowledge only after compression across examples.
Cluster certified positions by a candidate motif—overloaded defender, clearance,
interference, or a geometric mating net—then conjecture a sufficient condition
and prove that condition once.  The trainer should present:

- a sparse rule card stating the sufficient condition;
- positive exercises whose certificates instantiate it;
- minimal negative positions where one precondition is removed;
- unseen transfer positions, graded first by the certified answer and only
  secondarily by engine centipawn loss.

That progression separates three achievements: trustworthy puzzle answers,
an exact reusable tactical lemma, and actual human transfer.  Only the latter
two support a novelty claim.

## Prior-art boundary

Formal chess work is not new in itself.  Maliković built a Coq system for
retrograde chess problems [malikovic2008][malikovic2008], while later work
proved a complete executable KRK strategy in Isabelle/HOL.  Likewise,
certificate checking is mature in SAT.  A publishable contribution must
therefore be one of:

- a new chess-specific compact certificate format with a materially smaller
  trusted base and measured compression;
- a new tactic-family theorem extracted from many certificates;
- a verified exercise pipeline that catches a demonstrated solver/trainer
  semantic failure.

“We replayed engine lines in Lean” is explicitly below the bar.

## Local References

- **allis1994** — L. Victor Allis, Maarten van der Meulen, and H. Jaap van den Herik, “Proof-Number Search,” *Artificial Intelligence* 66(1), 1994, 91–124. DOI 10.1016/0004-3702(94)90004-3.
- **darbari2009** — Ashish Darbari, Bernd Fischer, and João Marques-Silva, “Industrial-Strength Formally Certified SAT Solving,” arXiv:0911.1678, 2009.
- **lammich2020** — Peter Lammich, “Efficient Verified (UN)SAT Certificate Checking,” *Journal of Automated Reasoning* 64, 2020, 513–532. DOI 10.1007/s10817-019-09525-z.
- **local-game** — `Chess/Game.lean`, checkmate, history, and FIDE draw semantics, local formalization repository snapshot inspected 14 July 2026.
- **local-rules** — `Chess/Rules.lean`, legal move enumeration and application, local formalization repository snapshot inspected 14 July 2026.
- **fide2023** — International Chess Federation, *FIDE Laws of Chess Taking Effect from 1 January 2023*, Articles 5.1, 5.2, 9.2, 9.3, and 9.6, approved 7 August 2022.
- **malikovic2008** — Marko Maliković, “A Formal System for Automated Reasoning about Retrograde Chess Problems Using Coq,” *Central European Conference on Information and Intelligent Systems*, 2008.

[allis1994]: https://doi.org/10.1016/0004-3702(94)90004-3
[darbari2009]: https://arxiv.org/abs/0911.1678
[lammich2020]: https://doi.org/10.1007/s10817-019-09525-z
[local-game]: ../../../Chess/Game.lean
[local-rules]: ../../../Chess/Rules.lean
[fide2023]: https://handbook.fide.com/chapter/E012023
[malikovic2008]: https://archive.ceciis.foi.hr/index.php/ceciis/2008/paper/view/174.html
