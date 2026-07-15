# Monorepo architecture

This repository has one purpose across three layers: produce exact chess
semantics, evidence-backed chess knowledge, and a trainer that a human can
actually use. The layers share identities and data contracts, but not trust
claims.

## Repository ownership

| Path | Owns | Must not own |
|---|---|---|
| `Chess/`, `Chess.lean` | FIDE state semantics, legal transitions, proofs, exact keys, corpus certificates | Engine-strength or pedagogical claims |
| `data/`, `scripts/` | One pinned copy of each dataset, hashes, licenses, reproducible validation | Generated caches or neural-network binaries |
| `engine/` | Imported Rust move generation, search, UCI, analysis UI, repertoire service, trainer | A competing definition of position identity or duplicated corpora |
| `research/` | Evidence, designs, experiments, limitations, and research artifacts | Runtime production data or uncited product claims |
| `schemas/` | Versioned interchange schemas and fixtures shared by Lean and Rust | Implementation-specific internal types |

When `~/projects/games/chess` is imported as `engine/`, preserve its history or
record its source commit and import method in `engine/UPSTREAM.md`. After the
import, `engine/` is the sole maintained Rust implementation; do not keep a
second synchronized source tree.

## Canonical cross-layer position identity

The only persistent position identity is `PositionId`, serialized as canonical
effective four-field EPD:

```text
<piece-placement> <side-to-move> <clean-castling-rights> <effective-ep-target>
```

The en-passant field is a square only when an en-passant capture is currently
legal; otherwise it is `-`. Castling rights are normalized to rights that
remain valid in the modeled position. Serialization uses canonical FEN board
placement, `w`/`b`, ordered `KQkq` rights or `-`, lowercase algebraic squares,
single ASCII spaces, and no trailing space.

The interchange boundary assigns `PositionId` only to well-formed positions.
Malformed internal states, such as stale castling bits on absent rooks, must be
rejected or repaired before serialization; they must not acquire a silently
different identity in Lean and Rust.

This is the executable form of Lean's `RepetitionKey` and FIDE repetition
position equality on well-formed reachable positions. Both Lean and Rust must
satisfy:

```text
PositionId(p) = PositionId(q)  iff  p and q are the same repetition position.
```

A Polyglot/Zobrist hash may index a cache, but it is never an ID, database key,
foreign key, card key, or equality witness. Hash-table hits must be confirmed
by `PositionId` or exact field equality.

`PositionId` intentionally excludes clocks and prior occurrences. Complete
game adjudication uses a separate `GameState` containing the current position,
half-move/full-move state, repetition history, and outcome-relevant metadata.
Do not use `PositionId` alone to decide repetition or 50/75-move draws.

## Shared repertoire graph

The durable repertoire model is a graph, not a list of SAN strings. Schema
fields use UCI for moves and `PositionId` for nodes; SAN is derived display
text.

```text
RepertoireNode
  position_id: PositionId
  repertoire_side: white | black
  choices: [MoveChoice]
  concepts: [ClaimRef]

MoveChoice
  move_uci: UciMove
  target_id: PositionId
  priority: main | alternate | avoid
  claims: [ClaimRef]

RouteOccurrence
  route_id: stable source-scoped ID
  start_id: PositionId
  moves_uci: [UciMove]
  source: SourceRef
  labels: [string]

RouteException
  route_prefix: [UciMove]
  opponent_move_uci: UciMove
  claim: ClaimRef
```

`RepertoireNode` merges transposed routes. `RouteOccurrence` preserves source
and opening-name provenance. `RouteException` records why two routes that
eventually transpose are not equally useful before the merge. Counts and names
are occurrence metadata unless a certified or measured projection explicitly
aggregates them at a node.

## Shared training cards

Content and scheduling state are separate. Card content is versioned and
rebuildable; personal review history is mutable user data.

```text
Card
  card_id: stable content ID
  kind: position_move | transposition | deviation | concept
  position_id: PositionId?
  route_context: [UciMove]?
  prompt: structured prompt
  accepted_moves_uci: [UciMove]
  claims: [ClaimRef]
  content_version: integer

ReviewState
  learner_id: local profile ID
  card_id: CardId
  due_at, interval, ease, lapses, last_result
```

