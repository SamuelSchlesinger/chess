#!/usr/bin/env python3
"""Reproduce the Polyglot-key/FIDE-repetition en-passant counterexample.

The position contains an adjacent pawn that can capture en passant
pseudo-legally, but the capture is illegal because it exposes its own king.
Polyglot therefore hashes the raw en-passant file while the FIDE repetition
key must erase it.

Dependency and invocation from the formalization repository root:

    uv run --with chess==1.11.2 python \
      research/novel-chess-theory/certified-chess-knowledge/data/repetition_ep_counterexample.py
"""

from __future__ import annotations

import argparse
from pathlib import Path
import chess
import chess.polyglot


WITH_EP = "8/8/8/8/k2Pp2Q/8/8/3K4 b - d3 0 1"
WITHOUT_EP = "8/8/8/8/k2Pp2Q/8/8/3K4 b - - 0 1"
EXPECTED_VERSION = "1.11.2"
OUTPUT = Path(__file__).resolve().parent / "repetition-ep-output.txt"


def fide_key(board: chess.Board) -> tuple[str, bool, str, int | None]:
    """The four components of FIDE Article 9.2.3 used by this project."""

    effective_ep = board.ep_square if board.has_legal_en_passant() else None
    return board.board_fen(), board.turn, board.castling_xfen(), effective_ep


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if chess.__version__ != EXPECTED_VERSION:
        raise SystemExit(
            f"expected chess=={EXPECTED_VERSION}, got chess=={chess.__version__}"
        )
    with_ep = chess.Board(WITH_EP)
    without_ep = chess.Board(WITHOUT_EP)

    assert with_ep.has_pseudo_legal_en_passant()
    assert not with_ep.has_legal_en_passant()
    assert fide_key(with_ep) == fide_key(without_ep)

    with_hash = chess.polyglot.zobrist_hash(with_ep)
    without_hash = chess.polyglot.zobrist_hash(without_ep)
    assert with_hash != without_hash

    report = (
        "pseudo-legal en passant: yes\n"
        "legal en passant: no\n"
        "FIDE repetition keys equal: yes\n"
        f"Polyglot hash with d3:    0x{with_hash:016x}\n"
        f"Polyglot hash without d3: 0x{without_hash:016x}\n"
        "Polyglot hashes equal: no\n"
    )
    if args.check:
        if report != OUTPUT.read_text(encoding="utf-8"):
            raise SystemExit("repetition-ep-output.txt is stale")
        print("repetition-ep-output.txt matches the pinned oracle")
    else:
        print(report, end="")


if __name__ == "__main__":
    main()
