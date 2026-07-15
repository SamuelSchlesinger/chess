#!/usr/bin/env python3
"""Classify exact opening transpositions by elementary algebraic mechanism.

This read-only pilot validates quantitative claims in the parent research
document.  It replays the repository's pinned Lichess opening-name corpus and
compares three equivalence relations on its distinct legal histories:

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
nonzero if either or any checked aggregate changes.
"""

from __future__ import annotations

import csv
import hashlib
import heapq
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import DefaultDict, Dict, Iterable, List, Mapping, Set, Tuple

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
    """Find local ``abc <-> cba`` relations visible on both catalog paths."""

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


def replay(history: History) -> chess.Board:
    board = chess.Board()
    for token in history:
        move = chess.Move.from_uci(token)
        if move not in board.legal_moves:
            raise RuntimeError(f"illegal recombined history: {' '.join(history)}")
        board.push(move)
    return board


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


def analyze(boards: Mapping[History, chess.Board]) -> Dict[str, int]:
    fibres: DefaultDict[PositionKey, List[History]] = defaultdict(list)
    for history, board in boards.items():
        fibres[repetition_key(board)].append(history)

    non_singleton = [histories for histories in fibres.values() if len(histories) > 1]
    endpoint_excess = sum(len(histories) - 1 for histories in non_singleton)

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
    projected_edges: Dict[Tuple[PositionKey, PositionKey], str] = {}
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

    chord_tail_tokens: List[int] = []
    chord_max_tail: List[int] = []
    chord_trace_compatible = 0
    chord_depth_changing = 0
    chord_direct_braids = 0
    for (source, target), move in projected_edges.items():
        if (source, target) in tree_edges:
            continue
        left = canonical[source] + (move,)
        right = canonical[target]
        common = 0
        while common < min(len(left), len(right)) and left[common] == right[common]:
            common += 1
        left_tail = left[common:]
        right_tail = right[common:]
        chord_tail_tokens.append(len(left_tail) + len(right_tail))
        chord_max_tail.append(max(len(left_tail), len(right_tail)))
        chord_trace_compatible += int(trace_signature(left) == trace_signature(right))
        chord_depth_changing += int(len(left) != len(right))
        chord_direct_braids += int(is_direct_alternating_braid(left, right))

    if len(tree_edges) != len(fibres) - 1:
        raise RuntimeError("canonical paths did not induce a rooted arborescence")
    if len(chord_tail_tokens) != chord_count:
        raise RuntimeError("chord count disagrees with indegree excess")

    return {
        "distinct histories": len(boards),
        "repetition nodes": len(fibres),
        "non-singleton endpoint fibres": len(non_singleton),
        "endpoint excess": endpoint_excess,
        "single-signature fibres": single_signature_fibres,
        "mixed-signature fibres": mixed_signature_fibres,
        "depth-varying fibres": depth_varying_fibres,
        "maximum excess explainable by ply permutations": permutation_excess,
        "excess requiring more than ply permutations": beyond_permutation_excess,
        "catalog-visible direct braid relations": len(braid_edges),
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
        "shortest-basis trace-compatible chords": chord_trace_compatible,
        "shortest-basis depth-changing chords": chord_depth_changing,
        "shortest-basis direct alternating braids": chord_direct_braids,
        "shortest-basis relations with at most 3 plies per side": sum(
            length <= 3 for length in chord_max_tail
        ),
        "shortest-basis relations with at most 5 plies per side": sum(
            length <= 5 for length in chord_max_tail
        ),
        "shortest-basis total tail tokens": sum(chord_tail_tokens),
        "shortest-basis maximum one-side tail": max(chord_max_tail),
        "shortest-basis maximum two-side tail tokens": max(chord_tail_tokens),
    }


# Independently inspected on 2026-07-14.  Keeping these values in the
# executable turns subsequent runs into regression checks rather than a report
# that silently changes with code or data.
EXPECTED_COUNTS: Mapping[str, int] = {  # populated below after graph-shortest audit
    "distinct histories": 8_646,
    "repetition nodes": 7_848,
    "non-singleton endpoint fibres": 570,
    "endpoint excess": 798,
    "single-signature fibres": 557,
    "mixed-signature fibres": 13,
    "depth-varying fibres": 3,
    "maximum excess explainable by ply permutations": 785,
    "excess requiring more than ply permutations": 13,
    "catalog-visible direct braid relations": 296,
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
    "shortest-basis trace-compatible chords": 201,
    "shortest-basis depth-changing chords": 1,
    "shortest-basis direct alternating braids": 84,
    "shortest-basis relations with at most 3 plies per side": 86,
    "shortest-basis relations with at most 5 plies per side": 138,
    "shortest-basis total tail tokens": 2_112,
    "shortest-basis maximum one-side tail": 17,
    "shortest-basis maximum two-side tail tokens": 34,
}


def main() -> int:
    corpus = repository_root() / "data" / "lichess-openings" / "all.tsv"
    try:
        counts = analyze(load_histories(corpus))
        for label, expected in EXPECTED_COUNTS.items():
            actual = counts[label]
            if actual != expected:
                raise RuntimeError(f"{label}: expected {expected}, got {actual}")
    except (OSError, ValueError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    for label, count in counts.items():
        print(f"{label}: {count}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