- `position_move` cards are keyed by position, so transpositions do not create
  duplicate reviews.
- `transposition` cards teach a route relation or recognition of a shared node.
- `deviation` cards retain the route prefix where opponent options differ.
- `concept` cards carry plans or motifs and are pedagogical unless separately
  supported by measured or certified claims.

Changing prose should not erase review history. Changing the tested position,
answer, route context, or card kind creates a new `card_id` or explicit
migration.

## Claim provenance

Every externally visible assertion carries one of three provenance tiers.
Tier applies per claim, not per file or record.

| Tier | Meaning | Required evidence |
|---|---|---|
| `certified` | Established by the Lean model or a checked certificate | Theorem/certificate name, source revision, validator version |
| `measured` | Reproducibly observed in a corpus or engine run | Dataset hash, script/config, engine and limit/version, timestamp |
| `pedagogical` | Authored explanation, plan, mnemonic, or training judgment | Author/reviewer, rationale, revision; no implication of proof |

Examples: move legality and exact transposition equality can be `certified`;
game frequency and engine evaluation are `measured`; “develop before starting
a kingside attack” is `pedagogical`. A UI must not render all three as equally
certain. Combining claims retains each field's provenance; it never promotes a
measured or pedagogical statement to certified.

## Validation boundaries

Lean is authoritative for modeled semantics, not playing strength. Rust is
authoritative for the shipped executable behavior after it passes shared
conformance tests, not for theorem truth. External engines are measurement
oracles, not part of the proof trust base.

Required validation chain:

1. `lake build` and `lake exe chess_validate` prove/build the semantics and
   replay pinned corpora.
2. Shared fixtures exercise castling loss, legal and pinned en passant,
   promotion, transpositions, and clock/history distinctions. Lean and Rust
   must emit identical `PositionId` values for every fixture.
3. `cargo test --manifest-path engine/Cargo.toml --release` validates Rust
   move generation, SAN/UCI/FEN interop, outcomes, and shared-schema decoding.
4. Corpus differential tests replay the same UCI histories through both
   implementations and compare `PositionId` plus legal-move sets at selected
   or all prefixes.
5. Engine evaluations record engine binary/hash, options, depth or time, and
   position ID. They may inform measured claims but cannot certify best play.
6. Trainer tests verify graph lookup across transpositions, preservation of
   route exceptions, stable card IDs, and schedule migration independently of
   chess-engine grading.

A cross-layer mismatch blocks release. It is resolved against the FIDE rule
and Lean specification, with a regression fixture added before either side is
changed.

## Data and neural-network policy

- Keep exactly one repository copy of each pinned corpus under `data/`, with
  source revision, license, SHA-256, and reproduction instructions.
- `engine/` and research scripts read canonical data in place. They may create
  ignored caches, indexes, or shards, but must not commit transformed copies.
- Small derived fixtures may be committed only when generated by a checked-in
  deterministic script and when their source hash is recorded.
- Large game dumps, tablebases, engine binaries, training checkpoints, and
  neural-network files are not committed. A versioned manifest records URL or
  origin, license, SHA-256, format, training provenance, and expected filename.
- Downloads land in a content-addressed ignored cache such as
  `.cache/chess/<sha256>/`; all consumers reference the manifest and hash.
- A locally trained net remains an ignored artifact until its dataset,
  configuration, code revision, evaluation, license, and hash are recorded.
- CI may restore caches by hash, but correctness must never depend on an
  unpinned mutable URL or a developer's private filesystem.

## Change workflow

1. Change semantics in Lean first when identity or legality changes.
2. Add certified fixtures and update schema versions.
3. Bring Rust into conformance without redefining the contract.
4. Rebuild measured artifacts from pinned inputs; never hand-edit outputs.
5. Review pedagogical content separately from semantic and empirical claims.
6. Run the complete validation chain before publishing trainer content.

This dependency direction keeps the monorepo honest: proofs define exact
meaning, measurements describe observed chess, and pedagogy turns both into
human practice without blurring their assurance levels.
