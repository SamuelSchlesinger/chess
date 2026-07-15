#!/usr/bin/env python3
"""Optionally check the repository's TSV fixtures against Stockfish.

This is an interoperability oracle, not part of the trusted Lean development.
It checks only facts that Stockfish can answer without reconstructing game
history:

* perft node counts;
* membership in the depth-one legal move list;
* legality of every ply in a trace; and
* explicitly labelled effective FENs produced after a trace.

In particular, this script deliberately does *not* use Stockfish to validate
raw FEN en-passant history, repetition counts, draw claims, automatic-draw
thresholds, phases, or any other FIDE game-history semantics.
"""

from __future__ import annotations

import argparse
import csv
import queue
import re
import subprocess
import sys
import threading
import time
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Sequence


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DATA_DIR = REPO_ROOT / "data"

PERFT_HEADERS = ("id", "fen", "depth", "nodes", "source_id")
MOVES_HEADERS = ("id", "fen", "uci", "legal", "source_id")
TRACES_HEADERS = (
    "id",
    "start_fen",
    "uci_moves",
    "expected_raw_fen",
    "expected_effective_fen",
    "repetitions",
    "threefold",
    "fivefold",
    "halfmove_ge_100",
    "halfmove_ge_150",
    "checkmate",
    "phase",
    "source_id",
)
OPENING_PAIRS_HEADERS = (
    "id",
    "start_fen",
    "left_moves",
    "right_moves",
    "relation",
    "left_raw_fen",
    "right_raw_fen",
    "left_effective_fen",
    "right_effective_fen",
    "left_phase",
    "right_phase",
    "source_id",
)

UCI_MOVE_RE = re.compile(r"^[a-h][1-8][a-h][1-8][qrbn]?$")
ROOT_MOVE_RE = re.compile(r"^([a-h][1-8][a-h][1-8][qrbn]?):\s*([0-9]+)\s*$")
NODES_RE = re.compile(r"^Nodes searched:\s*([0-9]+)\s*$", re.IGNORECASE)
FEN_RE = re.compile(r"^Fen:\s*(.*?)\s*$", re.IGNORECASE)


@dataclass(frozen=True)
class Record:
    path: Path
    line: int
    values: dict[str, str]

    @property
    def identifier(self) -> str:
        return self.values.get("id", "<missing-id>")

    def label(self) -> str:
        return f"{self.path}:{self.line} [{self.identifier}]"


@dataclass(frozen=True)
class PerftResult:
    nodes: int
    root_moves: frozenset[str]


class EngineError(RuntimeError):
    """A UCI process failed or stopped speaking the expected protocol."""


