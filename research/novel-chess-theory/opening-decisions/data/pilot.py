#!/usr/bin/env python3
"""Measure route-sensitive preparation structure in the pinned opening corpus.

This is a deliberately small *discriminating* pilot.  It checks whether the
curated corpus contains enough structure to distinguish:

1. move-word study cards from exact-position study cards;
2. routes to the same position whose opponent-facing branch exposure differs;
3. high-multiplicity transposition nodes with reusable downstream material.

It does not estimate move popularity, practical danger, or chess strength.  The
pinned opening-name corpus is a taxonomy, not a game sample.  Those quantities
require the larger-game experiment described in ../experiments.md.

Dependency:

    uv run --with chess==1.11.2 python \
      research/novel-chess-theory/opening-decisions/data/pilot.py
"""

from __future__ import annotations

import csv
import hashlib
import itertools
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import DefaultDict, Dict, List, Mapping, Set, Tuple

import chess


EXPECTED_CHESS_VERSION = "1.11.2"
EXPECTED_SHA256 = "fd710d16bf5cdd750a565ee1a6aba19eb2c7db7d74d7df961f6e00fb1cd04a6e"
ROOT = Path(__file__).resolve().parents[4]
INPUT = ROOT / "data" / "lichess-openings" / "all.tsv"

EXPECTED_PILOT = {
    "white history decision cards": 3_091,
    "white position decision cards": 2_741,
    "black history decision cards": 3_102,
    "black position decision cards": 2_736,
    "white controllable route pairs": 270,
    "white pairs with catalog-count difference": 247,
    "white pairs with legal-count difference": 32,
    "white strict catalog-outcome inclusions": 41,
    "black controllable route pairs": 265,
    "black pairs with catalog-count difference": 220,
    "black pairs with legal-count difference": 98,
    "black strict catalog-outcome inclusions": 46,
    "top hub score": 577,
    "top hub route multiplicity": 2,
    "top hub downstream endpoints": 577,
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
class RouteExposure:
    """Branch exposure along a fixed route, for one repertoire side.

    `catalog_outcomes` records exact child positions of alternative opponent
    moves present in this curated corpus. `catalog_count` counts alternatives
    at every encounter; `legal_count` does the same over all legal moves.  The
    latter is an opportunity-surface count, not a claim that every move is good.
    """

    catalog_outcomes: frozenset[PositionKey]
    catalog_count: int
    legal_count: int


def exposure(
    history: History,
    repertoire_side: chess.Color,
    boards: Mapping[History, chess.Board],
    children: Mapping[History, Set[str]],
) -> RouteExposure:
    catalog_outcomes: Set[PositionKey] = set()
    catalog_count = 0
    legal_count = 0

    for ply, intended_token in enumerate(history):
        prefix = history[:ply]
        board = boards[prefix]
        if board.turn == repertoire_side:
            continue

        intended = chess.Move.from_uci(intended_token)
        catalog_alternatives = children[prefix] - {intended_token}
        catalog_count += len(catalog_alternatives)
        legal_count += sum(move != intended for move in board.legal_moves)
        for token in catalog_alternatives:
            child = board.copy(stack=False)
            child.push(chess.Move.from_uci(token))
            catalog_outcomes.add(position_key(child))

    return RouteExposure(frozenset(catalog_outcomes), catalog_count, legal_count)


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


def main() -> None:
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

    print("decision-card compression (corpus-relative)")
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
        actual[f"{side_key} history decision cards"] = len(decision_histories)
        actual[f"{side_key} position decision cards"] = len(decision_positions)
        print(
            f"  {fmt_side(side)}: {len(decision_histories)} history cards -> "
            f"{len(decision_positions)} position cards; save {saving} ({percent:.2f}%)"
        )
    print()

    # A route pair is controllable by `side` under the recorded replies when
    # the opponent's move sequence is identical but that side's sequence is not.
    candidates: Dict[chess.Color, List[Tuple[int, History, History, RouteExposure, RouteExposure]]] = {
        chess.WHITE: [],
        chess.BLACK: [],
    }
    strict_outcome_dominance = Counter()
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
                delta = abs(left_exposure.catalog_count - right_exposure.catalog_count)
                candidates[side].append((delta, left, right, left_exposure, right_exposure))
                if (
                    left_exposure.catalog_outcomes < right_exposure.catalog_outcomes
                    or right_exposure.catalog_outcomes < left_exposure.catalog_outcomes
                ):
                    strict_outcome_dominance[side] += 1

    print("same-target route comparisons under identical recorded replies")
    for side in (chess.WHITE, chess.BLACK):
        with_catalog_difference = sum(
            left.catalog_count != right.catalog_count
            for _, _, _, left, right in candidates[side]
        )
        with_legal_difference = sum(
            left.legal_count != right.legal_count
            for _, _, _, left, right in candidates[side]
        )
        side_key = fmt_side(side).lower()
        actual[f"{side_key} controllable route pairs"] = len(candidates[side])
        actual[f"{side_key} pairs with catalog-count difference"] = with_catalog_difference
        actual[f"{side_key} pairs with legal-count difference"] = with_legal_difference
        actual[f"{side_key} strict catalog-outcome inclusions"] = strict_outcome_dominance[side]
        print(
            f"  {fmt_side(side)}: {len(candidates[side])} controllable pairs; "
            f"catalog-count differs in {with_catalog_difference}; "
            f"legal-count differs in {with_legal_difference}; "
            f"strict catalog-outcome inclusion in {strict_outcome_dominance[side]}"
        )
    print()

    print("largest curated branch-exposure contrasts")
    for side in (chess.WHITE, chess.BLACK):
        ranked = sorted(candidates[side], key=lambda item: (item[0], item[1], item[2]), reverse=True)
        print(f"  {fmt_side(side)}")
        emitted = 0
        for delta, left, right, left_exposure, right_exposure in ranked:
            if delta == 0:
                break
            print(
                f"    catalog {left_exposure.catalog_count} vs {right_exposure.catalog_count}; "
                f"legal {left_exposure.legal_count} vs {right_exposure.legal_count}"
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
    # share a position, and how many named endpoints remain below any of those
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

    print("top structural transposition hubs")
    ranked_hubs = sorted(
        hubs, reverse=True, key=lambda item: (item[0], item[1], item[2])
    )
    top_hub = ranked_hubs[0]
    actual["top hub score"] = top_hub[0]
    actual["top hub route multiplicity"] = top_hub[1]
    actual["top hub downstream endpoints"] = top_hub[2]
    for leverage, multiplicity, downstream_count, _, histories, downstream in ranked_hubs[:10]:
        representative = min(histories, key=lambda history: (len(history), history))
        names = sorted({terminal_names[h] for h in downstream})
        name_sample = "; ".join(names[:3]) + ("; ..." if len(names) > 3 else "")
        print(
            f"  score={leverage}, routes={multiplicity}, downstream named endpoints="
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


if __name__ == "__main__":
    main()
