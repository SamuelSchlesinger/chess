# From certified claims to a personal chess trainer

The Lean layer should be the authority for exact chess claims; the Rust crate
at `engine/` should be the fast delivery layer. The
bridge is not “show the user a proof.”  It is a small, versioned knowledge item
that turns one theorem into a rule, its boundary, contrastive exceptions, and
exercises whose answers can be checked independently.

This is a contribution to a path toward roughly 2000 playing strength, not a
rating guarantee. The training system still needs game review, calculation,
opening decisions, and practical play. When a concrete assurance artifact
entails an exact answer, it prevents the declared class of semantic errors
(such as an omitted legal defense or the wrong repetition identity). It does
not certify the explanation, curriculum, retention effect, or practical value.

## The existing delivery layer

The current Rust application already supplies most of the runtime mechanics:

- a fully legal board, FEN, UCI, and SAN;
- an embedded book of 21 named opening lines whose replies are selected by
  move-sequence prefix;
- a warm UCI engine, fixed-depth grading, an opponent fallback, and a
  consequence-first explanation after an error;
- short “reps” of six trainee moves and session-level accuracy statistics;
- a private diagnostic-review mode with exact replay-checked positions,
  answer-hidden recall, and a durable append-only review log whose answer
  releases survive restart until they are graded.

Those claims are directly checkable in the trainer sources
[trainer-book][trainer-book] [trainer-main][trainer-main]
[trainer-app][trainer-app] [trainer-review][trainer-review].  Free play still
clears its history for each new session, and its book remains a useful
demonstration set rather than a personal repertoire: the earliest matching line
chooses the opponent reply, and alternative adversarial deviations are not
scheduled. The separate diagnostic mode now persists reviews for a six-card
private pilot under a transparent fixed schedule. It does not yet implement the
position graph, route deviations, or transfer experiment proposed here.

The smallest extension—a data contract and durable review log rather than
another chess engine—is therefore implemented for the diagnostic pilot.

## Knowledge-item contract

Keep authored, immutable content separate from mutable review state.  One JSONL
record should have this conceptual shape:

```json
{
  "id": "fide-repetition-effective-ep-history/v1",
  "kind": "rule-exception",
  "prompt": {
    "initial_fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
    "uci_history": ["d2d4", "e7e5", "d4e5", "g8f6", "e2e4", "f6e4", "g1f3", "e4c5", "b1c3", "g7g6", "c1f4", "f8g7", "d1d2", "e8g8", "h2h3", "f8e8", "a2a3", "d7d5", "f3g1", "b8d7", "g1f3", "d7b8", "f3g1", "b8d7", "g1f3", "d7b8"],
    "question": "How many times has the current FIDE repetition position occurred?"
  },
  "answer": {
    "verdict": "3",
    "accepted_uci": [],
    "explanation": "After ...d7d5, e5d6 e.p. is pseudo-legal but exposes White's king to Re8. The raw target is ineffective, so the two later knight-cycle returns are the second and third FIDE occurrences."
  },
  "claim": {
    "semantics": "FIDE-2023",
    "assurance": "cross-validated-diagnostic",
    "concrete_lean_theorem": null,
    "evidence": [
      {"kind": "formal-model-theorem", "ref": "Chess.RepetitionKey.ofPosition_eq_iff", "scope": "generic identity equivalence only"},
      {"kind": "pinned-oracle", "ref": "repetition_ep_counterexample.py", "version": "chess==1.11.2"},
      {"kind": "engine-regression", "ref": "engine/tests/repetition.rs::standard_start_threefold_is_not_undercounted_by_polyglot_ep_semantics"}
    ],
    "artifact_sha256": "filled by export"
  },
  "tags": ["rules", "repetition", "en-passant", "exception"]
}
```

`assurance` is one disjoint primary value:

- `lean-theorem-instance` means a named concrete theorem entails this exact
  answer under the named semantics;
- `checked-certificate` means a versioned checker accepted a certificate whose
  digest is stored;
- `pinned-oracle` and `engine-regression` mean reproducible executable evidence,
  not a theorem;
- `engine-estimate` is a depth/time/model-specific empirical judgment;
- `human-hypothesis` is an ungraded proposition awaiting a study;
- `cross-validated-diagnostic` means several independent artifacts agree but no
  one formal artifact entails the complete external claim.