class Stockfish:
    def __init__(self, executable: str, timeout: float) -> None:
        self.executable = executable
        self.timeout = timeout
        self._stdout: queue.Queue[str | None] = queue.Queue()
        self._recent_stdout: deque[str] = deque(maxlen=30)
        self._recent_stderr: deque[str] = deque(maxlen=30)

        try:
            self.process = subprocess.Popen(
                [executable],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="replace",
                bufsize=1,
            )
        except OSError as error:
            raise EngineError(f"cannot launch {executable!r}: {error}") from error

        assert self.process.stdout is not None
        assert self.process.stderr is not None
        threading.Thread(
            target=self._pump_stdout,
            args=(self.process.stdout,),
            daemon=True,
        ).start()
        threading.Thread(
            target=self._pump_stderr,
            args=(self.process.stderr,),
            daemon=True,
        ).start()

        self.name = executable
        try:
            self._initialize()
        except BaseException:
            # __enter__ is never reached when initialization fails, so clean
            # up the subprocess here instead of relying on the context manager.
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)
            raise

    def _pump_stdout(self, stream: Iterable[str]) -> None:
        try:
            for line in stream:
                self._stdout.put(line.rstrip("\r\n"))
        finally:
            self._stdout.put(None)

    def _pump_stderr(self, stream: Iterable[str]) -> None:
        for line in stream:
            self._recent_stderr.append(line.rstrip("\r\n"))

    def _context(self) -> str:
        parts: list[str] = []
        if self._recent_stdout:
            parts.append("recent stdout: " + " | ".join(self._recent_stdout))
        if self._recent_stderr:
            parts.append("recent stderr: " + " | ".join(self._recent_stderr))
        return "; ".join(parts)

    def _send(self, command: str) -> None:
        if "\n" in command or "\r" in command:
            raise EngineError("refusing to send a multi-line UCI command")
        if self.process.poll() is not None:
            raise EngineError(
                f"engine exited with status {self.process.returncode}; {self._context()}"
            )
        assert self.process.stdin is not None
        try:
            self.process.stdin.write(command + "\n")
            self.process.stdin.flush()
        except (BrokenPipeError, OSError) as error:
            raise EngineError(f"cannot write to engine: {error}; {self._context()}") from error

    def _read_until(
        self,
        predicate: Callable[[str], bool],
        description: str,
    ) -> list[str]:
        deadline = time.monotonic() + self.timeout
        lines: list[str] = []
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise EngineError(
                    f"timed out waiting for {description}; {self._context()}"
                )
            try:
                line = self._stdout.get(timeout=remaining)
            except queue.Empty as error:
                raise EngineError(
                    f"timed out waiting for {description}; {self._context()}"
                ) from error
            if line is None:
                raise EngineError(
                    f"engine closed stdout while waiting for {description}; {self._context()}"
                )
            self._recent_stdout.append(line)
            lines.append(line)
            if predicate(line):
                return lines

    def _initialize(self) -> None:
        self._send("uci")
        lines = self._read_until(lambda line: line.strip() == "uciok", "uciok")
        for line in lines:
            if line.startswith("id name "):
                self.name = line[len("id name ") :].strip() or self.executable
                break
        # The corpus is orthodox chess. Be explicit in case an engine persists
        # options between embedding environments.
        self._send("setoption name UCI_Chess960 value false")
        self._send("isready")
        self._read_until(lambda line: line.strip() == "readyok", "readyok")

    def _set_position(self, fen: str, moves: Sequence[str]) -> None:
        command = "position fen " + fen
        if moves:
            command += " moves " + " ".join(moves)
        self._send(command)

    def perft(self, fen: str, moves: Sequence[str], depth: int) -> PerftResult:
        self._set_position(fen, moves)
        self._send(f"go perft {depth}")
        lines = self._read_until(
            lambda line: NODES_RE.match(line.strip()) is not None,
            f"perft({depth}) result",
        )

        nodes: int | None = None
        root_moves: set[str] = set()
        for line in lines:
            stripped = line.strip()
            nodes_match = NODES_RE.match(stripped)
            if nodes_match is not None:
                nodes = int(nodes_match.group(1))
                continue
            move_match = ROOT_MOVE_RE.match(stripped)
            if move_match is not None:
                root_moves.add(move_match.group(1))

        if nodes is None:
            raise EngineError(f"engine returned no node total; {self._context()}")
        return PerftResult(nodes=nodes, root_moves=frozenset(root_moves))

    def fen(self, start_fen: str, moves: Sequence[str]) -> str:
        self._set_position(start_fen, moves)
        self._send("d")
        # isready gives us an unambiguous delimiter after the multi-line `d`
        # response, instead of leaving trailing diagnostic lines queued.
        self._send("isready")
        lines = self._read_until(lambda line: line.strip() == "readyok", "d/readyok")
        found = [match.group(1) for line in lines if (match := FEN_RE.match(line.strip()))]
        if len(found) != 1:
            raise EngineError(
                f"expected one FEN in `d` output, found {len(found)}; {self._context()}"
            )
        return found[0]

    def close(self) -> None:
        if self.process.poll() is None:
            try:
                self._send("quit")
                self.process.wait(timeout=2)
            except (EngineError, subprocess.TimeoutExpired):
                self.process.terminate()
                try:
                    self.process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait(timeout=2)

    def __enter__(self) -> Stockfish:
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> None:
        self.close()


