#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

echo "[1/6] Pinned data integrity"
shasum -a 256 -c data/SHA256SUMS

echo "[2/6] Lean build"
lake build

echo "[3/6] Pinned Lean corpora"
lake exe chess_validate

echo "[4/6] Rust engine and applications"
cargo test --manifest-path engine/Cargo.toml

echo "[5/6] Research structure and dependency-free pilots"
python3 research/novel-chess-theory/certified-chess-knowledge/data/check_corpus.py
python3 research/novel-chess-theory/certified-chess-knowledge/data/rank_candidates.py --check
python3 research/novel-chess-theory/certified-chess-knowledge/data/state_space_bounds.py --check

echo "[6/6] Pinned python-chess pilots"
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/transposition-algebra/data/classify_transpositions.py >/dev/null
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/opening-decisions/data/pilot.py --check
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/certified-chess-knowledge/data/repetition_ep_counterexample.py --check
uv run --with chess==1.11.2 python scripts/player_games.py --self-test

echo "all chess validations passed"
