#!/usr/bin/env python3
"""Measure route-sensitive preparation structure in the pinned opening corpus.

This is a deliberately small *discriminating* pilot.  It checks whether the
curated corpus contains enough structure to distinguish:

1. move-history decision nodes from repetition-key decision nodes;
2. same-repetition-key-endpoint histories whose summed opponent branch
   incidences differ;
3. high-multiplicity transposition nodes with many downstream terminal
   histories in the taxonomy.

It does not estimate move popularity, practical danger, or chess strength.  The
pinned opening-name corpus is a taxonomy, not a game sample.  Those quantities
require the larger-game experiment described in ../experiments.md.

Dependency:

    uv run --with chess==1.11.2 python \
      research/novel-chess-theory/opening-decisions/data/pilot.py
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import itertools
from collections import Counter, defaultdict
from contextlib import redirect_stdout
from dataclasses import dataclass
from pathlib import Path
from typing import DefaultDict, Dict, List, Mapping, Set, Tuple

import chess


EXPECTED_CHESS_VERSION = "1.11.2"
EXPECTED_SHA256 = "fd710d16bf5cdd750a565ee1a6aba19eb2c7db7d74d7df961f6e00fb1cd04a6e"
ROOT = Path(__file__).resolve().parents[4]
INPUT = ROOT / "data" / "lichess-openings" / "all.tsv"
OUTPUT = Path(__file__).resolve().parent / "output.txt"

EXPECTED_PILOT = {
    "white history decision nodes": 3_091,
    "white repetition-key decision nodes": 2_741,
    "black history decision nodes": 3_102,
    "black repetition-key decision nodes": 2_736,
    "white opponent-projection-matched pairs": 270,
    "white pairs with catalog-incidence difference": 247,
    "white pairs with legal-incidence difference": 32,
    "black opponent-projection-matched pairs": 265,
    "black pairs with catalog-incidence difference": 220,
    "black pairs with legal-incidence difference": 98,
    "top hub score": 577,
    "top hub route multiplicity": 2,
    "top hub downstream terminal histories": 577,
}

History = Tuple[str, ...]
Placement = Tuple[str, ...]
CastlingRights = Tuple[bool, bool, bool, bool]
PositionKey = Tuple[Placement, bool, CastlingRights, int | None]


def position_key(board: chess.Board) -> PositionKey:
    """The project's exact FIDE repetition key, including effective EP only."""

    placement = tuple(
        piece.symbol() if (piece := board.piece_at(square)) is not None else "."
        for square in chess.SQUARES
    )
    rights = (
        board.has_kingside_castling_rights(chess.WHITE),
        board.has_queenside_castling_rights(chess.WHITE),
        board.has_kingside_castling_rights(chess.BLACK),
        board.has_queenside_castling_rights(chess.BLACK),
    )
    effective_ep = board.ep_square if board.has_legal_en_passant() else None
    return placement, board.turn, rights, effective_ep


def san(history: History) -> str:
    board = chess.Board()
    parts: List[str] = []
    for ply, token in enumerate(history):
        move = chess.Move.from_uci(token)
        notation = board.san(move)
        if ply % 2 == 0:
            parts.append(f"{ply // 2 + 1}.{notation}")
        else:
            parts[-1] += f" {notation}"
        board.push(move)
    return " ".join(parts) or "(initial position)"


@dataclass(frozen=True)
class DeviationEvent:
    """A typed, route-local alternative at one opponent decision.

    Keeping the decision index, pre-deviation key, move, and child key avoids
    conflating alternatives from different plies.  Events from different
    routes are not compared by set inclusion in this pilot: that requires the
    common opponent-scenario semantics specified in the formal model.
    """

    opponent_decision: int
    pre_deviation_key: PositionKey
    move: str
    child_key: PositionKey


@dataclass(frozen=True)
class RouteExposure:
    """Branch exposure along a fixed route, for one repertoire side.

    `catalog_events` preserves typed alternatives present in the curated
    corpus. The incidence counts sum alternatives separately at every opponent
    decision; they are not counts of unique moves or claims that any move is
    common, difficult, dangerous, or good.
    """

    catalog_events: frozenset[DeviationEvent]
    catalog_branch_incidences: int
    legal_branch_incidences: int