def parse_tsv(
    path: Path,
    expected_headers: Sequence[str],
    failures: list[str],
    *,
    required: bool,
) -> list[Record]:
    if not path.exists():
        if required:
            failures.append(f"{path}: required corpus file is missing")
        return []
    if not path.is_file():
        failures.append(f"{path}: expected a regular file")
        return []

    try:
        raw_lines = path.read_text(encoding="utf-8-sig").splitlines()
    except (OSError, UnicodeError) as error:
        failures.append(f"{path}: cannot read UTF-8 TSV: {error}")
        return []

    parsed_lines: list[tuple[int, list[str]]] = []
    for line_number, raw_line in enumerate(raw_lines, start=1):
        if not raw_line.strip() or raw_line.lstrip().startswith("#"):
            continue
        try:
            cells = next(csv.reader([raw_line], delimiter="\t", strict=True))
        except (csv.Error, StopIteration) as error:
            failures.append(f"{path}:{line_number}: malformed TSV: {error}")
            continue
        parsed_lines.append((line_number, [cell.strip() for cell in cells]))

    if not parsed_lines:
        failures.append(f"{path}: contains no header")
        return []

    header_line, headers = parsed_lines[0]
    duplicate_headers = sorted({header for header in headers if headers.count(header) > 1})
    if duplicate_headers:
        failures.append(
            f"{path}:{header_line}: duplicate headers: {', '.join(duplicate_headers)}"
        )
        return []

    expected = list(expected_headers)
    if headers != expected:
        missing = [header for header in expected if header not in headers]
        extra = [header for header in headers if header not in expected]
        details: list[str] = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if extra:
            details.append("unexpected " + ", ".join(extra))
        if not missing and not extra:
            details.append("columns are out of order")
        failures.append(
            f"{path}:{header_line}: expected header {expected!r} ({'; '.join(details)})"
        )
        if missing:
            return []

    records: list[Record] = []
    seen_ids: dict[str, int] = {}
    for line_number, cells in parsed_lines[1:]:
        if len(cells) != len(headers):
            failures.append(
                f"{path}:{line_number}: expected {len(headers)} fields, found {len(cells)}"
            )
            continue
        values = dict(zip(headers, cells, strict=True))
        identifier = values.get("id", "")
        if not identifier:
            failures.append(f"{path}:{line_number}: id must not be empty")
            continue
        if identifier in seen_ids:
            failures.append(
                f"{path}:{line_number}: duplicate id {identifier!r} "
                f"(first seen on line {seen_ids[identifier]})"
            )
            continue
        seen_ids[identifier] = line_number
        records.append(Record(path=path, line=line_number, values=values))
    return records


def require_six_field_fen(record: Record, field: str, failures: list[str]) -> str | None:
    fen = record.values[field]
    if len(fen.split()) != 6:
        failures.append(f"{record.label()}: {field} must be a six-field FEN")
        return None
    if "\n" in fen or "\r" in fen:
        failures.append(f"{record.label()}: {field} contains a line break")
        return None
    return fen


def parse_nonnegative_int(record: Record, field: str, failures: list[str]) -> int | None:
    text = record.values[field]
    try:
        value = int(text, 10)
    except ValueError:
        failures.append(f"{record.label()}: {field} is not an integer: {text!r}")
        return None
    if value < 0:
        failures.append(f"{record.label()}: {field} must be nonnegative, found {value}")
        return None
    return value


def parse_bool(record: Record, field: str, failures: list[str]) -> bool | None:
    value = record.values[field]
    if value == "0":
        return False
    if value == "1":
        return True
    failures.append(f"{record.label()}: {field} must be 0 or 1, found {value!r}")
    return None


def parse_moves(record: Record, field: str, failures: list[str]) -> tuple[str, ...] | None:
    text = record.values[field]
    if text == "-":
        return ()
    if not text:
        failures.append(f"{record.label()}: {field} must use '-' for an empty move list")
        return None
    moves = tuple(text.split())
    invalid = [move for move in moves if UCI_MOVE_RE.fullmatch(move) is None]
    if invalid:
        failures.append(
            f"{record.label()}: {field} contains malformed UCI move(s): {', '.join(invalid)}"
        )
        return None
    return moves


def require_source(record: Record, failures: list[str]) -> bool:
    if record.values["source_id"]:
        return True
    failures.append(f"{record.label()}: source_id must not be empty")
    return False


