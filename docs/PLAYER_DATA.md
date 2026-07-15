# Player data and privacy

Personal games are evidence for the training system, not source fixtures for
the public chess semantics. Raw exports remain untracked wherever they are
stored. Generated learner profiles are accepted by default only under
`data/private/`, which is ignored by Git.

The reusable importer is `scripts/player_games.py`. It has two assurance
layers:

1. The metadata layer validates the multi-game PGN with the python-chess
   library, installed as pinned `chess==1.11.2`, and reports exact
   sample-relative counts.
2. The optional engine layer analyzes completed, project-filtered games with a
   declared Stockfish executable and node budget. The default control filter is
   `base + 40 × increment >= 600s`, an estimate rather than observed duration.
   The executable must identify itself as Stockfish and expose `Threads`,
   `Hash`, and `Clear Hash`; the importer pins `Threads = 1` and `Hash = 64`.
   It compares best and played moves from the same root, invokes `Clear Hash`,
   and starts a new engine game before each unrestricted or restricted root
   search. For each selected game it retains the earliest large-loss
   position, or the earliest medium-loss position if no large one exists, then
   reruns it at the confirmation budget. Abandoned or unfinished games are
   excluded by default. Loss values and thresholds are bounded estimates used
   to rank review positions, not objective truth or a diagnosis of why a human
   made a move.

The importer rejects parser errors without echoing python-chess's identifying
error log, header/movetext result disagreements, ambiguous player inference,
duplicate games, and collisions among the PGN, Stockfish executable, and
outputs. It rejects existing symlink or multiply linked output files during
validation and atomically replaces each generated artifact, so a path change
during a long engine run cannot redirect a truncating write. By default it also
rejects raw PGNs inside the worktree outside `data/private/`, artifact paths
outside that private directory, and personal profiles on stdout. The narrowly
named `--allow-worktree-input`, `--allow-output-outside-private`, and
`--allow-stdout` flags are explicit privacy overrides.

Run the local profile from the repository root:

```sh
uv run --with chess==1.11.2 python scripts/player_games.py \
  /path/to/chess.com-export.pgn --player YOUR_USERNAME \
  --stockfish /path/to/stockfish --nodes 20000 --confirmation-nodes 100000 \
  --json-output data/private/player-baseline.json \
  --markdown-output data/private/player-baseline.md \
  --cards-output data/private/diagnostic-cards.json
```

The JSON retains stable game digests, per-game and aggregate metadata, and the
confirmed per-game candidate positions needed to build diagnostic exercises.
The digest hashes stable occurrence fields and complete UCI mainline while
excluding mutable rating and termination headers, so equivalent re-exports do
not orphan review state. It deliberately does not copy raw movetext into a
tracked file. The raw PGN remains the local source of truth. Generated private
files are written with mode `0600`. JSON is the current inspection/interchange
artifact. The diagnostic trainer now keeps its mutable review history in a
separate private append-only JSONL log; a future profile database may replace
that log only through an explicit migration.

The diagnostic-card bundle retains complete UCI occurrence history, raw FEN,
canonical effective `PositionId`, UCI move identities, and the confirmed engine
reference. It calls the engine move a reference rather than a correct answer.
The full history remains re-identifiable even without names, so the bundle is
private too. Both JSON artifacts carry an explicit
`personal-reidentifiable` privacy classification.

The engine executable is copied once to a private temporary snapshot; both
passes execute that snapshot, whose exact digest is recorded. PGN statistics
and source hash likewise come from one in-memory byte snapshot. The run records
UTC time, parser and analysis-algorithm versions, script digest, source digest,
engine digest, options, search-isolation protocol, budgets, thresholds, and
selection policy.

Each card has two content-addressed versions. `content_version` hashes the
tested semantics—occurrence, complete state, task, orientation, and reference
move—and is part of the future scheduler key. Prose and replaceable analysis
details do not reset mastery. The bundle's `analysis_config_version` hashes its
deterministic run configuration and input provenance. Each card's
`evidence_version` additionally hashes that card's reference and played PVs,
scores, loss bucket, and mate event. Evidence changes therefore remain visible
without silently changing the question.

## Private diagnostic review

The first personal deck is a six-card, manually curated subset of the 24
engine-ranked candidates. Run it from the repository root with:

```sh
scripts/run_personal_trainer.sh
```

If Stockfish is not on `PATH`, set it explicitly:

```sh
STOCKFISH=/path/to/stockfish scripts/run_personal_trainer.sh
```

The script reads `data/private/diagnostic-cards.json` and
`data/private/initial-six.json`, and appends observations to
`data/private/review-events-v1.jsonl`. The log is the source of truth; current
progress is rebuilt by replaying it. Before returning an answer, the server
fsyncs an answer-release record containing the card and evidence versions,
timestamps, move response, latency, reference match, hint use, and the exact
private feedback and analysis provenance that were shown. Grading appends the
self-grade and resulting schedule decision. An unfinished answer release is
restored after restart and cannot expire until graded. The log does not store
the player's typed reason. It is created with mode `0600` and refuses symlink or
multiply linked targets.

The pilot uses three human judgments: `pass` means the tactical idea was found,
`partial` means only the move or idea was found, and `miss` means it was not
found. A hint always applies a miss regardless of the submitted grade. Successive
passes schedule 2, 4, 7, 14, 30, and 60 days; partial schedules one day and
resets the success rung; miss schedules ten minutes, resets the rung, and adds a
lapse. These constants are transparent pilot policy, not an optimized model of
chess memory. The deck's `new_per_day` limit counts durable first answer
releases in a rolling 24-hour window, avoiding an implicit UTC or
machine-timezone day. An abandoned prompt whose answer was never revealed does
not consume that allowance.

Review state is keyed by `(card_id, content_version)`. A semantic change creates
new state; a change only to `evidence_version` remains visible in later events
without resetting the schedule. Matching the single engine reference is logged
as an observation, never treated as proof that another legal move was wrong.
The local server admits only its exact loopback Host, rejects foreign browser
Origins, and protects every answer-issuing or state-changing request with an
in-page capability. The review log is exclusively locked while the trainer is
running, so a second process cannot append a competing event sequence.

The source games and selected cards are discovery and training data, not a
held-out test. Games played after 2026-07-14 are the prospective stream for
checking whether the trained ideas transfer to play. Until that evidence exists,
the pilot makes no claim of practical improvement or rating gain.

Before publishing any personal artifact, decide separately whether to expose:

- account name and opponents;
- raw games or only derived aggregates;
- rating and time-control history;
- engine-ranked mistakes and training progress.

The default for every category is private.