Tablebase-backed records use `pinned-oracle` and must additionally name the
metric, table family, endpoint or file digest, and clock semantics. Assurance
labels are not silently promoted when a nearby generic theorem exists.

Within the current local profile, runtime review state is keyed by
`(item id, semantic content version)` and is replayed from immutable JSONL
observations containing answer release, due time, response, latency, hints,
lapses, and the scheduler decision. Answer feedback is fsynced before it is
returned and restored after interruption. A future multi-profile store would
add the user to that key.
Content changes create a new version rather than silently inheriting mastery of
a different question.
Replaceable evidence provenance has an independent version and does not reset
mastery when the tested semantics are unchanged.

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

The pinned `python-chess` implementation records the Polyglot convention
directly: an adjacent pawn causes the en-passant file to be hashed even when
the potential capture is illegal [python-chess112][python-chess112]. Its
`chess-1.11.2.tar.gz` source artifact is pinned by SHA-256 in the executable
report.

The current Rust implementation now separates the two identities:

1. `Board::hash()` remains Polyglot-compatible for opening books and the
   transposition table [rust-board][rust-board].
2. `RepetitionKey` stores the complete 64-square placement, side to move,
   castling rights, and a raw en-passant target only when at least one
   en-passant capture is legal. Equality is structural; derived hashing can
   index a collection but does not decide equality [rust-repetition][rust-repetition].
3. `Game::position_keys` stores those records and `Game::repetition_count`
   compares them exactly [rust-game][rust-game].
4. Search stores the same exact records, but deliberately reports an internal
   repetition draw after one matching ancestor in the reversible-move window.
   That twofold search heuristic is not the FIDE threefold claim rule
   [rust-search][rust-search].

The end-to-end witness starts from the standard initial position:

```text
1.d4 e5 2.dxe5 Nf6 3.e4 Nxe4 4.Nf3 Nc5 5.Nc3 g6
6.Bf4 Bg7 7.Qd2 O-O 8.h3 Re8 9.a3 d5
10.Ng1 Nbd7 11.Nf3 Nb8 12.Ng1 Nbd7 13.Nf3 Nb8
```

Immediately after `9...d5`, `e5xd6 e.p.` is pseudo-legal but illegal because
moving the e5-pawn exposes White's king on e1 to the rook on e8. FIDE identity
therefore erases the raw `d6` target. Each reversible four-ply knight cycle
returns to that same FIDE position after the raw target has expired. The pinned
oracle reports three current FIDE-key occurrences but only two occurrences of
the later Polyglot hash; its exact run is recorded in
[repetition-ep-output.txt](data/repetition-ep-output.txt), including a composite
SHA-256 of the four Rust repetition sources. The Rust regression replays the
same full history, asserts `Game::repetition_count() == 3`, and independently
simulates the legacy Polyglot count `2`
[rust-repetition-test][rust-repetition-test].

This is a repaired integration hazard, not a current `Game` undercount. It is
also a cross-validated diagnostic rather than a concrete Lean theorem: the Lean
result proves the generic modeled identity, while the Python and Rust artifacts
establish this particular external history. A future
`lean-theorem-instance` must replay these exact moves and prove the count before
the item can carry that stronger label.

This one result yields three human-facing artifacts:

- **Rule card:** same placement, side, and castling rights are not sufficient;
  en passant matters exactly when it creates a legal move.
- **Exception pair:** show the position immediately after `9...d5` beside the
  first knight-cycle return and ask whether the expired raw target changes
  FIDE identity before showing the pin.
- **System check:** reject any exercise export or game-history count that uses
  a raw FEN field or Polyglot hash as exact FIDE identity.

That is the desired pipeline in miniature: formal definition, executable
history, exact engine repair, explicitly bounded assurance, and a memorable
exception drill.

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
only after it carries one primary assurance value and the interface displays
that value. Exact-answer grading requires a concrete theorem instance, checked
certificate, pinned oracle, or engine regression. An `engine-estimate` must show
its search budget; a `human-hypothesis` may enter a study queue but cannot grade
an answer as fact.

The dashboard should privilege delayed first-attempt accuracy, exception false
positives, and unseen transfer.  Engine-match percentage and centipawn loss are
useful diagnostics, but optimizing them alone trains imitation of one search
configuration rather than stable chess knowledge.

