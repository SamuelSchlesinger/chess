#!/usr/bin/env python3
"""Reproduce a legal-history Polyglot/FIDE repetition counterexample.

The position contains an adjacent pawn that can capture en passant
pseudo-legally, but the capture is illegal because it exposes its own king.
Polyglot therefore hashes the raw en-passant file while the FIDE repetition
key must erase it.

The history starts from the ordinary initial position, reaches the pinned
en-passant state, and repeats a reversible knight cycle twice.  The current
FIDE position then occurs three times while the later Polyglot key occurs only
twice.

Dependency and invocation from the formalization repository root:

    uv run --with chess==1.11.2 python \
      research/novel-chess-theory/certified-chess-knowledge/data/repetition_ep_counterexample.py
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

import chess
import chess.polyglot


EXPECTED_VERSION = "1.11.2"
EXPECTED_SDIST_SHA256 = (
    "a8b43e5678fdb3000695bdaa573117ad683761e5ca38e591c4826eba6d25bb39"
)
SETUP_UCI = (
    "d2d4",
    "e7e5",
    "d4e5",
    "g8f6",
    "e2e4",
    "f6e4",
    "g1f3",
    "e4c5",
    "b1c3",
    "g7g6",
    "c1f4",
    "f8g7",
    "d1d2",
    "e8g8",
    "h2h3",
    "f8e8",
    "a2a3",
    "d7d5",
)
KNIGHT_CYCLE_UCI = ("f3g1", "b8d7", "g1f3", "d7b8")
DISPLAY_LINE = (
    "1.d4 e5 2.dxe5 Nf6 3.e4 Nxe4 4.Nf3 Nc5 5.Nc3 g6 "
    "6.Bf4 Bg7 7.Qd2 O-O 8.h3 Re8 9.a3 d5"
)
OUTPUT = Path(__file__).resolve().parent / "repetition-ep-output.txt"
REPOSITORY = Path(__file__).resolve().parents[4]
ENGINE_REPETITION_FILES = (
    "engine/src/repetition.rs",
    "engine/src/game.rs",
    "engine/src/search.rs",
    "engine/tests/repetition.rs",
)


def fide_key(board: chess.Board) -> tuple[str, bool, str, int | None]:
    """The four components of FIDE Article 9.2.3 used by this project."""

    effective_ep = board.ep_square if board.has_legal_en_passant() else None
    return board.board_fen(), board.turn, board.castling_xfen(), effective_ep


def push_uci(board: chess.Board, uci: str) -> None:
    move = chess.Move.from_uci(uci)
    assert board.is_legal(move), f"expected legal move {uci} from {board.fen()}"
    board.push(move)


def engine_repetition_snapshot_sha256() -> str:
    """Digest the exact Rust sources supporting the accompanying regression."""

    digest = hashlib.sha256()
    for relative in ENGINE_REPETITION_FILES:
        digest.update(relative.encode("utf-8"))
        digest.update(b"\0")
        digest.update((REPOSITORY / relative).read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if chess.__version__ != EXPECTED_VERSION:
        raise SystemExit(
            f"expected chess=={EXPECTED_VERSION}, got chess=={chess.__version__}"
        )
    board = chess.Board()
    fide_history = [fide_key(board)]
    polyglot_history = [chess.polyglot.zobrist_hash(board)]

    for uci in SETUP_UCI:
        push_uci(board, uci)
        fide_history.append(fide_key(board))
        polyglot_history.append(chess.polyglot.zobrist_hash(board))

    anchor_fen = board.fen(en_passant="fen")
    anchor_fide_key = fide_key(board)
    anchor_polyglot = chess.polyglot.zobrist_hash(board)
    ep_move = chess.Move.from_uci("e5d6")
    assert board.ep_square == chess.D6
    assert board.is_pseudo_legal(ep_move)
    assert not board.is_legal(ep_move)
    assert board.has_pseudo_legal_en_passant()
    assert not board.has_legal_en_passant()

    for _ in range(2):
        for uci in KNIGHT_CYCLE_UCI:
            push_uci(board, uci)
            fide_history.append(fide_key(board))
            polyglot_history.append(chess.polyglot.zobrist_hash(board))

    current_fide_key = fide_key(board)
    current_polyglot = chess.polyglot.zobrist_hash(board)
    fide_count = sum(key == current_fide_key for key in fide_history)
    polyglot_count = sum(key == current_polyglot for key in polyglot_history)
    assert current_fide_key == anchor_fide_key
    assert current_polyglot != anchor_polyglot
    assert fide_count == 3
    assert polyglot_count == 2

    report = (
        f"oracle: chess=={EXPECTED_VERSION}\n"
        f"declared sdist sha256: {EXPECTED_SDIST_SHA256}\n"
        f"engine repetition snapshot sha256: {engine_repetition_snapshot_sha256()}\n"
        f"legal setup: {DISPLAY_LINE}\n"
        f"anchor FEN: {anchor_fen}\n"
        f"repeated cycle (twice): {' '.join(KNIGHT_CYCLE_UCI)}\n"
        "pseudo-legal en passant: yes\n"
        "legal en passant: no\n"
        f"anchor Polyglot hash:  0x{anchor_polyglot:016x}\n"
        f"current Polyglot hash: 0x{current_polyglot:016x}\n"
        f"current FIDE-key occurrences: {fide_count}\n"
        f"current Polyglot-hash occurrences: {polyglot_count}\n"
    )
    if args.check:
        if report != OUTPUT.read_text(encoding="utf-8"):
            raise SystemExit("repetition-ep-output.txt is stale")
        print("repetition-ep-output.txt matches the pinned oracle")
    else:
        print(report, end="")


if __name__ == "__main__":
    main()
