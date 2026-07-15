#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repo_root"

engine=${STOCKFISH:-stockfish}

exec cargo run --release --manifest-path engine/Cargo.toml --bin chess-trainer -- \
  --engine "$engine" \
  --cards data/private/diagnostic-cards.json \
  --deck data/private/initial-six.json \
  --review-log data/private/review-events-v1.jsonl \
  "$@"
