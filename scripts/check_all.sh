#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

echo "[1/5] Lean build"
lake build

echo "[2/5] Pinned Lean corpora"
lake exe chess_validate

echo "[3/5] Rust engine and applications"
cargo test --manifest-path engine/Cargo.toml

echo "[4/5] Research structure and dependency-free pilots"
python3 research/novel-chess-theory/certified-chess-knowledge/data/check_corpus.py
python3 research/novel-chess-theory/certified-chess-knowledge/data/rank_candidates.py >/dev/null
python3 research/novel-chess-theory/certified-chess-knowledge/data/state_space_bounds.py >/dev/null

echo "[5/5] Pinned python-chess pilots"
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/transposition-algebra/data/classify_transpositions.py >/dev/null
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/opening-decisions/data/pilot.py >/dev/null
uv run --with chess==1.11.2 python \
  research/novel-chess-theory/certified-chess-knowledge/data/repetition_ep_counterexample.py >/dev/null

echo "all chess validations passed"