def exposure(
    history: History,
    repertoire_side: chess.Color,
    boards: Mapping[History, chess.Board],
    children: Mapping[History, Set[str]],
) -> RouteExposure:
    catalog_events: Set[DeviationEvent] = set()
    catalog_branch_incidences = 0
    legal_branch_incidences = 0
    opponent_decision = 0

    for ply, intended_token in enumerate(history):
        prefix = history[:ply]
        board = boards[prefix]
        if board.turn == repertoire_side:
            continue

        intended = chess.Move.from_uci(intended_token)
        catalog_alternatives = children[prefix] - {intended_token}
        catalog_branch_incidences += len(catalog_alternatives)
        legal_branch_incidences += sum(move != intended for move in board.legal_moves)
        for token in catalog_alternatives:
            child = board.copy(stack=False)
            child.push(chess.Move.from_uci(token))
            catalog_events.add(
                DeviationEvent(
                    opponent_decision,
                    position_key(board),
                    token,
                    position_key(child),
                )
            )
        opponent_decision += 1

    if len(catalog_events) != catalog_branch_incidences:
        raise AssertionError("typed catalog events unexpectedly collapsed")

    return RouteExposure(
        frozenset(catalog_events),
        catalog_branch_incidences,
        legal_branch_incidences,
    )


def side_moves(history: History, side: chess.Color) -> Tuple[str, ...]:
    parity = 0 if side == chess.WHITE else 1
    return history[parity::2]


def load() -> Tuple[
    Dict[History, chess.Board],
    Dict[History, Set[str]],
    Counter[History],
    Dict[History, str],
]:
    if chess.__version__ != EXPECTED_CHESS_VERSION:
        raise RuntimeError(
            f"expected chess=={EXPECTED_CHESS_VERSION}, got {chess.__version__}"
        )
    digest = hashlib.sha256(INPUT.read_bytes()).hexdigest()
    if digest != EXPECTED_SHA256:
        raise RuntimeError(f"unexpected input SHA-256: {digest}")

    boards: Dict[History, chess.Board] = {(): chess.Board()}
    children: DefaultDict[History, Set[str]] = defaultdict(set)
    terminal_counts: Counter[History] = Counter()
    terminal_names: Dict[History, str] = {}

    with INPUT.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        for row in reader:
            history: History = ()
            board = chess.Board()
            for token in row["uci"].split():
                move = chess.Move.from_uci(token)
                if move not in board.legal_moves:
                    raise RuntimeError(f"illegal corpus move {token} after {history}")
                children[history].add(token)
                board.push(move)
                history += (token,)
                old = boards.get(history)
                if old is None:
                    boards[history] = board.copy(stack=False)
                elif old.fen(en_passant="fen") != board.fen(en_passant="fen"):
                    raise RuntimeError(f"non-deterministic history: {history}")
            terminal_counts[history] += 1
            terminal_names[history] = row["name"]

    return boards, dict(children), terminal_counts, terminal_names


def descendant_terminals(
    boards: Mapping[History, chess.Board], terminal_counts: Mapping[History, int]
) -> Dict[History, Set[History]]:
    descendants: Dict[History, Set[History]] = {
        history: ({history} if terminal_counts.get(history, 0) else set())
        for history in boards
    }
    for history in sorted(boards, key=len, reverse=True):
        if history:
            descendants[history[:-1]].update(descendants[history])
    return descendants


def fmt_side(side: chess.Color) -> str:
    return "White" if side == chess.WHITE else "Black"


