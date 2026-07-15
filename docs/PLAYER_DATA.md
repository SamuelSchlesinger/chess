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
artifact; a future persistent trainer will use a private, migrated profile
database with content-addressed imports and occurrence-level history.

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

Before publishing any personal artifact, decide separately whether to expose:

- account name and opponents;
- raw games or only derived aggregates;
- rating and time-control history;
- engine-ranked mistakes and training progress.

The default for every category is private.
