# Position-graph trainer and personal repertoire

This is the product translation of the research program for the existing
the monorepo's `engine/` crate and browser trainer.

## Replace the sequence book, not the engine stack

The current trainer already supplies full legality, SAN/FEN/PGN interop, a warm
UCI engine, MultiPV grading, and a usable drill loop. Keep those pieces. Replace
the embedded 21-line, earliest-match opening book with a persistent
position-graph repertoire.

Current behavior and proposed behavior differ as follows:

| Current | Proposed |
|---|---|
| Earliest matching line chooses the opponent move | Weighted opponent replies from a pinned cohort, with deliberate rare-deviation drills |
| Every repetition starts at move one | Due cards start at the exact decision position, with occasional full-route integration reps |
| “Correct” means close to Stockfish's current first choice | Repertoire correctness and engine regret are separate fields |
| Session-only engine-match statistics | Persistent recall, latency, coverage, and live-game transfer metrics |
| Move-sequence prefix identifies the opening | Exact position identifies reusable knowledge; route occurrences retain provenance |

## Minimal data contract

```text
PositionDecision
  key                 canonical repetition fields, equality checked
  fen                 display/interchange position
  side                player side
  accepted_moves      one backbone move, optionally sound alternatives
  explanation         why the move belongs in this repertoire
  plans               pawn breaks, piece placements, tactical warnings
  engine_evidence     engine/net/options/nodes, MultiPV scores, timestamp
  occurrences[]       route ids and empirical weights

RouteOccurrence
  route_id
  moves               complete UCI prefix
  source_dataset      archive hash and cohort
  count
  first_deviations[]  opponent move, child key, count, severity, response

ReviewState
  card_id
  due_at
  stability
  difficulty
  attempts
  successes
  last_latency_ms
  hint_count
```

Do not use a 64-bit Zobrist value as identity. It is an excellent index, but
canonical equality must resolve collisions. Do not discard the route after
creating a position card: move-order warnings and opponent frequency are
occurrence properties.

## Five exercise types

1. **Position decision:** show a due board with no preceding moves; recall the
   repertoire move and its reason.
2. **Deviation response:** play a high-frequency or high-severity opponent
   deviation, then ask for the prepared response.
3. **Transposition transfer:** reach a known position through an unseen route;
   test whether the learner recognizes the same decision and plan.
4. **Commitment choice:** present two playable move orders and ask which
   opponent option or target structure each allows.
5. **Integration game:** start at move one and sample opponent replies from the
   cohort distribution, with a small adversarial probability reserved for
   supported dangerous sidelines.

The first four isolate knowledge; the fifth tests execution. This matters
because repeated retrieval improves delayed retention [karpicke08][karpicke08],
while opening specialization affects both recall and problem solving
[bilalic09][bilalic09]. A sequence-only drill cannot tell whether the learner
recognizes the position or merely continues a familiar song.

## Scheduling and priority

Schedule each card independently toward a declared target retention, initially
90%. Use a transparent stability/difficulty model and log every prediction;
only fit a more complex scheduler after enough personal reviews exist. Modern
work formulates review scheduling as a stochastic shortest-path problem
[ye22][ye22], but no scheduler should be imported without calibration to chess
cards.

Among cards due at similar times, prioritize by:

```text
priority = forgetting_risk
           * encounter_probability
           * mistake_severity
           * route_relevance
           * transfer_gap.
```

Always retain a small exploration quota for rare severe moves. Otherwise a
frequency optimizer teaches only what the player already survives.

Grade four dimensions separately:

- repertoire move recalled without hint;
- response latency;
- fixed-protocol engine regret;
- explanation/plan recalled.

A valid repertoire move is not “wrong” merely because another MultiPV move is
three centipawns higher. Conversely, memorizing an engine-best move without its
critical reply is not mastery.

## Provisional repertoire backbone

Without personal games or style data, only a testable starting hypothesis is
responsible:

- **White:** `1.d4` and `2.c4`, with a Catalan/QGD-oriented core;
- **Black versus `1.e4`:** Caro-Kann;
- **Black versus `1.d4`, `1.Nf3`, and `1.c4`:** a QGD/Semi-Slav central setup
  where legal and sound, with explicit route cards for independent English and
  Réti deviations.

Why this candidate: it is sound, structurally coherent, and the pinned pilot
finds unusually large transposition fibres and downstream reuse in the
`d4/c4/Nf3`, QGD, Catalan, Slav, and Semi-Slav region. Why it is not final: the
taxonomy has no play frequencies, the setup cannot be forced against every
move order, and no style preference has been elicited.

Freeze the actual repertoire only after importing the player's games and
running the held-out experiment. Every chosen branch must pass four gates:

1. no unacceptable engine regret under the pinned protocol;
2. meaningful encounter mass in the target cohort or personal history;
3. manageable unique-position and exception cost;
4. positions the player is willing to study and play repeatedly.

## Measurable progression toward approximately 2000

Rating is an outcome, not an opening-training metric. Track these leading
indicators weekly:

- delayed no-hint recall by card type;
- median response latency;
- transfer accuracy on unseen move orders;
- covered opponent probability mass by color and cohort;
- number and severity of uncovered supported deviations;
- worst pinned-engine regret among prescribed moves;
- unique position cards, route exceptions, and reviews per week;
- in personal games: first repertoire exit, time spent before that exit, and
  evaluation change over the next two player decisions.

A practical staged guide is:

1. **Baseline:** import recent games; identify the first avoidable opening loss
   or time sink in each; do not add theory indiscriminately.
2. **Backbone:** cover the most common replies with one sound move per own
   decision and short explanations.
3. **Robustness:** close every high-severity supported deviation, even when
   rare; add commitment warnings.
4. **Compression:** merge endpoint-invariant cards and test them through
   alternate routes; split any merged card that repeatedly fails transfer.
5. **Deepening:** extend only branches reached in personal games or carrying
   high cohort mass; use model games and plans, not moves alone.
6. **Maintenance:** cap daily opening reviews so tactics, calculation,
   endgames, and game analysis remain the majority of training.

The trainer succeeds if it produces reliable decisions in games with fewer
reviews, not if it maximizes XP or engine-match percentage.

## Local References

- **[bilalic09]** Merim Bilalić, Peter McLeod, and Fernand Gobet. “Specialization Effect and Its Influence on Memory and Problem Solving in Expert Chess Players.” *Cognitive Science* 33(6), 1117–1143, 2009. https://doi.org/10.1111/j.1551-6709.2009.01030.x
- **[karpicke08]** Jeffrey D. Karpicke and Henry L. Roediger III. “The Critical Importance of Retrieval for Learning.” *Science* 319(5865), 966–968, 2008. https://doi.org/10.1126/science.1152408
- **[ye22]** Junyao Ye, Jingyong Su, Liqiang Nie, Yilong Cao, and Yongyong Chen. “A Stochastic Shortest Path Algorithm for Optimizing Spaced Repetition Scheduling.” *Proceedings of the 28th ACM SIGKDD Conference on Knowledge Discovery and Data Mining*, 4381–4390, 2022. https://doi.org/10.1145/3534678.3539081

[bilalic09]: https://doi.org/10.1111/j.1551-6709.2009.01030.x
[karpicke08]: https://doi.org/10.1126/science.1152408
[ye22]: https://doi.org/10.1145/3534678.3539081
