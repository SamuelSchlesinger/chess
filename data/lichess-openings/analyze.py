#!/usr/bin/env python3
"""Reproduce the structural opening-graph report from the pinned TSV.

This program is deliberately read-only.  It replays ``all.tsv`` with
``chess==1.11.2``, constructs the distinct UCI prefix trie, projects every
history to a FIDE repetition key, and verifies every empirical count reported
in ``ANALYSIS.md``.  It does not estimate popularity, evaluation, or strength.

Install the exact dependency in an isolated environment, then run:

    python -m pip install chess==1.11.2
    python data/lichess-openings/analyze.py

The process exits nonzero on a version, input-hash, legality, endpoint, witness,
or aggregate mismatch.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import DefaultDict, Dict, Iterable, List, Mapping, Sequence, Set, Tuple

try:
    import chess
except ImportError as error:  # pragma: no cover - exercised only without dependency.
    raise SystemExit(
        "missing dependency: install the exact package with "
        "`python -m pip install chess==1.11.2`"
    ) from error


EXPECTED_CHESS_VERSION = "1.11.2"
EXPECTED_SHA256 = "fd710d16bf5cdd750a565ee1a6aba19eb2c7db7d74d7df961f6e00fb1cd04a6e"
EXPECTED_HEADER = ["eco", "name", "pgn", "uci", "epd"]

EXPECTED_COUNTS: Mapping[str, int] = {
    "named rows": 3_803,
    "plies across rows": 36_840,
    "row-prefix occurrences": 40_643,
    "distinct histories": 8_646,
    "prefix-trie edges": 8_645,
    "raw-en-passant keys": 7_921,
    "repetition nodes": 7_848,
    "transposition excess": 798,
    "non-singleton fibres": 570,
    "histories in non-singleton fibres": 1_368,
    "maximum node-fibre size": 8,
    "named endpoints with alternate route": 259,
    "depth-varying nodes": 3,
    "unique directed projected edges": 8_052,
    "multi-edge fibres": 430,
    "trie edges in multi-edge fibres": 1_023,
    "maximum edge-fibre size": 8,
    "self-loops": 0,
    "cyclic strongly connected components": 0,
    "row repetition-node revisits": 0,
    "histories with raw en-passant target": 1_418,
    "histories with legal en-passant capture": 27,
    "nodes combining raw-en-passant keys": 71,
    "distinct names": 3_174,
    "repeated-name groups": 293,
    "rows in repeated-name groups": 922,
    "maximum name multiplicity": 14,
    "repeated names spanning ECO codes": 108,
    "repeated names spanning ECO families": 6,
    "unique endpoint EPD fields": 3_803,
    "unique endpoint repetition keys": 3_803,
}

EXPECTED_NODE_MULTIPLICITIES = {
    2: 445,
    3: 69,
    4: 31,
    5: 15,
    6: 1,
    7: 6,
    8: 3,
}

EXPECTED_ENDPOINT_MULTIPLICITIES = {
    1: 3_544,
    2: 193,
    3: 34,
    4: 17,
    5: 7,
    6: 1,
    7: 4,
    8: 3,
}

EXPECTED_RAW_KEY_MULTIPLICITIES = {2: 69, 3: 2}
EXPECTED_DEPTH_SETS = [(15, 17), (16, 18), (17, 19)]

History = Tuple[str, ...]
Placement = Tuple[str, ...]
CastlingRights = Tuple[bool, bool, bool, bool]
BaseKey = Tuple[Placement, bool, CastlingRights]
PositionKey = Tuple[Placement, bool, CastlingRights, int | None]


class AnalysisError(RuntimeError):
    """A malformed or illegal corpus row prevented a meaningful analysis."""


def placement(board: chess.Board) -> Placement:
    """Canonical a1-through-h8 piece placement, matching Lean's key order."""

    return tuple(
        piece.symbol() if (piece := board.piece_at(square)) is not None else "."
        for square in chess.SQUARES
    )


def castling_rights(board: chess.Board) -> CastlingRights:
    """The four clean orthodox castling rights in Lean field order."""

    return (
        board.has_kingside_castling_rights(chess.WHITE),
        board.has_queenside_castling_rights(chess.WHITE),
        board.has_kingside_castling_rights(chess.BLACK),
        board.has_queenside_castling_rights(chess.BLACK),
    )