class Oracle:
    def __init__(self, engine: Stockfish) -> None:
        self.engine = engine
        self._perft: dict[tuple[str, tuple[str, ...], int], PerftResult] = {}
        self._fens: dict[tuple[str, tuple[str, ...]], str] = {}

    def perft(self, fen: str, moves: Sequence[str], depth: int) -> PerftResult:
        key = (fen, tuple(moves), depth)
        if key not in self._perft:
            self._perft[key] = self.engine.perft(fen, moves, depth)
        return self._perft[key]

    def legal_moves(self, fen: str, moves: Sequence[str]) -> frozenset[str]:
        return self.perft(fen, moves, 1).root_moves

    def fen(self, start_fen: str, moves: Sequence[str]) -> str:
        key = (start_fen, tuple(moves))
        if key not in self._fens:
            self._fens[key] = self.engine.fen(start_fen, moves)
        return self._fens[key]


def validate_trace(
    oracle: Oracle,
    record: Record,
    start_fen: str,
    moves: Sequence[str],
    expected_effective_fen: str | None,
    failures: list[str],
    *,
    branch: str | None = None,
) -> int:
    prefix: list[str] = []
    branch_text = f" {branch}" if branch is not None else ""
    for ply, move in enumerate(moves, start=1):
        legal = oracle.legal_moves(start_fen, prefix)
        if move not in legal:
            failures.append(
                f"{record.label()}:{branch_text} ply {ply} move {move!r} is not legal; "
                f"legal moves: {' '.join(sorted(legal)) or '<none>'}"
            )
            return ply - 1
        prefix.append(move)

    if expected_effective_fen is not None:
        actual = oracle.fen(start_fen, prefix)
        if actual != expected_effective_fen:
            failures.append(
                f"{record.label()}:{branch_text} effective FEN mismatch: "
                f"expected {expected_effective_fen!r}, got {actual!r}"
            )
    return len(moves)


