# Turning the relation basis into player knowledge

The 205-relation basis can reduce duplicated opening study, but it cannot turn
8,646 histories into 205 things to memorize. It compresses the *proof that
routes merge*, not the chess content at 7,848 distinct positions. Player value
requires three separate compression claims:

1. **state deduplication:** schedule one next-move card per exact position,
   removing duplicate route occurrences;
2. **relation compression:** explain all route mergers with a basis rather
   than listing every transposed history pair;
3. **schema compression:** teach several basis equations as one reusable chess
   idea, such as “develop either knight around the same waiting move.”

The corpus establishes (1) and the 205-chord theorem establishes (2). Whether
(3) exists is the genuinely player-relevant empirical question.

## What the current trainer gets wrong

The trainer in `engine/` currently embeds 21 SAN main lines. Its
`src/bin/chess-trainer/book.rs` indexes the book by exact UCI sequence
prefix and follows the earliest matching line. Consequently, a legal
transposition into a known position can be reported as “out of book,” and the
same position reached by two routes would need duplicate schedule state.

The replacement should use two indexes, not one:

```text
position index: exact opening key -> repertoire move, explanation, evaluations
route index:    path prefix       -> deviations encountered before a merge
```

The position key must use semantic equality—placement, turn, clean castling
rights, and effective en passant—with equality confirmation. A 64-bit
Polyglot/Zobrist value is an efficient lookup accelerator, not an identity
proof. The complete `Game` remains available for draw-rule history.

Opening books as position DAGs are already established in computer-chess work
[fishburn18][fishburn18]. The new point is to make the quotient visible in the
learning model while preserving route-specific risks.

## What can safely be merged

Once two opening routes reach the same exact position, their legal moves and
ordinary position evaluation agree. A repertoire answer at that node should
therefore be one **node card**, regardless of opening name or incoming move
order. On the pinned catalog this removes 798 duplicate history occurrences,
a 9.2% reduction from 8,646 history cards to 7,848 position cards.

That figure is a property of a sparse name catalog, not a forecast for a
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

Use the 205 chord equations as the candidate pool. Begin with the 84 direct
alternating braids, then the 54 additional relations having at most five plies
per side beyond the 86 very short relations.

### 3. Move-order contrast

For a chord, stop before the merge and present the opponent's plausible
deviations on each route. Ask which route admits or avoids the target sideline.
This turns an algebraic equality into the practical statement “same eventual
position, different options on the way.” Engine evaluation and played-game
frequency belong on this card; endpoint equality alone does not rank routes.

### 4. Substitution and detour diagnosis

Reserve the four non-permutation basis equations for explicit contrastive
study. Ask the learner to identify:

- which piece took a different route;
- which forcing move made the route coalesce;
- which two plies form a removable detour;
- whether the detour was harmless, useful move-order probing, or a concession.

This is more likely to build transferable pattern knowledge than rehearsing
the entire SAN sequence again.

## A human-readable canonical repertoire

The lexicographically least shortest-path arborescence is a reproducible
mathematical baseline, not the right personal backbone. Choose canonical paths
to minimize a weighted learning cost:

```text
route cost = personal unfamiliarity
           + opponent frequency
           + tactical punishment for forgetting
           + explanation length
```

For a fixed backbone, every non-tree edge becomes a relation card into a
canonical position. Changing the arborescence leaves the minimum count 205 but
changes which equations are short and recognizable. The optimization target
should therefore be review burden, not relation count.

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

## Small controlled trial in the existing workflow

Build a 30-position personal micro-repertoire with at least ten
transpositions. Randomly assign matched items to:

- **sequence condition:** ordinary exact-prefix next-move cards;
- **node condition:** position-keyed cards with incoming routes randomized;
- **relation condition:** node cards plus a short relation/deviation prompt.

Use the same scheduler and review budget for four weeks. Test:

1. next-move recall through a trained route;
2. next-move recall through an unseen legal transposition;
3. recognition of an opponent deviation before the merge;
4. move quality in the user's subsequent games;
5. reviews and seconds required per retained item.

The decisive metric is transfer to unseen move orders, not immediate rehearsal
accuracy. A relation-aware trainer is valuable if it reduces review burden or
improves transfer without increasing route mistakes. If it only makes the
backend cleaner, keep the graph representation and hide the algebra from the
player.

## Practical recommendation toward a 2000-level repertoire

Use the algebra as infrastructure, not as the syllabus. For a player working
toward roughly 2000:

- deduplicate position cards immediately;
- expose only short, frequent, high-consequence relation cards;
- prioritize deviations found in the player's own games over obscure named
  catalog routes;
- attach plans and tactical motifs to canonical positions;
- let the engine grade moves and consequences, but require human-readable
  explanations for every retained relation;
- measure whether relation cards transfer before expanding beyond the
  micro-repertoire.

This is the highest-value route from a formal path theorem to stronger chess:
less duplicate memorization, more recognition of shared positions, and exact
warnings about where move orders stop being interchangeable.

## Local References

- <a id="butler10"></a> **butler10** — Andrew C. Butler, “Repeated Testing Produces Superior Transfer of Learning Relative to Repeated Studying,” *Journal of Experimental Psychology: Learning, Memory, and Cognition* 36(5), 2010, pp. 1118–1133. [DOI](https://doi.org/10.1037/a0019902).
- <a id="cepeda06"></a> **cepeda06** — Nicholas J. Cepeda, Harold Pashler, Edward Vul, John T. Wixted, and Doug Rohrer, “Distributed Practice in Verbal Recall Tasks: A Review and Quantitative Synthesis,” *Psychological Bulletin* 132(3), 2006, pp. 354–380. [DOI](https://doi.org/10.1037/0033-2909.132.3.354).
- <a id="chase73"></a> **chase73** — William G. Chase and Herbert A. Simon, “Perception in Chess,” *Cognitive Psychology* 4(1), 1973, pp. 55–81. [DOI](https://doi.org/10.1016/0010-0285(73)90004-2).
- <a id="fishburn18"></a> **fishburn18** — John P. Fishburn, “Search-Based Opening Book Construction,” *ICGA Journal* 40(1), 2018, pp. 2–14. [DOI](https://doi.org/10.3233/ICG-180039).

[butler10]: https://doi.org/10.1037/a0019902
[cepeda06]: https://doi.org/10.1037/0033-2909.132.3.354
[chase73]: https://doi.org/10.1016/0010-0285(73)90004-2
[fishburn18]: https://doi.org/10.3233/ICG-180039