def base_key(board: chess.Board) -> BaseKey:
    return placement(board), board.turn, castling_rights(board)


def repetition_key(board: chess.Board) -> PositionKey:
    """Placement, turn, rights, and only legally effective en passant."""

    effective_ep = board.ep_square if board.has_legal_en_passant() else None
    return placement(board), board.turn, castling_rights(board), effective_ep


def raw_en_passant_key(board: chess.Board) -> PositionKey:
    return placement(board), board.turn, castling_rights(board), board.ep_square


def complete_fen(board: chess.Board) -> str:
    """All six FEN fields, retaining the nominal en-passant target."""

    return board.fen(en_passant="fen")


def parse_history(text: str) -> History:
    return tuple(text.split())


def format_distribution(distribution: Mapping[int, int]) -> str:
    return " ".join(f"{size}->{distribution[size]}" for size in sorted(distribution))


def tarjan_scc(vertex_count: int, edges: Iterable[Tuple[int, int]]) -> List[List[int]]:
    """Deterministic SCC decomposition over first-observation node numbers."""

    adjacency: List[Set[int]] = [set() for _ in range(vertex_count)]
    for source, target in edges:
        adjacency[source].add(target)

    next_index = 0
    indices = [-1] * vertex_count
    lowlinks = [0] * vertex_count
    stack: List[int] = []
    on_stack = [False] * vertex_count
    components: List[List[int]] = []

    def visit(vertex: int) -> None:
        nonlocal next_index
        indices[vertex] = next_index
        lowlinks[vertex] = next_index
        next_index += 1
        stack.append(vertex)
        on_stack[vertex] = True

        for target in sorted(adjacency[vertex]):
            if indices[target] == -1:
                visit(target)
                lowlinks[vertex] = min(lowlinks[vertex], lowlinks[target])
            elif on_stack[target]:
                lowlinks[vertex] = min(lowlinks[vertex], indices[target])

        if lowlinks[vertex] == indices[vertex]:
            component: List[int] = []
            while True:
                member = stack.pop()
                on_stack[member] = False
                component.append(member)
                if member == vertex:
                    break
            components.append(component)

    for vertex in range(vertex_count):
        if indices[vertex] == -1:
            visit(vertex)

    return components


