# From certified claims to a personal chess trainer

The Lean layer should be the authority for exact chess claims; the Rust crate
at `engine/` should be the fast delivery layer. The
bridge is not “show the user a proof.”  It is a small, versioned knowledge item
that turns one theorem into a rule, its boundary, contrastive exceptions, and
exercises whose answers can be checked independently.

This is a contribution to a path toward roughly 2000 playing strength, not a
rating guarantee.  The training system still needs game review, calculation,
opening decisions, and practical play.  Certification prevents bad material
from entering that loop; it does not decide the best curriculum by itself.

## The existing delivery layer

The current Rust application already supplies most of the runtime mechanics:

- a fully legal board, FEN, UCI, and SAN;
- an embedded book of 21 named opening lines whose replies are selected by
  move-sequence prefix;
- a warm UCI engine, fixed-depth grading, an opponent fallback, and a
  consequence-first explanation after an error;
- short “reps” of six trainee moves and session-level accuracy statistics.

Those claims are directly checkable in the trainer sources
[trainer-book][trainer-book] [trainer-main][trainer-main]
[trainer-app][trainer-app].  The present frontend clears its history for each
new session and has no persistent scheduler.  Its book is a useful demonstration
set, not yet a personal repertoire: the earliest matching line chooses the
opponent reply, and alternative adversarial deviations are not scheduled.

The smallest extension is therefore a data contract and a durable review log,
not another chess engine.

## Knowledge-item contract

Keep authored, immutable content separate from mutable review state.  One JSONL
record should have this conceptual shape:

```json
{
  "id": "fide-repetition-effective-ep/v1",
  "kind": "rule-exception",
  "prompt": {
    "fen": "8/8/8/8/k2Pp2Q/8/8/3K4 b - d3 0 1",
    "question": "Does removing the d3 en-passant field change the FIDE repetition position?"
  },
  "answer": {
    "verdict": "no",
    "accepted_uci": [],
    "explanation": "...exd3 e.p. is pseudo-legal but exposes the black king on a4 to Qh4, so the target is ineffective."
  },
  "claim": {
    "semantics": "FIDE-2023",
    "lean_name": "Chess.RepetitionKey.ofPosition_eq_iff",
    "checker": "repetition_ep_counterexample.py",
    "artifact_sha256": "filled by export"
  },
  "tags": ["rules", "repetition", "en-passant", "exception"]
}
```

The runtime review row is keyed by `(user, item id, content version)` and holds
only observations such as due time, attempts, response, latency, hints, lapses,
and the scheduler's current stability estimate.  Content changes create a new
version rather than silently inheriting mastery of a different question.

For history-sensitive claims, a FEN is not enough.  The prompt must also carry
the preceding UCI history or a certified history artifact, because threefold
and fivefold repetition are properties of a game state.  Likewise, a Polyglot
hash is not a proof identity: exported artifacts need an exact, collision-
checked repetition key.

## Flagship theorem-to-system example

FIDE Article 9.2.3 distinguishes positions by en-passant state only when the
possible moves differ [fide2023][fide2023].  The Lean repository computes that
notion as `effectiveEnPassantTarget` and proves that equality of
`RepetitionKey.ofPosition` is equivalent to the modeled FIDE relation; its hash
is merely a bucket hash backed by exact equality
[lean-repetition-key][lean-repetition-key].

The Rust engine has the right ingredients but currently composes them at the
wrong boundary:

1. `Board::hash()` is explicitly Polyglot-compatible.
2. Its en-passant contribution tests for an adjacent capturing pawn, which is
   the Polyglot convention, but does not test whether the capture leaves that
   pawn's king safe [rust-board][rust-board].
3. `Game::repetition_count` compares those hashes, and search seeds and scans
   the same keys for repetition [rust-game][rust-game]
   [rust-search][rust-search].

The local counterexample is:

```text
with en passant:    8/8/8/8/k2Pp2Q/8/8/3K4 b - d3 0 1
without en passant: 8/8/8/8/k2Pp2Q/8/8/3K4 b - - 0 1
```

Black's pawn on e4 is adjacent to d3, but `...exd3 e.p.` removes the e4 and d4
pawns and exposes the black king on a4 to the white queen on h4.  The Rust
move-generation test already establishes that the capture is illegal
[rust-ep-test][rust-ep-test].  The pinned oracle script independently confirms
that the two FIDE keys are equal while their Polyglot hashes differ; the exact
run is recorded in [repetition-ep-output.txt](data/repetition-ep-output.txt).

This mismatch can undercount a real repetition in both `Game` and search.  It
does not require abandoning Polyglot compatibility: the design lesson is to
retain `Board::hash()` for book and transposition uses while adding a distinct
FIDE repetition identity whose en-passant component is conditioned on a
*legal* capture.  No change to the Rust repository is made in this research
branch.

This one result yields three human-facing artifacts:

- **Rule card:** same placement, side, and castling rights are not sufficient;
  en passant matters exactly when it creates a legal move.
- **Exception pair:** show the two FENs above and ask whether they are the same
  repetition position before showing the pin.
- **System check:** reject any exercise export or game-history count that uses
  a raw FEN field or Polyglot hash as exact FIDE identity.

That is the desired pipeline in miniature: formal definition, executable
counterexample, engine-level consequence, and a memorable exception drill.

## Four exercise forms

Every promoted result should produce all four forms below.

