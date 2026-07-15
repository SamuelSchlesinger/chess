#!/usr/bin/env python3
"""Classify exact opening transpositions by elementary algebraic mechanism.

By default this read-only pilot validates quantitative claims and the committed
rooted-path certificate in the parent research document.  It replays the
repository's pinned Lichess opening-name corpus and compares three equivalence
relations on its distinct legal histories:

1. exact FIDE repetition endpoint equality;
2. equality of ``(length, multiset of UCI plies)``, a necessary condition for
   any trace theory generated only by permuting atomic plies;
3. closure under *catalog-visible alternating braids* ``abc <-> cba``, where
   both histories occur in the corpus and the two three-ply segments have the
   same exact repetition endpoint.

Relation (3) is deliberately an under-approximation: a braid path whose
intermediate history is legal but absent from this name catalog is not counted.
That makes a low coverage result a reason to run a broader pilot, not evidence
that the legal chess relation itself is rare.

Dependency and invocation (from the repository root):

    uv run --with chess==1.11.2 python \
      research/novel-chess-theory/transposition-algebra/data/classify_transpositions.py

The script pins both the python-chess version and the input SHA-256 and exits
nonzero if either, any checked aggregate, or either generated artifact changes.
Pass ``--write-artifacts`` only when intentionally refreshing
``rooted_path_basis.json`` and ``classify_transpositions.output.txt``.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import heapq
import json
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, DefaultDict, Dict, Iterable, List, Mapping, Set, Tuple

try:
    import chess
except ImportError as error:  # pragma: no cover - dependency failure path.
    raise SystemExit(
        "missing dependency: run with `uv run --with chess==1.11.2 python ...`"
    ) from error


EXPECTED_CHESS_VERSION = "1.11.2"
EXPECTED_SHA256 = "fd710d16bf5cdd750a565ee1a6aba19eb2c7db7d74d7df961f6e00fb1cd04a6e"

History = Tuple[str, ...]
PositionKey = Tuple[str, bool, str, int | None]
TraceSignature = Tuple[int, Tuple[Tuple[str, int], ...]]
ProjectedEdge = Tuple[PositionKey, PositionKey]


def repository_root() -> Path:
    return Path(__file__).resolve().parents[4]


def repetition_key(board: chess.Board) -> PositionKey:
    """FIDE repetition equality: placement, turn, rights, effective e.p."""

    effective_ep = board.ep_square if board.has_legal_en_passant() else None
    return (
        board.board_fen(),
        board.turn,
        board.castling_xfen(),
        effective_ep,
    )


def trace_signature(history: History) -> TraceSignature:
    """Invariant of every equivalence generated solely by letter swaps."""

    return len(history), tuple(sorted(Counter(history).items()))


def trace_signature_record(history: History) -> Dict[str, Any]:
    """Stable JSON rendering of the necessary permutation signature."""

    length, multiset = trace_signature(history)
    return {
        "length": length,
        "uci_multiset": [
            {"move": move, "count": multiplicity}
            for move, multiplicity in multiset
        ],
    }


def key_record(key: PositionKey) -> Dict[str, str]:
    """Stable, exact JSON rendering of a repetition key."""

    placement, turn, castling, effective_ep = key
    return {
        "placement": placement,
        "turn": "white" if turn else "black",
        "castling": castling,
        "effective_en_passant": (
            chess.square_name(effective_ep) if effective_ep is not None else "-"
        ),
    }


def history_record(history: History) -> List[str]:
    """JSON uses an array so UCI tokens remain unambiguous."""

    return list(history)


def load_histories(path: Path) -> Dict[History, chess.Board]:
    if chess.__version__ != EXPECTED_CHESS_VERSION:
        raise RuntimeError(
            f"expected chess=={EXPECTED_CHESS_VERSION}, got {chess.__version__}"
        )
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != EXPECTED_SHA256:
        raise RuntimeError(
            f"unexpected corpus SHA-256: expected {EXPECTED_SHA256}, got {digest}"
        )

    boards: Dict[History, chess.Board] = {(): chess.Board()}
    with path.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if reader.fieldnames != ["eco", "name", "pgn", "uci", "epd"]:
            raise RuntimeError(f"unexpected TSV header: {reader.fieldnames!r}")
        for line_number, row in enumerate(reader, start=2):
            board = chess.Board()
            history: History = ()
            for ply, token in enumerate(row["uci"].split(), start=1):
                move = chess.Move.from_uci(token)
                if move not in board.legal_moves:
                    raise RuntimeError(
                        f"{path}:{line_number}: illegal ply {ply}: {token}"
                    )
                board.push(move)
                history += (move.uci(),)
                previous = boards.setdefault(history, board.copy(stack=False))
                if previous.fen(en_passant="fen") != board.fen(en_passant="fen"):
                    raise RuntimeError(f"nondeterministic history: {' '.join(history)}")
    return boards


class UnionFind:
    def __init__(self, elements: Iterable[History]) -> None:
        self.parent = {element: element for element in elements}

    def find(self, element: History) -> History:
        root = element
        while self.parent[root] != root:
            root = self.parent[root]
        while self.parent[element] != element:
            following = self.parent[element]
            self.parent[element] = root
            element = following
        return root

    def union(self, left: History, right: History) -> None:
        left_root = self.find(left)
        right_root = self.find(right)
        if left_root != right_root:
            self.parent[right_root] = left_root


def direct_catalog_braids(
    boards: Mapping[History, chess.Board],
) -> Set[Tuple[History, History]]:
    """Find contextual ``abc <-> cba`` applications on complete histories."""

    histories = set(boards)
    edges: Set[Tuple[History, History]] = set()
    for history in histories:
        if len(history) < 3:
            continue
        for offset in range(len(history) - 2):
            candidate = (
                history[:offset]
                + (history[offset + 2], history[offset + 1], history[offset])
                + history[offset + 3 :]
            )
            if candidate not in histories or candidate == history:
                continue
            left_prefix = history[: offset + 3]
            right_prefix = candidate[: offset + 3]
            if repetition_key(boards[left_prefix]) != repetition_key(boards[right_prefix]):
                continue
            edge = (history, candidate) if history < candidate else (candidate, history)
            edges.add(edge)
    return edges


def local_braid_prefixes(left: History, right: History) -> Tuple[History, History]:
    """Recover the sound local prefix equation underlying a contextual braid."""

    if len(left) != len(right):
        raise RuntimeError("contextual braid histories have different lengths")
    for offset in range(len(left) - 2):
        candidate = (
            left[:offset]
            + (left[offset + 2], left[offset + 1], left[offset])
            + left[offset + 3 :]
        )
        if candidate != right or candidate == left:
            continue
        left_prefix = left[: offset + 3]
        right_prefix = right[: offset + 3]
        if repetition_key(replay(left_prefix)) != repetition_key(replay(right_prefix)):
            raise RuntimeError("contextual braid has an unsound local prefix")
        return (
            (left_prefix, right_prefix)
            if left_prefix < right_prefix
            else (right_prefix, left_prefix)
        )
    raise RuntimeError("history pair is not one alternating braid")


def replay(history: History) -> chess.Board:
    board = chess.Board()
    for token in history:
        move = chess.Move.from_uci(token)
        if move not in board.legal_moves:
            raise RuntimeError(f"illegal recombined history: {' '.join(history)}")
        board.push(move)
    return board


def replay_key_path(history: History) -> Tuple[chess.Board, List[PositionKey]]:
    """Replay a history and retain every exact projected vertex on the path."""

    board = chess.Board()
    keys = [repetition_key(board)]
    for token in history:
        move = chess.Move.from_uci(token)
        if move not in board.legal_moves:
            raise RuntimeError(f"illegal recombined history: {' '.join(history)}")
        board.push(move)
        keys.append(repetition_key(board))
    return board, keys


def is_direct_alternating_braid(left: History, right: History) -> bool:
    """Whether two root histories differ by one sound ``abc <-> cba``."""

    if len(left) != len(right):
        return False
    for offset in range(len(left) - 2):
        candidate = (
            left[:offset]
            + (left[offset + 2], left[offset + 1], left[offset])
            + left[offset + 3 :]
        )
        if candidate != right or candidate == left:
            continue
        left_prefix = replay(left[: offset + 3])
        right_prefix = replay(right[: offset + 3])
        return repetition_key(left_prefix) == repetition_key(right_prefix)
    return False


def analyze(
    boards: Mapping[History, chess.Board],
) -> Tuple[Dict[str, int], Dict[str, Any]]:
    fibres: DefaultDict[PositionKey, List[History]] = defaultdict(list)
    for history, board in boards.items():
        fibres[repetition_key(board)].append(history)

    non_singleton = [histories for histories in fibres.values() if len(histories) > 1]
    endpoint_excess = sum(len(histories) - 1 for histories in non_singleton)

    # A study decision can occur only at an observed prefix with at least one
    # recorded continuation and on the learner's side to move.  These remain
    # corpus-relative decision nodes, not instantiated scheduler cards.
    decision_histories = {history[:-1] for history in boards if history}
    side_decision_counts: Dict[bool, Tuple[int, int]] = {}
    for side in (chess.WHITE, chess.BLACK):
        side_histories = [
            history
            for history in decision_histories
            if boards[history].turn == side
        ]
        side_positions = {repetition_key(boards[history]) for history in side_histories}
        side_decision_counts[side] = (len(side_histories), len(side_positions))

    single_signature_fibres = 0
    mixed_signature_fibres = 0
    permutation_excess = 0
    beyond_permutation_excess = 0
    depth_varying_fibres = 0
    for histories in non_singleton:
        signature_counts = Counter(trace_signature(history) for history in histories)
        if len(signature_counts) == 1:
            single_signature_fibres += 1
        else:
            mixed_signature_fibres += 1
        permutation_excess += sum(count - 1 for count in signature_counts.values())
        beyond_permutation_excess += len(signature_counts) - 1
        depth_varying_fibres += int(len({len(history) for history in histories}) > 1)

    braid_edges = direct_catalog_braids(boards)
    local_braid_contexts: DefaultDict[
        Tuple[History, History], Set[Tuple[History, History]]
    ] = defaultdict(set)
    for left, right in braid_edges:
        local_braid_contexts[local_braid_prefixes(left, right)].add((left, right))
    union_find = UnionFind(boards)
    for left, right in braid_edges:
        union_find.union(left, right)
    braid_excess = 0
    braid_nontrivial_fibres = 0
    braid_components = 0
    for histories in non_singleton:
        components = {union_find.find(history) for history in histories}
        collapsed = len(histories) - len(components)
        braid_excess += collapsed
        braid_nontrivial_fibres += int(collapsed > 0)
        braid_components += len(components)

    # The projected corpus graph has repetition keys as vertices and one
    # directed edge for each distinct source/target pair induced by a trie
    # edge.  Since the source key fixes the board and UCI is deterministic,
    # retaining the label is useful for constructing a readable arborescence.
    projected_edges: Dict[ProjectedEdge, str] = {}
    for history, board in boards.items():
        if not history:
            continue
        source = repetition_key(boards[history[:-1]])
        target = repetition_key(board)
        edge = (source, target)
        previous = projected_edges.setdefault(edge, history[-1])
        if previous != history[-1]:
            raise RuntimeError(
                "two UCI labels induce the same projected source/target edge: "
                f"{previous} and {history[-1]}"
            )

    indegrees = Counter(target for _source, target in projected_edges)
    indegree_distribution = Counter(
        indegree for node, indegree in indegrees.items() if indegree > 1
    )
    merge_nodes = sum(indegree_distribution.values())
    chord_count = sum(
        indegree - 1 for node, indegree in indegrees.items() if indegree > 1
    )

    # Choose a genuinely shortest, then lexicographically least root path in
    # the *projected graph*.  This may recombine catalog edges into a legal
    # history that was not itself a row prefix, so selecting only among
    # observed histories would not be sufficient.  A unit-weight Dijkstra
    # search with the full UCI word as tie-breaker constructs the rooted
    # arborescence and its canonical paths together.
    root_key = repetition_key(boards[()])
    adjacency: DefaultDict[PositionKey, List[Tuple[str, PositionKey]]] = defaultdict(list)
    for (source, target), label in projected_edges.items():
        adjacency[source].append((label, target))
    for edges in adjacency.values():
        edges.sort(key=lambda item: item[0])

    canonical: Dict[PositionKey, History] = {root_key: ()}
    parent_edge: Dict[PositionKey, Tuple[PositionKey, PositionKey]] = {}
    serial = 0
    queue: List[Tuple[int, History, int, PositionKey]] = [(0, (), serial, root_key)]
    while queue:
        _depth, path, _serial, source = heapq.heappop(queue)
        if canonical.get(source) != path:
            continue
        for label, target in adjacency[source]:
            candidate = path + (label,)
            previous = canonical.get(target)
            if previous is not None and (len(previous), previous) <= (
                len(candidate),
                candidate,
            ):
                continue
            canonical[target] = candidate
            parent_edge[target] = (source, target)
            serial += 1
            heapq.heappush(queue, (len(candidate), candidate, serial, target))
    tree_edges = set(parent_edge.values())

    if len(canonical) != len(fibres):
        raise RuntimeError("not every projected vertex is reachable from the root")
    if len(tree_edges) != len(fibres) - 1:
        raise RuntimeError("canonical paths did not induce a rooted arborescence")
    if indegrees[root_key] != 0:
        raise RuntimeError("the projected root unexpectedly has an incoming edge")

    # Recheck every recombined canonical path rather than relying only on the
    # abstract fact that repetition-equivalent positions have equal legal
    # futures.  This is the executable bridge from the projected graph to the
    # concrete python-chess replay used by the certificate.
    for expected_key, path in canonical.items():
        board, keys = replay_key_path(path)
        if repetition_key(board) != expected_key:
            raise RuntimeError(
                "canonical path reaches the wrong key: " + " ".join(path)
            )
        for index, token in enumerate(path):
            edge = (keys[index], keys[index + 1])
            if projected_edges.get(edge) != token:
                raise RuntimeError(
                    "canonical path uses an absent projected edge: "
                    + " ".join(path[: index + 1])
                )

    chord_tail_tokens: List[int] = []
    chord_max_tail: List[int] = []
    chord_trace_compatible = 0
    chord_depth_changing = 0
    chord_direct_braids = 0
    chord_direct_and_short = 0
    chord_nondirect_and_short = 0
    chord_nondirect_four_or_five = 0
    chord_longer = 0
    relation_records: List[Dict[str, Any]] = []
    chord_items = [
        (edge, move)
        for edge, move in projected_edges.items()
        if edge not in tree_edges
    ]
    chord_items.sort(
        key=lambda item: (
            canonical[item[0][0]] + (item[1],),
            canonical[item[0][1]],
            item[0],
        )
    )
    for relation_number, ((source, target), move) in enumerate(chord_items, start=1):
        left = canonical[source] + (move,)
        right = canonical[target]
        left_board, left_keys = replay_key_path(left)
        right_board, right_keys = replay_key_path(right)
        if repetition_key(left_board) != target or repetition_key(right_board) != target:
            raise RuntimeError(
                "chord certificate has unequal endpoints: "
                f"{' '.join(left)} ~= {' '.join(right)}"
            )

        # The mod-2 boundary of a fundamental chord equation contains exactly
        # its own non-tree edge.  This gives an executable triangular
        # independence certificate for all chord boundaries.
        boundary: Counter[ProjectedEdge] = Counter()
        for path_keys in (left_keys, right_keys):
            for edge in zip(path_keys, path_keys[1:]):
                boundary[edge] += 1
        odd_non_tree_edges = {
            edge
            for edge, multiplicity in boundary.items()
            if multiplicity % 2 == 1 and edge not in tree_edges
        }
        if odd_non_tree_edges != {(source, target)}:
            raise RuntimeError("relation is not the expected fundamental cycle")

        common = 0
        while common < min(len(left), len(right)) and left[common] == right[common]:
            common += 1
        left_tail = left[common:]
        right_tail = right[common:]
        tail_tokens = len(left_tail) + len(right_tail)
        max_tail = max(len(left_tail), len(right_tail))
        same_length = len(left) == len(right)
        same_multiset = Counter(left) == Counter(right)
        signature_compatible = same_length and same_multiset
        direct_braid = is_direct_alternating_braid(left, right)
        if signature_compatible:
            syntactic_class = "signature-compatible-unclassified"
        elif same_length:
            syntactic_class = "same-length-different-uci-multiset"
        else:
            syntactic_class = "unequal-length"

        chord_tail_tokens.append(tail_tokens)
        chord_max_tail.append(max_tail)
        chord_trace_compatible += int(signature_compatible)
        chord_depth_changing += int(not same_length)
        chord_direct_braids += int(direct_braid)
        chord_direct_and_short += int(direct_braid and max_tail <= 3)
        chord_nondirect_and_short += int(not direct_braid and max_tail <= 3)
        chord_nondirect_four_or_five += int(
            not direct_braid and 4 <= max_tail <= 5
        )
        chord_longer += int(max_tail > 5)
        relation_records.append(
            {
                "id": f"R{relation_number:03d}",
                "edge": {
                    "source": key_record(source),
                    "move": move,
                    "target": key_record(target),
                },
                "left": history_record(left),
                "right": history_record(right),
                "common_prefix_plies": common,
                "left_tail": history_record(left_tail),
                "right_tail": history_record(right_tail),
                "left_trace_signature": trace_signature_record(left),
                "right_trace_signature": trace_signature_record(right),
                "same_length": same_length,
                "same_uci_multiset": same_multiset,
                "signature_compatible": signature_compatible,
                "direct_alternating_braid": direct_braid,
                "syntactic_class": syntactic_class,
                "fundamental_cycle_non_tree_edge_count": len(odd_non_tree_edges),
            }
        )

    if len(chord_tail_tokens) != chord_count:
        raise RuntimeError("chord count disagrees with indegree excess")
    if chord_count != len(projected_edges) - len(tree_edges):
        raise RuntimeError("indegree excess disagrees with graph cycle rank")

    counts = {
        "distinct histories": len(boards),
        "repetition nodes": len(fibres),
        "non-singleton endpoint fibres": len(non_singleton),
        "endpoint excess": endpoint_excess,
        "white history decision nodes": side_decision_counts[chess.WHITE][0],
        "white repetition-key decision nodes": side_decision_counts[chess.WHITE][1],
        "black history decision nodes": side_decision_counts[chess.BLACK][0],
        "black repetition-key decision nodes": side_decision_counts[chess.BLACK][1],
        "single-signature fibres": single_signature_fibres,
        "mixed-signature fibres": mixed_signature_fibres,
        "depth-varying fibres": depth_varying_fibres,
        "maximum excess explainable by ply permutations": permutation_excess,
        "excess requiring more than ply permutations": beyond_permutation_excess,
        "catalog-visible contextual braid applications": len(braid_edges),
        "distinct local catalog-visible braid relations": len(local_braid_contexts),
        "suffix-context applications beyond local braid relations": (
            len(braid_edges) - len(local_braid_contexts)
        ),
        "maximum contexts for one local braid relation": max(
            len(contexts) for contexts in local_braid_contexts.values()
        ),
        "fibres touched by catalog-visible braids": braid_nontrivial_fibres,
        "excess collapsed by catalog-visible braid closure": braid_excess,
        "braid components across non-singleton fibres": braid_components,
        "projected directed edges": len(projected_edges),
        "primitive merge nodes": merge_nodes,
        "indegree-excess chords": chord_count,
        "merge nodes of indegree 2": indegree_distribution[2],
        "merge nodes of indegree 3": indegree_distribution[3],
        "merge nodes of indegree 4": indegree_distribution[4],
        "merge nodes of indegree 5": indegree_distribution[5],
        "merge nodes of indegree 6": indegree_distribution[6],
        "shortest-basis signature-compatible chords": chord_trace_compatible,
        "shortest-basis depth-changing chords": chord_depth_changing,
        "shortest-basis direct alternating braids": chord_direct_braids,
        "shortest-basis relations with at most 3 plies per side": sum(
            length <= 3 for length in chord_max_tail
        ),
        "shortest-basis relations with at most 5 plies per side": sum(
            length <= 5 for length in chord_max_tail
        ),
        "curriculum direct braids at most 3 plies per side": chord_direct_and_short,
        "curriculum other relations at most 3 plies per side": (
            chord_nondirect_and_short
        ),
        "curriculum other relations with 4 or 5 plies per side": (
            chord_nondirect_four_or_five
        ),
        "curriculum relations longer than 5 plies per side": chord_longer,
        "shortest-basis total tail tokens": sum(chord_tail_tokens),
        "shortest-basis maximum one-side tail": max(chord_max_tail),
        "shortest-basis maximum two-side tail tokens": max(chord_tail_tokens),
    }

    local_braid_records = []
    for braid_number, ((left, right), contexts) in enumerate(
        sorted(local_braid_contexts.items()), start=1
    ):
        local_braid_records.append(
            {
                "id": f"B{braid_number:03d}",
                "left_prefix": history_record(left),
                "right_prefix": history_record(right),
                "contextual_application_count": len(contexts),
                "contexts": [
                    {
                        "left": history_record(context_left),
                        "right": history_record(context_right),
                    }
                    for context_left, context_right in sorted(contexts)
                ],
            }
        )

    certificate: Dict[str, Any] = {
        "schema_version": 1,
        "scope": (
            "Concrete endpoint-sound equations for all root-originating paths "
            "of the fixed edge-retaining projected graph"
        ),
        "python_chess_version": EXPECTED_CHESS_VERSION,
        "input_sha256": EXPECTED_SHA256,
        "graph": {
            "root": key_record(root_key),
            "vertices": len(fibres),
            "directed_edges": len(projected_edges),
            "tree_edges": len(tree_edges),
            "non_tree_edges": len(relation_records),
            "binary_cycle_rank": len(projected_edges) - len(fibres) + 1,
        },
        "corpus_relative_decision_nodes": {
            "white": {
                "history_prefixes": side_decision_counts[chess.WHITE][0],
                "repetition_keys": side_decision_counts[chess.WHITE][1],
            },
            "black": {
                "history_prefixes": side_decision_counts[chess.BLACK][0],
                "repetition_keys": side_decision_counts[chess.BLACK][1],
            },
        },
        "assurance": {
            "all_canonical_paths_replayed": True,
            "all_relation_sides_replayed": True,
            "all_relation_endpoints_equal": True,
            "each_boundary_has_exactly_its_own_non_tree_edge": True,
        },
        "rooted_path_relations": relation_records,
        "local_catalog_visible_braids": local_braid_records,
    }
    return counts, certificate


# Independently inspected on 2026-07-14.  Keeping these values in the
# executable turns subsequent runs into regression checks rather than a report
# that silently changes with code or data.
EXPECTED_COUNTS: Mapping[str, int] = {  # populated below after graph-shortest audit
    "distinct histories": 8_646,
    "repetition nodes": 7_848,
    "non-singleton endpoint fibres": 570,
    "endpoint excess": 798,
    "white history decision nodes": 3_091,
    "white repetition-key decision nodes": 2_741,
    "black history decision nodes": 3_102,
    "black repetition-key decision nodes": 2_736,
    "single-signature fibres": 557,
    "mixed-signature fibres": 13,
    "depth-varying fibres": 3,
    "maximum excess explainable by ply permutations": 785,
    "excess requiring more than ply permutations": 13,
    "catalog-visible contextual braid applications": 296,
    "distinct local catalog-visible braid relations": 87,
    "suffix-context applications beyond local braid relations": 209,
    "maximum contexts for one local braid relation": 16,
    "fibres touched by catalog-visible braids": 241,
    "excess collapsed by catalog-visible braid closure": 296,
    "braid components across non-singleton fibres": 1_072,
    "projected directed edges": 8_052,
    "primitive merge nodes": 193,
    "indegree-excess chords": 205,
    "merge nodes of indegree 2": 181,
    "merge nodes of indegree 3": 12,
    "merge nodes of indegree 4": 0,
    "merge nodes of indegree 5": 0,
    "merge nodes of indegree 6": 0,
    "shortest-basis signature-compatible chords": 201,
    "shortest-basis depth-changing chords": 1,
    "shortest-basis direct alternating braids": 84,
    "shortest-basis relations with at most 3 plies per side": 86,
    "shortest-basis relations with at most 5 plies per side": 138,
    "curriculum direct braids at most 3 plies per side": 84,
    "curriculum other relations at most 3 plies per side": 2,
    "curriculum other relations with 4 or 5 plies per side": 52,
    "curriculum relations longer than 5 plies per side": 67,
    "shortest-basis total tail tokens": 2_112,
    "shortest-basis maximum one-side tail": 17,
    "shortest-basis maximum two-side tail tokens": 34,
}


def render_output(counts: Mapping[str, int], certificate_sha256: str) -> bytes:
    lines = [
        f"python-chess: {EXPECTED_CHESS_VERSION}",
        f"input SHA-256: {EXPECTED_SHA256}",
        f"rooted-path certificate SHA-256: {certificate_sha256}",
        "",
    ]
    lines.extend(f"{label}: {count}" for label, count in counts.items())
    return ("\n".join(lines) + "\n").encode("utf-8")


def main(argv: List[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write-artifacts",
        action="store_true",
        help=(
            "refresh rooted_path_basis.json and "
            "classify_transpositions.output.txt after all checks pass"
        ),
    )
    arguments = parser.parse_args(argv)
    corpus = repository_root() / "data" / "lichess-openings" / "all.tsv"
    artifact_directory = Path(__file__).resolve().parent
    certificate_path = artifact_directory / "rooted_path_basis.json"
    output_path = artifact_directory / "classify_transpositions.output.txt"
    try:
        counts, certificate = analyze(load_histories(corpus))
        for label, expected in EXPECTED_COUNTS.items():
            actual = counts[label]
            if actual != expected:
                raise RuntimeError(f"{label}: expected {expected}, got {actual}")
        if set(counts) != set(EXPECTED_COUNTS):
            unexpected = sorted(set(counts) - set(EXPECTED_COUNTS))
            missing = sorted(set(EXPECTED_COUNTS) - set(counts))
            raise RuntimeError(
                f"aggregate key mismatch: unexpected={unexpected}, missing={missing}"
            )

        certificate_bytes = (
            json.dumps(certificate, indent=2, sort_keys=True) + "\n"
        ).encode("utf-8")
        certificate_sha256 = hashlib.sha256(certificate_bytes).hexdigest()
        output_bytes = render_output(counts, certificate_sha256)

        if arguments.write_artifacts:
            certificate_path.write_bytes(certificate_bytes)
            output_path.write_bytes(output_bytes)
        else:
            for path, expected_bytes in (
                (certificate_path, certificate_bytes),
                (output_path, output_bytes),
            ):
                if path.read_bytes() != expected_bytes:
                    raise RuntimeError(
                        f"generated artifact drift: {path}; rerun with "
                        "--write-artifacts only after reviewing the change"
                    )
    except (OSError, ValueError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    sys.stdout.buffer.write(output_bytes)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