def analyze(path: Path) -> Tuple[Dict[str, int], Dict[str, object], List[str]]:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != EXPECTED_SHA256:
        raise AnalysisError(
            f"unexpected SHA-256 for {path}: expected {EXPECTED_SHA256}, got {digest}"
        )

    with path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if reader.fieldnames != EXPECTED_HEADER:
            raise AnalysisError(
                f"unexpected header: expected {EXPECTED_HEADER!r}, got {reader.fieldnames!r}"
            )
        rows = list(reader)

    root = chess.Board()
    boards: Dict[History, chess.Board] = {(): root.copy(stack=False)}
    endpoint_keys: List[PositionKey] = []
    row_revisits = 0
    plies = 0
    prefix_occurrences = 0

    for row_index, row in enumerate(rows, start=2):
        board = chess.Board()
        history: History = ()
        seen_in_row = {repetition_key(board)}
        tokens = row["uci"].split()
        if not tokens:
            raise AnalysisError(f"{path}:{row_index}: empty UCI line")

        plies += len(tokens)
        prefix_occurrences += len(tokens) + 1
        for ply, token in enumerate(tokens, start=1):
            try:
                move = chess.Move.from_uci(token)
            except ValueError as error:
                raise AnalysisError(
                    f"{path}:{row_index}: ply {ply}: invalid UCI {token!r}: {error}"
                ) from error
            if move not in board.legal_moves:
                raise AnalysisError(
                    f"{path}:{row_index}: ply {ply}: illegal UCI {token!r} in "
                    f"{complete_fen(board)}"
                )
            board.push(move)
            history = history + (move.uci(),)

            previous = boards.get(history)
            if previous is None:
                boards[history] = board.copy(stack=False)
            elif complete_fen(previous) != complete_fen(board):
                raise AnalysisError(
                    f"determinism failure for canonical history {' '.join(history)!r}"
                )

            key = repetition_key(board)
            if key in seen_in_row:
                row_revisits += 1
            seen_in_row.add(key)

        actual_epd = board.epd(en_passant="legal")
        if actual_epd != row["epd"]:
            raise AnalysisError(
                f"{path}:{row_index}: endpoint mismatch: expected {row['epd']!r}, "
                f"got {actual_epd!r}"
            )
        endpoint_keys.append(repetition_key(board))

    fibres: DefaultDict[PositionKey, List[History]] = defaultdict(list)
    raw_keys_by_node: DefaultDict[PositionKey, Set[PositionKey]] = defaultdict(set)
    depths_by_node: DefaultDict[PositionKey, Set[int]] = defaultdict(set)
    raw_keys: Set[PositionKey] = set()
    raw_ep_histories = 0
    legal_ep_histories = 0

    for history, board in boards.items():
        key = repetition_key(board)
        raw_key = raw_en_passant_key(board)
        fibres[key].append(history)
        raw_keys_by_node[key].add(raw_key)
        depths_by_node[key].add(len(history))
        raw_keys.add(raw_key)
        raw_ep_histories += int(board.ep_square is not None)
        legal_ep_histories += int(board.has_legal_en_passant())

    node_multiplicities = Counter(len(histories) for histories in fibres.values())
    non_singleton = {
        key: histories for key, histories in fibres.items() if len(histories) > 1
    }
    depth_varying = {
        key: depths for key, depths in depths_by_node.items() if len(depths) > 1
    }
    raw_key_multiplicities = Counter(
        len(keys) for keys in raw_keys_by_node.values() if len(keys) > 1
    )

    node_ids = {key: node_id for node_id, key in enumerate(fibres)}
    edge_fibres: Counter[Tuple[int, int]] = Counter()
    labelled_edges: Set[Tuple[int, str, int]] = set()
    for history, board in boards.items():
        if not history:
            continue
        parent = history[:-1]
        source_id = node_ids[repetition_key(boards[parent])]
        target_id = node_ids[repetition_key(board)]
        edge_fibres[(source_id, target_id)] += 1
        labelled_edges.add((source_id, history[-1], target_id))

    multi_edge_fibres = [count for count in edge_fibres.values() if count > 1]
    self_loops = sum(source == target for source, target in edge_fibres)
    components = tarjan_scc(len(fibres), edge_fibres)
    cyclic_components = sum(len(component) > 1 for component in components)

    endpoint_multiplicities = Counter(len(fibres[key]) for key in endpoint_keys)
    alternate_endpoints = sum(
        count for size, count in endpoint_multiplicities.items() if size > 1
    )

    name_groups: DefaultDict[str, List[Mapping[str, str]]] = defaultdict(list)
    for row in rows:
        name_groups[row["name"]].append(row)
    repeated_names = {
        name: group for name, group in name_groups.items() if len(group) > 1
    }
    multi_eco_names = sum(
        len({row["eco"] for row in group}) > 1 for group in repeated_names.values()
    )
    multi_family_names = sum(
        len({row["eco"][0] for row in group}) > 1
        for group in repeated_names.values()
    )

    counts: Dict[str, int] = {
        "named rows": len(rows),
        "plies across rows": plies,
        "row-prefix occurrences": prefix_occurrences,
        "distinct histories": len(boards),
        "prefix-trie edges": len(boards) - 1,
        "raw-en-passant keys": len(raw_keys),
        "repetition nodes": len(fibres),
        "transposition excess": len(boards) - len(fibres),
        "non-singleton fibres": len(non_singleton),
        "histories in non-singleton fibres": sum(map(len, non_singleton.values())),
        "maximum node-fibre size": max(node_multiplicities),
        "named endpoints with alternate route": alternate_endpoints,
        "depth-varying nodes": len(depth_varying),
        "unique directed projected edges": len(edge_fibres),
        "multi-edge fibres": len(multi_edge_fibres),
        "trie edges in multi-edge fibres": sum(multi_edge_fibres),
        "maximum edge-fibre size": max(multi_edge_fibres),
        "self-loops": self_loops,
        "cyclic strongly connected components": cyclic_components,
        "row repetition-node revisits": row_revisits,
        "histories with raw en-passant target": raw_ep_histories,
        "histories with legal en-passant capture": legal_ep_histories,
        "nodes combining raw-en-passant keys": sum(raw_key_multiplicities.values()),
        "distinct names": len(name_groups),
        "repeated-name groups": len(repeated_names),
        "rows in repeated-name groups": sum(map(len, repeated_names.values())),
        "maximum name multiplicity": max(map(len, repeated_names.values())),
        "repeated names spanning ECO codes": multi_eco_names,
        "repeated names spanning ECO families": multi_family_names,
        "unique endpoint EPD fields": len({row["epd"] for row in rows}),
        "unique endpoint repetition keys": len(set(endpoint_keys)),
    }

    details: Dict[str, object] = {
        "node multiplicities": {
            size: count for size, count in node_multiplicities.items() if size > 1
        },
        "endpoint multiplicities": dict(endpoint_multiplicities),
        "raw-key multiplicities": dict(raw_key_multiplicities),
        "depth sets": sorted(tuple(sorted(depths)) for depths in depth_varying.values()),
        "strongly connected components": len(components),
        "maximum SCC size": max(map(len, components)),
        "labelled projected edges": len(labelled_edges),
        "input SHA-256": digest,
    }

    witness_failures: List[str] = []

    def require_witness(description: str, condition: bool) -> None:
        if not condition:
            witness_failures.append(description)

    def board_for(text: str) -> chess.Board:
        history = parse_history(text)
        board = boards.get(history)
        if board is None:
            witness_failures.append(f"missing corpus history: {text}")
            return chess.Board()
        return board

    semi_slav = board_for(
        "d2d4 d7d5 c2c4 c7c6 b1c3 g8f6 e2e3 e7e6 g1f3"
    )
    require_witness(
        "Semi-Slav witness does not have node-fibre size eight",
        len(fibres[repetition_key(semi_slav)]) == 8,
    )

    italian_a = board_for(
        "e2e4 e7e5 g1f3 b8c6 f1c4 f8c5 e1g1 g8f6"
    )
    italian_b = board_for(
        "e2e4 e7e5 g1f3 b8c6 f1c4 g8f6 e1g1 f8c5"
    )
    require_witness(
        "ordinary move-order diamond endpoints differ",
        repetition_key(italian_a) == repetition_key(italian_b),
    )

    caro_nc3_text = "e2e4 c7c6 d2d4 d7d5 b1c3 d5e4 c3e4 b8d7"
    caro_nd2_text = "e2e4 c7c6 d2d4 d7d5 b1d2 d5e4 d2e4 b8d7"
    caro_nc3 = board_for(caro_nc3_text)
    caro_nd2 = board_for(caro_nd2_text)
    require_witness(
        "Caro-Kann coalescence does not reach one complete position",
        complete_fen(caro_nc3) == complete_fen(caro_nd2),
    )
    require_witness(
        "Caro-Kann coalescence histories unexpectedly are move permutations",
        Counter(parse_history(caro_nc3_text)) != Counter(parse_history(caro_nd2_text)),
    )

    catalan_direct_text = (
        "d2d4 g8f6 c2c4 e7e6 g1f3 d7d5 g2g3 f8e7 f1g2 e8g8 "
        "e1g1 b8d7 d1c2 c7c6 c1f4"
    )
    catalan_detour_text = (
        "d2d4 g8f6 c2c4 e7e6 g1f3 d7d5 g2g3 f8b4 c1d2 b4e7 "
        "f1g2 e8g8 e1g1 c7c6 d1c2 b8d7 d2f4"
    )
    catalan_direct = board_for(catalan_direct_text)
    catalan_detour = board_for(catalan_detour_text)
    require_witness(
        "Catalan unequal-depth histories do not transpose",
        repetition_key(catalan_direct) == repetition_key(catalan_detour),
    )
    require_witness(
        "Catalan witness depths are not 15 and 17",
        (len(parse_history(catalan_direct_text)), len(parse_history(catalan_detour_text)))
        == (15, 17),
    )

    ineffective_a = board_for("c2c4 e7e5 e2e3")
    ineffective_b = board_for("e2e3 e7e5 c2c4")
    require_witness(
        "ineffective-en-passant histories do not merge",
        repetition_key(ineffective_a) == repetition_key(ineffective_b),
    )
    require_witness(
        "ineffective-en-passant histories do not have distinct raw keys",
        raw_en_passant_key(ineffective_a) != raw_en_passant_key(ineffective_b),
    )

    effective_a = board_for("b1c3 e7e5 f2f4 e5f4 e2e4")
    effective_b = board_for("e2e4 e7e5 f2f4 e5f4 b1c3")
    require_witness(
        "effective-en-passant histories do not agree on base fields",
        base_key(effective_a) == base_key(effective_b),
    )
    require_witness(
        "effective-en-passant histories were incorrectly merged",
        repetition_key(effective_a) != repetition_key(effective_b),
    )
    require_witness(
        "effective-en-passant witness does not have exactly one legal EP side",
        effective_a.has_legal_en_passant() and not effective_b.has_legal_en_passant(),
    )

    return counts, details, witness_failures