def validate_corpus(data_dir: Path, executable: str, timeout: float) -> int:
    failures: list[str] = []
    perft_rows = parse_tsv(
        data_dir / "perft.tsv", PERFT_HEADERS, failures, required=True
    )
    move_rows = parse_tsv(data_dir / "moves.tsv", MOVES_HEADERS, failures, required=True)
    trace_rows = parse_tsv(
        data_dir / "traces.tsv", TRACES_HEADERS, failures, required=True
    )
    opening_rows = parse_tsv(
        data_dir / "opening_pairs.tsv",
        OPENING_PAIRS_HEADERS,
        failures,
        required=False,
    )

    checked_perft = 0
    checked_membership = 0
    checked_traces = 0
    checked_plies = 0
    checked_opening_branches = 0
    engine_name: str | None = None
    engine_failed = False

    try:
        with Stockfish(executable, timeout) as engine:
            engine_name = engine.name
            oracle = Oracle(engine)

            for record in perft_rows:
                fen = require_six_field_fen(record, "fen", failures)
                depth = parse_nonnegative_int(record, "depth", failures)
                expected = parse_nonnegative_int(record, "nodes", failures)
                source_ok = require_source(record, failures)
                if fen is None or depth is None or expected is None or not source_ok:
                    continue
                try:
                    actual = oracle.perft(fen, (), depth).nodes
                except EngineError as error:
                    failures.append(f"{record.label()}: engine failure: {error}")
                    engine_failed = True
                    break
                checked_perft += 1
                if actual != expected:
                    failures.append(
                        f"{record.label()}: perft({depth}) expected {expected}, got {actual}"
                    )

            if not engine_failed:
                for record in move_rows:
                    fen = require_six_field_fen(record, "fen", failures)
                    move = record.values["uci"]
                    if UCI_MOVE_RE.fullmatch(move) is None:
                        failures.append(
                            f"{record.label()}: malformed UCI move in uci: {move!r}"
                        )
                        move = ""
                    expected = parse_bool(record, "legal", failures)
                    source_ok = require_source(record, failures)
                    if fen is None or not move or expected is None or not source_ok:
                        continue
                    try:
                        actual = move in oracle.legal_moves(fen, ())
                    except EngineError as error:
                        failures.append(f"{record.label()}: engine failure: {error}")
                        engine_failed = True
                        break
                    checked_membership += 1
                    if actual != expected:
                        failures.append(
                            f"{record.label()}: legal membership for {move!r} "
                            f"expected {int(expected)}, got {int(actual)}"
                        )

            if not engine_failed:
                for record in trace_rows:
                    start_fen = require_six_field_fen(record, "start_fen", failures)
                    moves = parse_moves(record, "uci_moves", failures)
                    effective_text = record.values["expected_effective_fen"]
                    effective = None
                    if effective_text not in ("", "-"):
                        effective = require_six_field_fen(
                            record, "expected_effective_fen", failures
                        )
                    source_ok = require_source(record, failures)
                    if start_fen is None or moves is None or not source_ok:
                        continue
                    if effective_text not in ("", "-") and effective is None:
                        continue
                    try:
                        checked_plies += validate_trace(
                            oracle,
                            record,
                            start_fen,
                            moves,
                            effective,
                            failures,
                        )
                    except EngineError as error:
                        failures.append(f"{record.label()}: engine failure: {error}")
                        engine_failed = True
                        break
                    checked_traces += 1

            # Opening-pair rows are checked only as two independent legal
            # traces with labelled effective endpoints. Their claimed relation,
            # raw endpoints, phases, and history semantics are intentionally
            # outside this Stockfish oracle.
            if not engine_failed:
                for record in opening_rows:
                    start_fen = require_six_field_fen(record, "start_fen", failures)
                    left_moves = parse_moves(record, "left_moves", failures)
                    right_moves = parse_moves(record, "right_moves", failures)
                    left_text = record.values["left_effective_fen"]
                    right_text = record.values["right_effective_fen"]
                    left_effective = None
                    right_effective = None
                    if left_text not in ("", "-"):
                        left_effective = require_six_field_fen(
                            record, "left_effective_fen", failures
                        )
                    if right_text not in ("", "-"):
                        right_effective = require_six_field_fen(
                            record, "right_effective_fen", failures
                        )
                    source_ok = require_source(record, failures)
                    malformed_effective = (
                        left_text not in ("", "-") and left_effective is None
                    ) or (right_text not in ("", "-") and right_effective is None)
                    if (
                        start_fen is None
                        or left_moves is None
                        or right_moves is None
                        or malformed_effective
                        or not source_ok
                    ):
                        continue
                    try:
                        checked_plies += validate_trace(
                            oracle,
                            record,
                            start_fen,
                            left_moves,
                            left_effective,
                            failures,
                            branch="left",
                        )
                        checked_opening_branches += 1
                        checked_plies += validate_trace(
                            oracle,
                            record,
                            start_fen,
                            right_moves,
                            right_effective,
                            failures,
                            branch="right",
                        )
                        checked_opening_branches += 1
                    except EngineError as error:
                        failures.append(f"{record.label()}: engine failure: {error}")
                        engine_failed = True
                        break
    except EngineError as error:
        failures.append(f"Stockfish setup failed: {error}")

    if engine_name is not None:
        print(f"engine: {engine_name}")
    print(
        "checked: "
        f"{checked_perft} perft rows, "
        f"{checked_membership} move-membership rows, "
        f"{checked_traces} traces, "
        f"{checked_opening_branches} opening branches, "
        f"{checked_plies} trace plies"
    )
    print(
        "not checked by design: raw FEN/history, repetition and draw-claim "
        "semantics, automatic-draw thresholds, checkmate claims, and phases"
    )

    if failures:
        print(f"failures ({len(failures)}):", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1
    print("all Stockfish interoperability checks passed")
    return 0


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--stockfish",
        default="stockfish",
        metavar="PATH",
        help="Stockfish executable (default: %(default)s)",
    )
    parser.add_argument(
        "--data-dir",
        "--data",
        dest="data_dir",
        type=Path,
        default=DEFAULT_DATA_DIR,
        metavar="DIR",
        help=f"TSV corpus directory (default: {DEFAULT_DATA_DIR})",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=30.0,
        metavar="SECONDS",
        help="timeout for each UCI response (default: %(default)s)",
    )
    args = parser.parse_args(argv)
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    return validate_corpus(args.data_dir.resolve(), args.stockfish, args.timeout)


if __name__ == "__main__":
    raise SystemExit(main())