## Promotion and stop rules

Promote a knowledge item only when:

- its semantics are named (`FIDE game`, history-free board, WDL, DTZ, or DTM);
- its primary assurance label matches the exact artifact actually present;
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
- **trainer-main** — `engine/src/bin/chess-trainer/main.rs`, free-play and private-review HTTP API plus fixed-depth UCI grading, working-tree snapshot inspected 15 July 2026.
- **trainer-app** — `engine/src/bin/chess-trainer/app.js`, free-play reps and persistent diagnostic-review UI, working-tree snapshot inspected 15 July 2026.
- **trainer-review** — `engine/src/bin/chess-trainer/review.rs`, append-only event log, replayed state, and fixed pilot scheduler, working-tree snapshot inspected 15 July 2026.
- **fide2023** — International Chess Federation, *FIDE Laws of Chess Taking Effect from 1 January 2023*, Articles 5.2.2, 9.2.3, 9.3, and 9.6, approved 7 August 2022.
- **lean-repetition-key** — `Chess/RepetitionKey.lean`, exact executable repetition key and equivalence theorem, local formalization repository snapshot inspected 14 July 2026.
- **python-chess112** — Niklas Fiekas, `python-chess` v1.11.2, `chess/polyglot.py`, released 25 February 2025; the implementation states that legality of a potential en-passant capture is irrelevant to the Polyglot hash. The `chess-1.11.2.tar.gz` SHA-256 used in the validation ledger is `a8b43e5678fdb3000695bdaa573117ad683761e5ca38e591c4826eba6d25bb39`.
- **rust-board** — `engine/src/board.rs`, `Board::hash` and `ep_hash_contribution`, imported Rust snapshot inspected 14 July 2026.
- **rust-repetition** — `engine/src/repetition.rs`, structural `RepetitionKey`, legally effective en-passant component, and canonical position ID, working-tree snapshot inspected 14 July 2026.
- **rust-game** — `engine/src/game.rs`, exact `Game::position_keys` and `Game::repetition_count`, working-tree snapshot inspected 14 July 2026.
- **rust-search** — `engine/src/search.rs`, exact structural search history and internal earlier-ancestor repetition heuristic, working-tree snapshot inspected 14 July 2026.
- **rust-repetition-test** — `engine/tests/repetition.rs`, full-start en-passant history asserting exact count 3 and simulated legacy Polyglot count 2, working-tree snapshot inspected 14 July 2026.
- **roediger-karpicke2006** — Henry L. Roediger III and Jeffrey D. Karpicke, “Test-Enhanced Learning: Taking Memory Tests Improves Long-Term Retention,” *Psychological Science* 17(3), 2006, 249–255. DOI 10.1111/j.1467-9280.2006.01693.x.
- **cepeda2008** — Nicholas J. Cepeda, Edward Vul, Doug Rohrer, John T. Wixted, and Harold Pashler, “Spacing Effects in Learning: A Temporal Ridgeline of Optimal Retention,” *Psychological Science* 19(11), 2008, 1095–1102. DOI 10.1111/j.1467-9280.2008.02209.x.

[trainer-book]: ../../../engine/src/bin/chess-trainer/book.rs
[trainer-main]: ../../../engine/src/bin/chess-trainer/main.rs
[trainer-app]: ../../../engine/src/bin/chess-trainer/app.js
[trainer-review]: ../../../engine/src/bin/chess-trainer/review.rs
[fide2023]: https://handbook.fide.com/chapter/E012023
[lean-repetition-key]: ../../../Chess/RepetitionKey.lean
[python-chess112]: https://github.com/niklasf/python-chess/blob/v1.11.2/chess/polyglot.py
[rust-board]: ../../../engine/src/board.rs
[rust-repetition]: ../../../engine/src/repetition.rs
[rust-game]: ../../../engine/src/game.rs
[rust-search]: ../../../engine/src/search.rs
[rust-repetition-test]: ../../../engine/tests/repetition.rs
[roediger-karpicke2006]: https://doi.org/10.1111/j.1467-9280.2006.01693.x
[cepeda2008]: https://doi.org/10.1111/j.1467-9280.2008.02209.x