def report_mismatches(
    counts: Mapping[str, int], details: Mapping[str, object], witness_failures: Sequence[str]
) -> List[str]:
    failures = [
        f"{label}: expected {expected}, got {counts.get(label)!r}"
        for label, expected in EXPECTED_COUNTS.items()
        if counts.get(label) != expected
    ]

    expected_details = {
        "node multiplicities": EXPECTED_NODE_MULTIPLICITIES,
        "endpoint multiplicities": EXPECTED_ENDPOINT_MULTIPLICITIES,
        "raw-key multiplicities": EXPECTED_RAW_KEY_MULTIPLICITIES,
        "depth sets": EXPECTED_DEPTH_SETS,
        "strongly connected components": EXPECTED_COUNTS["repetition nodes"],
        "maximum SCC size": 1,
        "labelled projected edges": EXPECTED_COUNTS["unique directed projected edges"],
    }
    failures.extend(
        f"{label}: expected {expected!r}, got {details.get(label)!r}"
        for label, expected in expected_details.items()
        if details.get(label) != expected
    )
    failures.extend(f"witness: {failure}" for failure in witness_failures)
    return failures


def print_report(counts: Mapping[str, int], details: Mapping[str, object]) -> None:
    print(f"python-chess: {chess.__version__}")
    print(f"input SHA-256: {details['input SHA-256']}")
    print()
    print("prefix corpus")
    for label in (
        "named rows",
        "plies across rows",
        "row-prefix occurrences",
        "distinct histories",
        "prefix-trie edges",
        "raw-en-passant keys",
        "repetition nodes",
    ):
        print(f"  {label}: {counts[label]}")
    print()
    print("node fibres")
    for label in (
        "transposition excess",
        "non-singleton fibres",
        "histories in non-singleton fibres",
        "maximum node-fibre size",
        "named endpoints with alternate route",
        "depth-varying nodes",
    ):
        print(f"  {label}: {counts[label]}")
    print(f"  multiplicities: {format_distribution(details['node multiplicities'])}")
    print(f"  endpoint multiplicities: {format_distribution(details['endpoint multiplicities'])}")
    print(f"  depth sets: {details['depth sets']}")
    print()
    print("projected graph")
    for label in (
        "unique directed projected edges",
        "multi-edge fibres",
        "trie edges in multi-edge fibres",
        "maximum edge-fibre size",
        "self-loops",
        "cyclic strongly connected components",
        "row repetition-node revisits",
    ):
        print(f"  {label}: {counts[label]}")
    print(f"  SCCs: {details['strongly connected components']}")
    print(f"  maximum SCC size: {details['maximum SCC size']}")
    print()
    print("en passant")
    for label in (
        "histories with raw en-passant target",
        "histories with legal en-passant capture",
        "nodes combining raw-en-passant keys",
    ):
        print(f"  {label}: {counts[label]}")
    print(f"  raw-key multiplicities: {format_distribution(details['raw-key multiplicities'])}")
    print()
    print("names and endpoints")
    for label in (
        "distinct names",
        "repeated-name groups",
        "rows in repeated-name groups",
        "maximum name multiplicity",
        "repeated names spanning ECO codes",
        "repeated names spanning ECO families",
        "unique endpoint EPD fields",
        "unique endpoint repetition keys",
    ):
        print(f"  {label}: {counts[label]}")


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input",
        type=Path,
        default=Path(__file__).with_name("all.tsv"),
        help="pinned all.tsv path (default: next to this script)",
    )
    args = parser.parse_args(argv)

    if chess.__version__ != EXPECTED_CHESS_VERSION:
        print(
            f"FAIL: expected chess=={EXPECTED_CHESS_VERSION}, got chess=={chess.__version__}",
            file=sys.stderr,
        )
        return 2

    try:
        counts, details, witness_failures = analyze(args.input)
    except (AnalysisError, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1

    print_report(counts, details)
    failures = report_mismatches(counts, details, witness_failures)
    if failures:
        print(file=sys.stderr)
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1

    print()
    print("all ANALYSIS.md counts and concrete witnesses match")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