def run() -> None:
    boards, children, terminal_counts, terminal_names = load()
    descendants = descendant_terminals(boards, terminal_counts)

    fibres: DefaultDict[PositionKey, List[History]] = defaultdict(list)
    for history, board in boards.items():
        fibres[position_key(board)].append(history)

    print(f"python-chess: {chess.__version__}")
    print(f"input SHA-256: {EXPECTED_SHA256}")
    print(f"histories: {len(boards)}")
    print(f"exact repetition positions: {len(fibres)}")
    print()

    actual: Dict[str, int] = {}

    print("decision-node quotient (corpus-relative)")
    for side in (chess.WHITE, chess.BLACK):
        decision_histories = [
            history
            for history, board in boards.items()
            if history in children and board.turn == side
        ]
        decision_positions = {position_key(boards[h]) for h in decision_histories}
        saving = len(decision_histories) - len(decision_positions)
        percent = 100.0 * saving / len(decision_histories)
        side_key = fmt_side(side).lower()
        actual[f"{side_key} history decision nodes"] = len(decision_histories)
        actual[f"{side_key} repetition-key decision nodes"] = len(decision_positions)
        print(
            f"  {fmt_side(side)}: {len(decision_histories)} history decision nodes -> "
            f"{len(decision_positions)} repetition-key decision nodes; "
            f"duplicate-key differential {saving} ({percent:.2f}%)"
        )
    print()

    # These are projection-matched recorded histories, not yet controllable
    # route policies: the opponent's raw UCI projection is identical while the
    # selected side's projection differs. Gate 2 constructs conditional policies
    # over a common opponent-scenario space before using control language.
    candidates: Dict[
        chess.Color,
        List[Tuple[int, History, History, RouteExposure, RouteExposure]],
    ] = {
        chess.WHITE: [],
        chess.BLACK: [],
    }
    for histories in fibres.values():
        for left, right in itertools.combinations(histories, 2):
            if len(left) != len(right):
                continue
            for side in (chess.WHITE, chess.BLACK):
                if side_moves(left, not side) != side_moves(right, not side):
                    continue
                if side_moves(left, side) == side_moves(right, side):
                    continue
                left_exposure = exposure(left, side, boards, children)
                right_exposure = exposure(right, side, boards, children)
                delta = abs(
                    left_exposure.catalog_branch_incidences
                    - right_exposure.catalog_branch_incidences
                )
                candidates[side].append((delta, left, right, left_exposure, right_exposure))

    print(
        "same-repetition-key-endpoint history comparisons under identical "
        "opponent UCI projection"
    )
    for side in (chess.WHITE, chess.BLACK):
        with_catalog_difference = sum(
            left.catalog_branch_incidences != right.catalog_branch_incidences
            for _, _, _, left, right in candidates[side]
        )
        with_legal_difference = sum(
            left.legal_branch_incidences != right.legal_branch_incidences
            for _, _, _, left, right in candidates[side]
        )
        side_key = fmt_side(side).lower()
        actual[f"{side_key} opponent-projection-matched pairs"] = len(candidates[side])
        actual[f"{side_key} pairs with catalog-incidence difference"] = (
            with_catalog_difference
        )
        actual[f"{side_key} pairs with legal-incidence difference"] = (
            with_legal_difference
        )
        print(
            f"  {fmt_side(side)}: {len(candidates[side])} projection-matched pairs; "
            f"summed catalog branch incidences differ in {with_catalog_difference}; "
            f"summed legal branch incidences differ in {with_legal_difference}"
        )
    print()

    print("largest curated branch-exposure contrasts")
    for side in (chess.WHITE, chess.BLACK):
        ranked = sorted(
            candidates[side],
            key=lambda item: (item[0], item[1], item[2]),
            reverse=True,
        )
        print(f"  {fmt_side(side)}")
        emitted = 0
        for delta, left, right, left_exposure, right_exposure in ranked:
            if delta == 0:
                break
            print(
                f"    catalog incidences {left_exposure.catalog_branch_incidences} vs "
                f"{right_exposure.catalog_branch_incidences}; legal incidences "
                f"{left_exposure.legal_branch_incidences} vs "
                f"{right_exposure.legal_branch_incidences}"
            )
            print(f"      A: {san(left)}")
            print(f"      B: {san(right)}")
            emitted += 1
            if emitted == 3:
                break
        if emitted == 0:
            print("    (none)")
    print()

    # Hub score is intentionally descriptive: how many distinct trie histories
    # share a position, and how many terminal histories remain below any of those
    # representatives.  It is not a popularity or strength estimate.
    hubs: List[Tuple[int, int, int, PositionKey, List[History], Set[History]]] = []
    for key, histories in fibres.items():
        if len(histories) < 2:
            continue
        downstream: Set[History] = set()
        for history in histories:
            downstream.update(descendants[history])
        leverage = (len(histories) - 1) * len(downstream)
        hubs.append((leverage, len(histories), len(downstream), key, histories, downstream))

    print("top taxonomy transposition hubs")
    ranked_hubs = sorted(
        hubs, reverse=True, key=lambda item: (item[0], item[1], item[2])
    )
    top_hub = ranked_hubs[0]
    actual["top hub score"] = top_hub[0]
    actual["top hub route multiplicity"] = top_hub[1]
    actual["top hub downstream terminal histories"] = top_hub[2]
    for (
        taxonomy_score,
        multiplicity,
        downstream_count,
        _,
        histories,
        downstream,
    ) in ranked_hubs[:10]:
        representative = min(histories, key=lambda history: (len(history), history))
        names = sorted({terminal_names[h] for h in downstream})
        name_sample = "; ".join(names[:3]) + ("; ..." if len(names) > 3 else "")
        print(
            f"  score={taxonomy_score}, routes={multiplicity}, downstream terminal histories="
            f"{downstream_count}: {san(representative)}"
        )
        print(f"    {name_sample or '(no named endpoint below this corpus node)'}")

    failures = [
        f"{label}: expected {expected}, got {actual.get(label)!r}"
        for label, expected in EXPECTED_PILOT.items()
        if actual.get(label) != expected
    ]
    if failures:
        raise RuntimeError("pilot aggregate mismatch:\n  " + "\n  ".join(failures))
    print()
    print("all pilot aggregate checks match")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    stream = io.StringIO()
    with redirect_stdout(stream):
        run()
    report = stream.getvalue()
    if args.check:
        if report != OUTPUT.read_text(encoding="utf-8"):
            raise SystemExit("output.txt is stale")
        print("output.txt matches the pinned opening-decision pilot")
    else:
        print(report, end="")


if __name__ == "__main__":
    main()