1. **Rule recall.** State the rule from a sparse diagram and name its scope.
2. **Boundary judgment.** Decide whether the rule applies, without moving a
   piece.  This catches memorized slogans detached from preconditions.
3. **Minimal-pair exception.** Alternate a positive example with a position
   differing in one relevant feature.  The en-passant pair above is the model.
4. **Transfer move.** Find the move or plan in an unseen position, with the
   engine used only for secondary practical grading after the certified answer
   has been checked.

A tactical certificate can populate the transfer answer with one or several
winning first moves.  A tablebase theorem should populate both positive and
negative boundary examples.  An opening route theorem should populate the
opponent deviations that distinguish two move orders, not just replay a single
main line.

Retrieval should occur before feedback: delayed testing improves retention
relative to repeated study in the classic experiments
[roediger-karpicke2006][roediger-karpicke2006].  Reviews should also be spaced,
but the interval must respond to the intended retention horizon rather than
copy a universal magic sequence [cepeda2008][cepeda2008].  A reasonable pilot
starts with short expanding intervals, logs every response, and calibrates from
delayed performance.

## Human-executable study loop

A compact daily loop can use the same queue without conflating its item types:

1. clear due rule and exception cards without an engine hint;
2. calculate a small set of certificate-backed positions to completion;
3. rehearse personal-repertoire decision nodes against sampled deviations;
4. finish with one unseen transfer position and record the reason for any miss.

Once per week, import recent games into the existing analysis GUI and convert
recurring errors into tagged candidate items.  A candidate becomes schedulable
only after its exact answer has a theorem, a checked certificate, a tablebase
provenance record, or an explicitly labeled empirical engine threshold.

The dashboard should privilege delayed first-attempt accuracy, exception false
positives, and unseen transfer.  Engine-match percentage and centipawn loss are
useful diagnostics, but optimizing them alone trains imitation of one search
configuration rather than stable chess knowledge.

## Promotion and stop rules

Promote a knowledge item only when:

- its semantics are named (`FIDE game`, history-free board, WDL, DTZ, or DTM);
- every accepted answer is independently checked;
- the rule text states its domain and known exceptions;
- at least one near-miss or mutation is rejected by the checker;
- the Rust importer round-trips its FEN/UCI and verifies the artifact digest.

Stop or rewrite an item when delayed users can memorize the diagram but fail a
minimal pair, when the “rule” needs a lookup-sized exception list, or when a
source engine/tablebase metric cannot be reconciled with the claimed game
semantics.

## Local References

- **trainer-book** — `engine/src/bin/chess-trainer/book.rs`, embedded opening book and prefix-selection logic, imported Rust snapshot inspected 14 July 2026.
- **trainer-main** — `engine/src/bin/chess-trainer/main.rs`, trainer HTTP API and fixed-depth UCI grading, imported Rust snapshot inspected 14 July 2026.
- **trainer-app** — `engine/src/bin/chess-trainer/app.js`, session state, six-move reps, and statistics, imported Rust snapshot inspected 14 July 2026.
- **fide2023** — International Chess Federation, *FIDE Laws of Chess Taking Effect from 1 January 2023*, Articles 5.2.2, 9.2.3, 9.3, and 9.6, approved 7 August 2022.
- **lean-repetition-key** — `Chess/RepetitionKey.lean`, exact executable repetition key and equivalence theorem, local formalization repository snapshot inspected 14 July 2026.
- **rust-board** — `engine/src/board.rs`, `Board::hash` and `ep_hash_contribution`, imported Rust snapshot inspected 14 July 2026.
- **rust-game** — `engine/src/game.rs`, `Game::position_keys` and `Game::repetition_count`, imported Rust snapshot inspected 14 July 2026.
- **rust-search** — `engine/src/search.rs`, search-history and repetition detection, imported Rust snapshot inspected 14 July 2026.
- **rust-ep-test** — `engine/tests/legal_movegen.rs`, `ep_discovered_check_is_illegal`, imported Rust snapshot inspected 14 July 2026.
- **roediger-karpicke2006** — Henry L. Roediger III and Jeffrey D. Karpicke, “Test-Enhanced Learning: Taking Memory Tests Improves Long-Term Retention,” *Psychological Science* 17(3), 2006, 249–255. DOI 10.1111/j.1467-9280.2006.01693.x.
- **cepeda2008** — Nicholas J. Cepeda, Edward Vul, Doug Rohrer, John T. Wixted, and Harold Pashler, “Spacing Effects in Learning: A Temporal Ridgeline of Optimal Retention,” *Psychological Science* 19(11), 2008, 1095–1102. DOI 10.1111/j.1467-9280.2008.02209.x.

[trainer-book]: ../../../engine/src/bin/chess-trainer/book.rs
[trainer-main]: ../../../engine/src/bin/chess-trainer/main.rs
[trainer-app]: ../../../engine/src/bin/chess-trainer/app.js
[fide2023]: https://handbook.fide.com/chapter/E012023
[lean-repetition-key]: ../../../Chess/RepetitionKey.lean
[rust-board]: ../../../engine/src/board.rs
[rust-game]: ../../../engine/src/game.rs
[rust-search]: ../../../engine/src/search.rs
[rust-ep-test]: ../../../engine/tests/legal_movegen.rs
[roediger-karpicke2006]: https://doi.org/10.1111/j.1467-9280.2006.01693.x
[cepeda2008]: https://doi.org/10.1111/j.1467-9280.2008.02209.x
