#!/usr/bin/env python3
"""Validate a Chess.com PGN and build a private, reproducible learner profile.

The metadata pass is exact relative to the supplied PGN.  Supplying Stockfish
adds a bounded diagnostic pass over the slower games.  Engine-loss thresholds
are calibration heuristics, not theorem-backed labels.

Example:

    uv run --with chess==1.11.2 python scripts/player_games.py \
      ~/Downloads/chess_com_games.pgn --player YOUR_USERNAME \
      --stockfish /opt/homebrew/bin/stockfish \
      --json-output data/private/player-baseline.json \
      --markdown-output data/private/player-baseline.md \
      --cards-output data/private/diagnostic-cards.json
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import logging
import os
import re
import statistics
import sys
import tempfile
from collections import Counter, defaultdict
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Iterator, TextIO

import chess
import chess.engine
import chess.pgn


EXPECTED_CHESS_VERSION = "1.11.2"
SCHEMA_VERSION = 1
ENGINE_ANALYSIS_VERSION = 2
DEFAULT_NODES = 20_000
DEFAULT_CONFIRMATION_NODES = 100_000
DEFAULT_MIN_ESTIMATED_SECONDS = 600
DEFAULT_MIN_PLIES = 10
MATE_SCORE = 100_000
LOSS_THRESHOLDS = {
    "large": 200,
    "medium": 100,
    "small": 50,
}
PRIVATE_OUTPUT_ROOT = Path(__file__).resolve().parents[1] / "data" / "private"
SCRIPT_SHA256 = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()


@dataclass
class GameRecord:
    index: int
    game_id: str
    game: chess.pgn.Game
    player_color: chess.Color
    opponent: str
    result: str
    player_rating: int | None
    opponent_rating: int | None
    date: str
    end_time: str
    time_control: str
    estimated_seconds: int | None
    termination: str
    termination_kind: str
    plies: int
    opening_prefix: str
    player_first_move: str | None
    player_second_move: str | None
    castled: bool
    castle_ply: int | None
    early_queen_moves: int
    captures: int
    checks: int
    parse_errors: list[str]
    standard_start: bool
    final_checkmate: bool
    final_threefold: bool


class ValidatingGameBuilder(chess.pgn.GameBuilder):
    """Retain the movetext result token for header/terminator validation."""

    def __init__(self) -> None:
        super().__init__()
        self.original_header_result: str | None = None
        self.movetext_result: str | None = None

    def visit_header(self, tagname: str, tagvalue: str) -> None:
        if tagname == "Result":
            self.original_header_result = tagvalue
        super().visit_header(tagname, tagvalue)

    def visit_result(self, result: str) -> None:
        self.movetext_result = result
        super().visit_result(result)

    def result(self) -> chess.pgn.Game:
        game = super().result()
        game.original_header_result = self.original_header_result
        game.movetext_result = self.movetext_result
        return game


def normalized_name(value: str) -> str:
    return value.strip().casefold()


def integer_header(value: str | None) -> int | None:
    if value is None:
        return None
    try:
        return int(value)
    except ValueError:
        return None


def parse_time_control(value: str) -> tuple[int | None, int | None, int | None]:
    """Return base, increment, and a forty-move estimated duration."""

    match = re.fullmatch(r"(\d+)(?:\+(\d+))?", value)
    if match is None:
        return None, None, None
    base = int(match.group(1))
    increment = int(match.group(2) or 0)
    return base, increment, base + 40 * increment


def pace_bucket(estimated_seconds: int | None) -> str:
    """A project-local pace bucket, deliberately not a platform rating class."""

    if estimated_seconds is None:
        return "unknown"
    if estimated_seconds < 180:
        return "very-fast"
    if estimated_seconds < 600:
        return "fast"
    return "deliberate"


def result_for_player(result: str, color: chess.Color) -> str:
    if result == "1/2-1/2":
        return "draw"
    if result == "1-0":
        return "win" if color == chess.WHITE else "loss"
    if result == "0-1":
        return "win" if color == chess.BLACK else "loss"
    return "unknown"


def termination_kind(value: str) -> str:
    lower = value.casefold()
    if "checkmate" in lower:
        return "checkmate"
    if "resignation" in lower:
        return "resignation"
    if "on time" in lower or "time out" in lower:
        return "time"
    if "abandoned" in lower:
        return "abandoned"
    if "repetition" in lower:
        return "repetition"
    if "stalemate" in lower:
        return "stalemate"
    if "insufficient" in lower:
        return "insufficient-material"
    return "other"


def canonical_game_id(game: chess.pgn.Game) -> str:
    """A cross-export occurrence key that excludes mutable rating metadata."""

    initial_fen = game.board().fen(en_passant="fen")
    identity = {
        "site": game.headers.get("Site", "").strip().casefold(),
        "date": game.headers.get("Date", "").strip(),
        "end_time": game.headers.get("EndTime", "").strip(),
        "white": normalized_name(game.headers.get("White", "")),
        "black": normalized_name(game.headers.get("Black", "")),
        "initial_fen": initial_fen,
        "mainline_uci": [move.uci() for move in game.mainline_moves()],
    }
    return json_sha256(identity)


def read_all_games(handle: TextIO) -> list[chess.pgn.Game]:
    games: list[chess.pgn.Game] = []
    pgn_logger = logging.getLogger("chess.pgn")
    logger_was_disabled = pgn_logger.disabled
    pgn_logger.disabled = True
    try:
        while True:
            game = chess.pgn.read_game(handle, Visitor=ValidatingGameBuilder)
            if game is None:
                return games
            games.append(game)
    finally:
        pgn_logger.disabled = logger_was_disabled


def infer_player(games: Iterable[chess.pgn.Game]) -> str:
    appearances: Counter[str] = Counter()
    display: dict[str, str] = {}
    total = 0
    for game in games:
        total += 1
        for key in ("White", "Black"):
            name = game.headers.get(key, "").strip()
            if name:
                normalized = normalized_name(name)
                appearances[normalized] += 1
                display.setdefault(normalized, name)
    if not appearances:
        raise ValueError("PGN contains no White or Black player names")
    candidates = sorted(
        normalized for normalized, count in appearances.items() if count == total
    )
    if not candidates:
        raise ValueError(
            "could not infer one player present in every game; pass --player explicitly"
        )
    if len(candidates) != 1:
        raise ValueError(
            f"player inference is ambiguous across {len(candidates)} names; "
            "pass --player explicitly"
        )
    return display[candidates[0]]


def inspect_game(
    game: chess.pgn.Game,
    index: int,
    player: str,
) -> GameRecord | None:
    white = game.headers.get("White", "")
    black = game.headers.get("Black", "")
    target = normalized_name(player)
    if normalized_name(white) == target:
        player_color = chess.WHITE
        opponent = black
        rating_header = "WhiteElo"
        opponent_rating_header = "BlackElo"
    elif normalized_name(black) == target:
        player_color = chess.BLACK
        opponent = white
        rating_header = "BlackElo"
        opponent_rating_header = "WhiteElo"
    else:
        return None

    board = game.board()
    standard_start = (
        "FEN" not in game.headers
        and game.headers.get("Variant", "Standard").casefold() == "standard"
        and board.fen() == chess.Board().fen()
    )
    sans: list[str] = []
    player_first_move: str | None = None
    player_second_move: str | None = None
    castled = False
    castle_ply: int | None = None
    early_queen_moves = 0
    captures = 0
    checks = 0
    plies = 0
    player_move_index = 0

    for move in game.mainline_moves():
        plies += 1
        san = board.san(move)
        sans.append(san)
        if board.turn == player_color:
            player_move_index += 1
            if player_first_move is None:
                player_first_move = san
            elif player_move_index == 2 and player_second_move is None:
                player_second_move = san
            if board.is_castling(move):
                castled = True
                if castle_ply is None:
                    castle_ply = plies
            piece = board.piece_at(move.from_square)
            if (
                piece is not None
                and piece.piece_type == chess.QUEEN
                and player_move_index <= 5
            ):
                early_queen_moves += 1
            captures += int(board.is_capture(move))
            checks += int(board.gives_check(move))
        board.push(move)

    time_control = game.headers.get("TimeControl", "?")
    _, _, estimated_seconds = parse_time_control(time_control)
    header_result = game.headers.get("Result", "*")
    errors = [str(error) for error in game.errors]
    return GameRecord(
        index=index,
        game_id=canonical_game_id(game),
        game=game,
        player_color=player_color,
        opponent=opponent,
        result=result_for_player(header_result, player_color),
        player_rating=integer_header(game.headers.get(rating_header)),
        opponent_rating=integer_header(game.headers.get(opponent_rating_header)),
        date=game.headers.get("Date", "????.??.??"),
        end_time=game.headers.get("EndTime", ""),
        time_control=time_control,
        estimated_seconds=estimated_seconds,
        termination=game.headers.get("Termination", ""),
        termination_kind=termination_kind(game.headers.get("Termination", "")),
        plies=plies,
        opening_prefix=" ".join(sans[:6]),
        player_first_move=player_first_move,
        player_second_move=player_second_move,
        castled=castled,
        castle_ply=castle_ply,
        early_queen_moves=early_queen_moves,
        captures=captures,
        checks=checks,
        parse_errors=errors,
        standard_start=standard_start,
        final_checkmate=board.is_checkmate(),
        final_threefold=board.is_repetition(3),
    )


def load_records(
    path: Path,
    requested_player: str | None,
) -> tuple[str, list[GameRecord], int, bytes]:
    if chess.__version__ != EXPECTED_CHESS_VERSION:
        raise ValueError(
            f"expected chess=={EXPECTED_CHESS_VERSION}, got {chess.__version__}"
        )
    source_bytes = path.read_bytes()
    games = read_all_games(io.StringIO(source_bytes.decode("utf-8-sig")))
    if not games:
        raise ValueError("PGN contains no games")
    parse_error_indexes = [
        index for index, game in enumerate(games, start=1) if game.errors
    ]
    if parse_error_indexes:
        indexes = ", ".join(str(index) for index in parse_error_indexes)
        raise ValueError(
            f"refusing a partial profile: parser errors in game(s) {indexes}"
        )
    result_mismatch_indexes = [
        index
        for index, game in enumerate(games, start=1)
        if getattr(game, "original_header_result", None) is None
        or getattr(game, "movetext_result", None) is None
        or game.original_header_result != game.movetext_result
    ]
    if result_mismatch_indexes:
        indexes = ", ".join(str(index) for index in result_mismatch_indexes)
        raise ValueError(
            f"refusing inconsistent header/movetext results in game(s) {indexes}"
        )
    indexes_by_id: dict[str, list[int]] = defaultdict(list)
    for index, game in enumerate(games, start=1):
        indexes_by_id[canonical_game_id(game)].append(index)
    duplicate_groups = [
        indexes for indexes in indexes_by_id.values() if len(indexes) > 1
    ]
    if duplicate_groups:
        rendered = "; ".join(
            ",".join(str(index) for index in indexes) for indexes in duplicate_groups
        )
        raise ValueError(
            f"refusing duplicate games at source indexes {rendered}; "
            "deduplicate overlapping exports first"
        )
    player = requested_player or infer_player(games)
    records = [
        record
        for index, game in enumerate(games, start=1)
        if (record := inspect_game(game, index, player)) is not None
    ]
    if not records:
        raise ValueError("the requested player does not occur in the PGN")
    return player, records, len(games), source_bytes


def counter_dict(values: Iterable[str]) -> dict[str, int]:
    return dict(sorted(Counter(values).items(), key=lambda item: (-item[1], item[0])))


def result_counter(records: Iterable[GameRecord]) -> dict[str, int]:
    counts = Counter(record.result for record in records)
    return {key: counts.get(key, 0) for key in ("win", "loss", "draw", "unknown")}


def is_completed_non_abandoned(record: GameRecord) -> bool:
    return record.result != "unknown" and record.termination_kind != "abandoned"


def rating_values_summary(values: list[int]) -> dict[str, Any]:
    if not values:
        return {"observations": 0}
    return {
        "observations": len(values),
        "minimum": min(values),
        "maximum": max(values),
        "median": statistics.median(values),
    }


def rating_summary(records: list[GameRecord]) -> dict[str, Any]:
    values = [
        record.player_rating for record in records if record.player_rating is not None
    ]
    grouped: dict[str, list[int]] = defaultdict(list)
    for record in records:
        if record.player_rating is not None:
            grouped[record.time_control].append(record.player_rating)
    return {
        "overall": rating_values_summary(values),
        "by_time_control": {
            time_control: rating_values_summary(ratings)
            for time_control, ratings in sorted(grouped.items())
        },
        "interpretation": (
            "The export mixes ratings observed under several raw time controls "
            "and likely multiple speed-rating pools, but does not identify the "
            "pool explicitly. Do not read the overall values as one trajectory "
            "or assume that every raw control has a separate pool."
        ),
    }


def time_control_rows(records: list[GameRecord]) -> list[dict[str, Any]]:
    grouped: dict[str, list[GameRecord]] = defaultdict(list)
    for record in records:
        grouped[record.time_control].append(record)
    rows = []
    for raw, games in grouped.items():
        base, increment, estimated = parse_time_control(raw)
        ratings = [
            game.player_rating for game in games if game.player_rating is not None
        ]
        rows.append(
            {
                "time_control": raw,
                "base_seconds": base,
                "increment_seconds": increment,
                "estimated_seconds": estimated,
                "pace_bucket": pace_bucket(estimated),
                "games": len(games),
                "results": result_counter(games),
                "player_ratings": rating_values_summary(ratings),
            }
        )
    return sorted(rows, key=lambda row: (-row["games"], row["time_control"]))


def build_profile(
    path: Path,
    player: str,
    records: list[GameRecord],
    games_in_file: int,
    source_bytes: bytes,
) -> dict[str, Any]:
    ratings = rating_summary(records)
    dates = sorted(record.date for record in records)
    by_color = {}
    for color, label in ((chess.WHITE, "white"), (chess.BLACK, "black")):
        subset = [record for record in records if record.player_color == color]
        by_color[label] = {"games": len(subset), "results": result_counter(subset)}

    deliberate = [
        record
        for record in records
        if record.estimated_seconds is not None
        and record.estimated_seconds >= DEFAULT_MIN_ESTIMATED_SECONDS
    ]
    completed_deliberate = [
        record for record in deliberate if is_completed_non_abandoned(record)
    ]
    plies = [record.plies for record in records]
    white_first_moves = [
        record.player_first_move or "?"
        for record in records
        if record.player_color == chess.WHITE
    ]
    white_second_moves = [
        record.player_second_move or "?"
        for record in records
        if record.player_color == chess.WHITE
    ]
    black_prefixes = [
        record.opening_prefix
        for record in records
        if record.player_color == chess.BLACK
    ]
    black_first_moves = [
        record.player_first_move or "?"
        for record in records
        if record.player_color == chess.BLACK
    ]
    white_prefixes = [
        record.opening_prefix
        for record in records
        if record.player_color == chess.WHITE
    ]
    parse_error_records = [record for record in records if record.parse_errors]
    terminal_mismatches = []
    for record in records:
        if record.termination_kind == "checkmate" and not record.final_checkmate:
            terminal_mismatches.append(
                {"game_index": record.index, "expected": "checkmate"}
            )
        if record.termination_kind == "repetition" and not record.final_threefold:
            terminal_mismatches.append(
                {"game_index": record.index, "expected": "threefold repetition"}
            )

    return {
        "schema_version": SCHEMA_VERSION,
        "privacy": {
            "classification": "personal-reidentifiable",
            "handling_requirement": (
                "ignored-local-only unless an explicit output override is used"
            ),
        },
        "assurance": {
            "metadata": "exact-relative-to-supplied-pgn",
            "engine": "absent",
            "human_learning_claims": "not-yet-tested",
        },
        "source": {
            "file_name": path.name,
            "sha256": hashlib.sha256(source_bytes).hexdigest(),
            "bytes": len(source_bytes),
        },
        "parser": {
            "python_chess_version": EXPECTED_CHESS_VERSION,
            "game_id_method": (
                "SHA-256 of site, date, end time, normalized players, initial FEN, "
                "and complete mainline UCI; mutable ratings are excluded"
            ),
        },
        "player": player,
        "validation": {
            "games_in_file": games_in_file,
            "games_matching_player": len(records),
            "games_without_parse_errors": len(records) - len(parse_error_records),
            "games_with_parse_errors": len(parse_error_records),
            "parse_errors": [
                {
                    "game_index": record.index,
                    "game_id": record.game_id,
                    "errors": record.parse_errors,
                }
                for record in parse_error_records
            ],
            "standard_start_games": sum(record.standard_start for record in records),
            "plies_replayed": sum(record.plies for record in records),
            "terminal_semantic_mismatches": terminal_mismatches,
            "clock_comments_present": b"[%clk " in source_bytes,
        },
        "period": {"first_date": dates[0], "last_date": dates[-1]},
        "results": result_counter(records),
        "by_color": by_color,
        "ratings": ratings,
        "time_controls": time_control_rows(records),
        "deliberate_sample": {
            "minimum_estimated_seconds": DEFAULT_MIN_ESTIMATED_SECONDS,
            "games": len(deliberate),
            "results": result_counter(deliberate),
            "completed_non_abandoned_games": len(completed_deliberate),
            "completed_non_abandoned_results": result_counter(completed_deliberate),
        },
        "termination_kinds": counter_dict(
            record.termination_kind for record in records
        ),
        "game_length": {
            "minimum_plies": min(plies),
            "maximum_plies": max(plies),
            "median_plies": statistics.median(plies),
        },
        "observable_habits": {
            "games_castled": sum(record.castled for record in records),
            "games_with_early_queen_move": sum(
                record.early_queen_moves > 0 for record in records
            ),
            "player_captures": sum(record.captures for record in records),
            "player_checks": sum(record.checks for record in records),
            "interpretation": (
                "Descriptive PGN counts only; they do not establish move quality, "
                "tactical awareness, or causal habits."
            ),
        },
        "opening_surface": {
            "white_first_moves": counter_dict(white_first_moves),
            "white_second_moves": counter_dict(white_second_moves),
            "black_first_moves": counter_dict(black_first_moves),
            "white_first_six_ply_prefixes": counter_dict(white_prefixes),
            "black_first_six_ply_prefixes": counter_dict(black_prefixes),
            "interpretation": (
                "Observed prefixes, not repertoire recommendations or opening-strength estimates."
            ),
        },
        "games": [
            {
                "index": record.index,
                "game_id": record.game_id,
                "date": record.date,
                "end_time": record.end_time,
                "color": "white" if record.player_color == chess.WHITE else "black",
                "opponent": record.opponent,
                "result": record.result,
                "player_rating": record.player_rating,
                "opponent_rating": record.opponent_rating,
                "time_control": record.time_control,
                "estimated_seconds": record.estimated_seconds,
                "termination_kind": record.termination_kind,
                "plies": record.plies,
                "opening_prefix": record.opening_prefix,
                "castled": record.castled,
                "castle_ply": record.castle_ply,
                "parse_errors": record.parse_errors,
                "final_checkmate": record.final_checkmate,
                "final_threefold": record.final_threefold,
            }
            for record in records
        ],
    }


def pov_score(info: dict[str, Any], color: chess.Color) -> tuple[int, int | None]:
    score = info["score"].pov(color)
    numeric = score.score(mate_score=MATE_SCORE)
    if numeric is None:
        raise ValueError("engine returned a score without centipawn or mate value")
    return numeric, score.mate()


def principal_variation_san(
    board: chess.Board,
    moves: Iterable[chess.Move],
    limit: int = 6,
) -> list[str]:
    replay = board.copy(stack=False)
    sans: list[str] = []
    for move in moves:
        if len(sans) == limit or move not in replay.legal_moves:
            break
        sans.append(replay.san(move))
        replay.push(move)
    return sans


def principal_variation_uci(moves: Iterable[chess.Move], limit: int = 6) -> list[str]:
    return [move.uci() for move in list(moves)[:limit]]


def effective_position_id(board: chess.Board) -> str:
    """The monorepo's canonical legal-en-passant four-field EPD."""

    fields = board.fen(en_passant="legal").split()
    if len(fields) != 6:
        raise ValueError("python-chess emitted a non-six-field FEN")
    return " ".join(fields[:4])


def analyze_root(
    engine: chess.engine.SimpleEngine,
    board: chess.Board,
    color: chess.Color,
    nodes: int,
    root_move: chess.Move | None = None,
) -> tuple[int, int | None, list[str], list[str]]:
    options: dict[str, Any] = {}
    if root_move is not None:
        options["root_moves"] = [root_move]
    if "Clear Hash" in engine.options:
        engine.configure({"Clear Hash": None})
    info = engine.analyse(
        board,
        chess.engine.Limit(nodes=nodes),
        game=object(),
        **options,
    )
    score, mate = pov_score(info, color)
    moves = info.get("pv", [])
    return (
        score,
        mate,
        principal_variation_san(board, moves),
        principal_variation_uci(moves),
    )


def decision_loss(
    best_score: int,
    best_mate: int | None,
    played_score: int,
    played_mate: int | None,
) -> tuple[int, str | None]:
    """Rank a move without letting mate-distance arithmetic swamp the sample."""

    if best_mate is not None and best_mate > 0:
        if played_mate is not None and played_mate > 0:
            return 0, "preserved-forced-win"
        return 10_000, "missed-forced-win"
    if best_mate is not None and best_mate < 0:
        if played_mate is not None and played_mate < 0:
            return 0, "already-in-forced-loss"
        return 0, "bounded-search-disagreement"
    if played_mate is not None and played_mate < 0:
        return 10_000, "allowed-forced-mate"
    return max(0, best_score - played_score), None


def classify_loss(loss: int) -> str:
    if loss >= LOSS_THRESHOLDS["large"]:
        return "large"
    if loss >= LOSS_THRESHOLDS["medium"]:
        return "medium"
    if loss >= LOSS_THRESHOLDS["small"]:
        return "small"
    return "stable"


def phase_for_move(fullmove_number: int) -> str:
    if fullmove_number <= 10:
        return "opening"
    if fullmove_number <= 25:
        return "middlegame"
    return "late-game"


@contextmanager
def executable_snapshot(source: Path) -> Iterator[tuple[Path, dict[str, Any]]]:
    """Copy and hash exactly the executable bytes used by both engine passes."""

    with tempfile.TemporaryDirectory(prefix="chess-engine-") as directory:
        target = Path(directory) / "stockfish-snapshot"
        descriptor = os.open(
            target,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL,
            0o700,
        )
        digest = hashlib.sha256()
        size = 0
        try:
            os.fchmod(descriptor, 0o700)
            with source.open("rb") as source_handle:
                with os.fdopen(descriptor, "wb") as target_handle:
                    descriptor = -1
                    while chunk := source_handle.read(1024 * 1024):
                        digest.update(chunk)
                        size += len(chunk)
                        target_handle.write(chunk)
        finally:
            if descriptor >= 0:
                os.close(descriptor)
        yield (
            target,
            {
                "file_name": source.name,
                "sha256": digest.hexdigest(),
                "bytes": size,
            },
        )


def configure_engine(engine: chess.engine.SimpleEngine) -> dict[str, int]:
    engine_name = str(engine.id.get("name", ""))
    if not engine_name.casefold().startswith("stockfish"):
        raise ValueError("engine must identify itself as Stockfish")
    if "Clear Hash" not in engine.options:
        raise ValueError("engine must support the UCI Clear Hash option")
    missing = [name for name in ("Threads", "Hash") if name not in engine.options]
    if missing:
        raise ValueError(
            "Stockfish must expose deterministic options: " + ", ".join(missing)
        )
    configured: dict[str, int] = {}
    for name, value in (("Threads", 1), ("Hash", 64)):
        engine.configure({name: value})
        configured[name] = value
    return configured


def collect_engine_decisions(
    selected: list[GameRecord],
    engine_path: Path,
    nodes: int,
) -> tuple[dict[str, Any], dict[str, int], list[dict[str, Any]]]:
    decisions: list[dict[str, Any]] = []
    engine = chess.engine.SimpleEngine.popen_uci(str(engine_path))
    try:
        configured = configure_engine(engine)
        engine_identity = dict(engine.id)
        for record in selected:
            board = record.game.board()
            initial_fen = board.fen(en_passant="fen")
            history_uci: list[str] = []
            for ply, move in enumerate(record.game.mainline_moves(), start=1):
                if board.turn != record.player_color:
                    board.push(move)
                    history_uci.append(move.uci())
                    continue
                fen = board.fen(en_passant="fen")
                position_id = effective_position_id(board)
                actual_san = board.san(move)
                actual_uci = move.uci()
                fullmove = board.fullmove_number
                best_score, best_mate, best_pv, best_pv_uci = analyze_root(
                    engine,
                    board,
                    record.player_color,
                    nodes,
                )
                best_san = best_pv[0] if best_pv else None
                best_uci = best_pv_uci[0] if best_pv_uci else None
                if best_uci == actual_uci:
                    played_score = best_score
                    played_mate = best_mate
                    played_pv = best_pv
                    played_pv_uci = best_pv_uci
                else:
                    played_score, played_mate, played_pv, played_pv_uci = analyze_root(
                        engine,
                        board,
                        record.player_color,
                        nodes,
                        root_move=move,
                    )
                loss, mate_event = decision_loss(
                    best_score,
                    best_mate,
                    played_score,
                    played_mate,
                )
                history_before = list(history_uci)
                board.push(move)
                history_uci.append(actual_uci)
                decisions.append(
                    {
                        "game_index": record.index,
                        "game_id": record.game_id,
                        "date": record.date,
                        "time_control": record.time_control,
                        "result": record.result,
                        "color": (
                            "white" if record.player_color == chess.WHITE else "black"
                        ),
                        "ply": ply,
                        "move_number": fullmove,
                        "phase": phase_for_move(fullmove),
                        "fen": fen,
                        "position_id": position_id,
                        "initial_fen": initial_fen,
                        "history_uci": history_before,
                        "actual_san": actual_san,
                        "actual_uci": actual_uci,
                        "best_san": best_san,
                        "best_uci": best_uci,
                        "best_line_san": best_pv,
                        "best_line_uci": best_pv_uci,
                        "played_line_san": played_pv,
                        "played_line_uci": played_pv_uci,
                        "best_score_cp_equivalent": best_score,
                        "played_score_cp_equivalent": played_score,
                        "best_mate": best_mate,
                        "played_mate": played_mate,
                        "mate_event": mate_event,
                        "loss_cp_equivalent": loss,
                        "loss_bucket": classify_loss(loss),
                    }
                )
        return engine_identity, configured, decisions
    finally:
        engine.quit()


def confirm_candidates(
    candidates: list[dict[str, Any]],
    engine_path: Path,
    confirmation_nodes: int,
    engine_factory: Any = chess.engine.SimpleEngine.popen_uci,
) -> tuple[dict[str, Any], dict[str, int], list[dict[str, Any]]]:
    confirmed: list[dict[str, Any]] = []
    engine = engine_factory(str(engine_path))
    try:
        configured = configure_engine(engine)
        engine_identity = dict(engine.id)
        for candidate in candidates:
            board = replay_candidate_board(candidate)
            move = chess.Move.from_uci(candidate["actual_uci"])
            if move not in board.legal_moves:
                raise ValueError(
                    "candidate played move is not legal during confirmation"
                )
            best_score, best_mate, best_pv, best_pv_uci = analyze_root(
                engine,
                board,
                board.turn,
                confirmation_nodes,
            )
            best_san = best_pv[0] if best_pv else None
            best_uci = best_pv_uci[0] if best_pv_uci else None
            if best_uci == candidate["actual_uci"]:
                played_score = best_score
                played_mate = best_mate
                played_pv = best_pv
                played_pv_uci = best_pv_uci
            else:
                played_score, played_mate, played_pv, played_pv_uci = analyze_root(
                    engine,
                    board,
                    board.turn,
                    confirmation_nodes,
                    root_move=move,
                )
            loss, mate_event = decision_loss(
                best_score,
                best_mate,
                played_score,
                played_mate,
            )
            candidate["confirmation"] = {
                "nodes_per_position": confirmation_nodes,
                "best_san": best_san,
                "best_uci": best_uci,
                "best_line_san": best_pv,
                "best_line_uci": best_pv_uci,
                "played_line_san": played_pv,
                "played_line_uci": played_pv_uci,
                "best_score_cp_equivalent": best_score,
                "played_score_cp_equivalent": played_score,
                "best_mate": best_mate,
                "played_mate": played_mate,
                "mate_event": mate_event,
                "loss_cp_equivalent": loss,
                "loss_bucket": classify_loss(loss),
            }
            if candidate["confirmation"]["loss_bucket"] in ("medium", "large"):
                confirmed.append(candidate)
        return engine_identity, configured, confirmed
    finally:
        engine.quit()


def add_engine_analysis(
    profile: dict[str, Any],
    records: list[GameRecord],
    stockfish: Path,
    nodes: int,
    confirmation_nodes: int,
    minimum_estimated_seconds: int,
    minimum_plies: int,
    include_abandoned: bool,
    max_games: int | None,
) -> None:
    selected = [
        record
        for record in records
        if not record.parse_errors
        and record.standard_start
        and record.result != "unknown"
        and record.estimated_seconds is not None
        and record.estimated_seconds >= minimum_estimated_seconds
        and record.plies >= minimum_plies
        and (include_abandoned or record.termination_kind != "abandoned")
    ]
    selected.sort(key=lambda record: record.index)
    if max_games is not None:
        selected = selected[:max_games]
    if not selected:
        raise ValueError("no games satisfy the engine-analysis selection")

    with executable_snapshot(stockfish) as (engine_path, engine_binary):
        engine_identity, engine_options, decisions = collect_engine_decisions(
            selected,
            engine_path,
            nodes,
        )

        by_game: dict[str, list[dict[str, Any]]] = defaultdict(list)
        for decision in decisions:
            by_game[decision["game_id"]].append(decision)
        first_actionable = []
        for game_decisions in by_game.values():
            game_decisions.sort(key=lambda decision: decision["ply"])
            candidate = next(
                (
                    decision
                    for decision in game_decisions
                    if decision["loss_bucket"] == "large"
                ),
                None,
            )
            if candidate is None:
                candidate = next(
                    (
                        decision
                        for decision in game_decisions
                        if decision["loss_bucket"] == "medium"
                    ),
                    None,
                )
            if candidate is not None:
                first_actionable.append(candidate)
        candidates = sorted(
            first_actionable,
            key=lambda decision: (
                -decision["loss_cp_equivalent"],
                decision["date"],
                decision["game_index"],
                decision["ply"],
            ),
        )
        confirmation_identity, confirmation_options, confirmed = confirm_candidates(
            candidates,
            engine_path,
            confirmation_nodes,
        )
    if confirmation_identity != engine_identity:
        raise ValueError("engine identity changed between analysis passes")
    if confirmation_options != engine_options:
        raise ValueError("engine options changed between analysis passes")

    confirmed.sort(
        key=lambda decision: (
            -decision["confirmation"]["loss_cp_equivalent"],
            decision["date"],
            decision["game_index"],
            decision["ply"],
        )
    )
    losses = [decision["loss_cp_equivalent"] for decision in decisions]
    bucket_counts = Counter(decision["loss_bucket"] for decision in decisions)
    phase_counts: dict[str, Counter[str]] = defaultdict(Counter)
    for decision in decisions:
        phase_counts[decision["phase"]][decision["loss_bucket"]] += 1

    profile["assurance"]["engine"] = "bounded-stockfish-estimate"
    profile["engine_analysis"] = {
        "analysis_algorithm_version": ENGINE_ANALYSIS_VERSION,
        "generated_at_utc": datetime.now(timezone.utc)
        .replace(microsecond=0)
        .isoformat(),
        "source_snapshot": {
            "sha256": profile["source"]["sha256"],
            "bytes": profile["source"]["bytes"],
        },
        "parser": profile["parser"],
        "script_snapshot": {
            "file_name": Path(__file__).name,
            "sha256": SCRIPT_SHA256,
        },
        "engine": engine_identity,
        "engine_binary": engine_binary,
        "engine_options": engine_options,
        "search_protocol": {
            "same_root_comparison": True,
            "ucinewgame_before_each_root_search": True,
            "clear_hash_before_each_root_search": True,
        },
        "nodes_per_position": nodes,
        "confirmation_nodes_per_position": confirmation_nodes,
        "minimum_estimated_seconds": minimum_estimated_seconds,
        "minimum_plies": minimum_plies,
        "maximum_games": max_games,
        "completed_result_required": True,
        "abandoned_games_included": include_abandoned,
        "selected_games": len(selected),
        "decisions": len(decisions),
        "loss_thresholds_cp_equivalent": LOSS_THRESHOLDS,
        "method": (
            "At each player turn, compare independent fresh-state bounded searches "
            "from the same root: an unrestricted best search and a search restricted "
            "to the played move. Identical best and played moves have zero loss. "
            "Preserved forced results have zero loss; newly allowed or missed forced "
            "mates receive a separate fixed priority. Thresholds are calibration "
            "heuristics. For each selected game, retain its earliest large-loss "
            "position; if none exists, retain its earliest medium-loss position. "
            "Rerun that position at the declared confirmation budget and discard it "
            "if it does not remain medium or large."
        ),
        "loss_summary": {
            "median_cp_equivalent": statistics.median(losses),
            "mean_cp_equivalent": round(statistics.mean(losses), 2),
            "buckets": {
                key: bucket_counts.get(key, 0)
                for key in ("stable", "small", "medium", "large")
            },
            "by_phase": {
                phase: {
                    key: counts.get(key, 0)
                    for key in ("stable", "small", "medium", "large")
                }
                for phase, counts in sorted(phase_counts.items())
            },
        },
        "first_actionable_training_candidates": confirmed,
    }


def replay_uci(initial_fen: str, moves: Iterable[str]) -> chess.Board:
    board = chess.Board(initial_fen)
    for token in moves:
        move = chess.Move.from_uci(token)
        if move not in board.legal_moves:
            raise ValueError(f"illegal card history move {token} from {board.fen()}")
        board.push(move)
    return board


def replay_candidate_board(candidate: dict[str, Any]) -> chess.Board:
    board = replay_uci(candidate["initial_fen"], candidate["history_uci"])
    if board.fen(en_passant="fen") != candidate["fen"]:
        raise ValueError("candidate history does not reproduce its confirmation FEN")
    return board


def json_sha256(value: Any) -> str:
    canonical = json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    )
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def diagnostic_card_content_version(card: dict[str, Any]) -> str:
    """Hash the tested semantics, excluding prose and replaceable evidence."""

    semantic_content = {
        "kind": card["kind"],
        "occurrence": card["occurrence"],
        "position": card["position"],
        "task": card["prompt"]["task"],
        "orientation": card["prompt"]["orientation"],
        "reference_move_uci": card["answer"]["reference_move_uci"],
    }
    return f"sha256:{json_sha256(semantic_content)}"


def build_diagnostic_cards(profile: dict[str, Any]) -> dict[str, Any]:
    analysis = profile.get("engine_analysis")
    if analysis is None:
        raise ValueError("--cards-output requires --stockfish")
    card_analysis = {
        "analysis_algorithm_version": analysis["analysis_algorithm_version"],
        "generated_at_utc": analysis["generated_at_utc"],
        "assurance": profile["assurance"]["engine"],
        "source_snapshot": analysis["source_snapshot"],
        "parser": analysis["parser"],
        "script_snapshot": analysis["script_snapshot"],
        "engine": analysis["engine"],
        "engine_binary": analysis["engine_binary"],
        "engine_options": analysis["engine_options"],
        "search_protocol": analysis["search_protocol"],
        "selection_nodes_per_position": analysis["nodes_per_position"],
        "confirmation_nodes_per_position": analysis["confirmation_nodes_per_position"],
        "loss_thresholds_cp_equivalent": analysis["loss_thresholds_cp_equivalent"],
        "selection": {
            "minimum_estimated_seconds": analysis["minimum_estimated_seconds"],
            "minimum_plies": analysis["minimum_plies"],
            "maximum_games": analysis["maximum_games"],
            "completed_result_required": analysis["completed_result_required"],
            "abandoned_games_included": analysis["abandoned_games_included"],
            "candidate_rule": (
                "earliest large loss per game; earliest medium if no large loss"
            ),
        },
    }
    analysis_config = {
        key: value for key, value in card_analysis.items() if key != "generated_at_utc"
    }
    analysis_config_version = f"sha256:{json_sha256(analysis_config)}"
    cards = []
    for candidate in analysis["first_actionable_training_candidates"]:
        confirmation = candidate["confirmation"]
        history = candidate["history_uci"]
        if len(history) != candidate["ply"] - 1:
            raise ValueError("diagnostic history length does not match decision ply")
        board = replay_uci(candidate["initial_fen"], history)
        current_fen = board.fen(en_passant="fen")
        position_id = effective_position_id(board)
        if current_fen != candidate["fen"]:
            raise ValueError("diagnostic history does not reproduce its raw FEN")
        if position_id != candidate["position_id"]:
            raise ValueError("diagnostic history does not reproduce its PositionId")
        for token in (candidate["actual_uci"], confirmation["best_uci"]):
            if token is None or chess.Move.from_uci(token) not in board.legal_moves:
                raise ValueError("diagnostic answer contains an illegal root move")
        expected_roots = {
            "best_line_uci": confirmation["best_uci"],
            "played_line_uci": candidate["actual_uci"],
        }
        for line_name, expected_root in expected_roots.items():
            line = confirmation[line_name]
            if not line or line[0] != expected_root:
                raise ValueError(
                    f"diagnostic {line_name} does not begin with its root move"
                )
            replay_uci(current_fen, line)

        card = {
            "card_id": (f"engine-diagnostic/{candidate['game_id']}/{candidate['ply']}"),
            "kind": "engine-diagnostic-move",
            "occurrence": {
                "game_id": candidate["game_id"],
                "decision_ply": candidate["ply"],
                "initial_fen": candidate["initial_fen"],
                "history_uci": history,
            },
            "position": {
                "current_fen": current_fen,
                "position_id": position_id,
            },
            "prompt": {
                "orientation": candidate["color"],
                "task": "select-move",
                "text": "What would you play?",
            },
            "answer": {
                "reference_move_uci": confirmation["best_uci"],
                "played_move_uci": candidate["actual_uci"],
                "reference_line_uci": confirmation["best_line_uci"],
                "played_line_uci": confirmation["played_line_uci"],
                "reference_score_cp_equivalent": confirmation[
                    "best_score_cp_equivalent"
                ],
                "played_score_cp_equivalent": confirmation[
                    "played_score_cp_equivalent"
                ],
                "loss_cp_equivalent": confirmation["loss_cp_equivalent"],
                "loss_bucket": confirmation["loss_bucket"],
                "mate_event": confirmation["mate_event"],
            },
            "tags": [candidate["phase"], "personal-game"],
        }
        card["content_version"] = diagnostic_card_content_version(card)
        evidence_content = {
            "analysis_config_version": analysis_config_version,
            "answer": card["answer"],
        }
        card["evidence_version"] = f"sha256:{json_sha256(evidence_content)}"
        cards.append(card)
    return {
        "schema": "chess-diagnostic-cards",
        "schema_version": 1,
        "privacy": {
            "classification": "personal-reidentifiable",
            "handling_requirement": (
                "ignored-local-only unless an explicit output override is used"
            ),
        },
        "analysis": card_analysis,
        "analysis_config_version": analysis_config_version,
        "cards": cards,
    }


def markdown_table(headers: list[str], rows: list[list[Any]]) -> list[str]:
    rendered = ["| " + " | ".join(headers) + " |"]
    rendered.append("|" + "|".join("---" for _ in headers) + "|")
    rendered.extend("| " + " | ".join(str(cell) for cell in row) + " |" for row in rows)
    return rendered


def sorted_count_items(counts: dict[str, int]) -> list[tuple[str, int]]:
    return sorted(counts.items(), key=lambda item: (-item[1], item[0]))


def render_markdown(profile: dict[str, Any]) -> str:
    results = profile["results"]
    validation = profile["validation"]
    overall_ratings = profile["ratings"]["overall"]
    lines = [
        f"# Local player baseline: {profile['player']}",
        "",
        "> This is a measurement of one exported sample, not a judgment of the player.",
        "",
        "## Sample",
        "",
        f"- Games: {validation['games_matching_player']} of "
        f"{validation['games_in_file']} in the file",
        f"- Dates: {profile['period']['first_date']} through {profile['period']['last_date']}",
        f"- Parse-clean games: {validation['games_without_parse_errors']}",
        f"- Standard-start games: {validation['standard_start_games']}",
        f"- Results: {results['win']} wins, {results['loss']} losses, {results['draw']} draws",
        f"- Header-rating range across mixed raw controls: "
        f"{overall_ratings.get('minimum', '?')}–{overall_ratings.get('maximum', '?')} "
        f"(not one rating trajectory)",
        f"- PGN SHA-256: `{profile['source']['sha256']}`",
        "",
        "## Time controls",
        "",
    ]
    time_rows = []
    for row in profile["time_controls"]:
        row_results = row["results"]
        time_rows.append(
            [
                row["time_control"],
                row["pace_bucket"],
                row["games"],
                f"{row_results['win']}-{row_results['loss']}-{row_results['draw']}",
                f"{row['player_ratings'].get('minimum', '?')}–"
                f"{row['player_ratings'].get('maximum', '?')}",
            ]
        )
    lines.extend(
        markdown_table(
            ["Control", "Project pace", "Games", "W-L-D", "Rating range"],
            time_rows,
        )
    )
    completed = profile["deliberate_sample"]["completed_non_abandoned_results"]
    white_first = profile["opening_surface"]["white_first_moves"]
    white_second = profile["opening_surface"]["white_second_moves"]
    black_first = profile["opening_surface"]["black_first_moves"]
    white_first_text = ", ".join(
        f"{move}: {count}" for move, count in sorted_count_items(white_first)
    )
    white_second_text = ", ".join(
        f"{move}: {count}" for move, count in sorted_count_items(white_second)
    )
    black_first_text = ", ".join(
        f"{move}: {count}" for move, count in sorted_count_items(black_first)
    )
    lines.extend(
        [
            "",
            "## Directly observable structure",
            "",
            f"- Completed slower-game baseline: "
            f"{profile['deliberate_sample']['completed_non_abandoned_games']} games, "
            f"{completed['win']}-{completed['loss']}-{completed['draw']} W-L-D.",
            f"- First moves as White: {white_first_text}.",
            f"- Second moves as White: {white_second_text}.",
            f"- First moves as Black: {black_first_text}.",
            f"- Castled in {profile['observable_habits']['games_castled']} games.",
            f"- Moved the queen in the first five player moves in "
            f"{profile['observable_habits']['games_with_early_queen_move']} games.",
            f"- Median game length: {profile['game_length']['median_plies']} plies.",
            "- Terminations: "
            + ", ".join(
                f"{count} {kind}"
                for kind, count in sorted_count_items(profile["termination_kinds"])
            )
            + ".",
            f"- Per-move clock comments present: "
            f"{'yes' if validation['clock_comments_present'] else 'no'}.",
            "",
            "These are descriptive counts. They do not establish that a move was good, a piece was",
            "hung, or a tactical opportunity was understood. Without per-move clock comments, the",
            "sample cannot establish where time was spent or whether time pressure caused a move.",
        ]
    )

    analysis = profile.get("engine_analysis")
    if analysis is None:
        lines.extend(
            [
                "",
                "## Next evidence needed",
                "",
                "Run the bounded Stockfish pass on the deliberate sample, then convert the largest",
                "stable errors into diagnostic positions. No tactical or strategic weakness is",
                "inferred from metadata alone.",
            ]
        )
    else:
        summary = analysis["loss_summary"]
        buckets = summary["buckets"]
        abandoned_policy = (
            "including abandoned games"
            if analysis["abandoned_games_included"]
            else "excluding abandoned games"
        )
        lines.extend(
            [
                "",
                "## Bounded engine diagnostic",
                "",
                f"- Selected diagnostic games: {analysis['selected_games']}",
                f"- Selection: parse-clean, standard-start, declared-result games of at least "
                f"{analysis['minimum_plies']} plies, {abandoned_policy}, with project-estimated "
                f"control length "
                f"`base + 40 × increment >= {analysis['minimum_estimated_seconds']}s`.",
                f"- Player decisions analyzed: {analysis['decisions']}",
                f"- Nodes per position: {analysis['nodes_per_position']}",
                f"- Candidate-confirmation nodes per position: "
                f"{analysis['confirmation_nodes_per_position']}",
                f"- Heuristic loss buckets: {buckets['small']} small, "
                f"{buckets['medium']} medium, {buckets['large']} large",
                "",
                "These are bounded engine estimates. They rank candidate review positions; they do",
                "not yet explain the human cause of an error. Estimated control length is a",
                "filter, not observed elapsed time.",
                "",
                "Phase is a move-number bucket (`opening` means moves 1–10), not a diagnosis of",
                "opening knowledge.",
                "",
                "### Largest confirmed per-game review positions",
                "",
            ]
        )
        candidate_rows = []
        for decision in analysis["first_actionable_training_candidates"][:15]:
            confirmation = decision["confirmation"]
            candidate_rows.append(
                [
                    decision["game_index"],
                    decision["move_number"],
                    decision["phase"],
                    decision["actual_san"],
                    confirmation["best_san"] or "terminal",
                    confirmation["loss_cp_equivalent"],
                ]
            )
        lines.extend(
            markdown_table(
                [
                    "Game",
                    "Move",
                    "Phase",
                    "Played",
                    "Confirmed engine reference",
                    "Bounded loss (cp-equivalent)",
                ],
                candidate_rows,
            )
        )

    lines.extend(
        [
            "",
            "## Assurance",
            "",
            f"- Metadata: {profile['assurance']['metadata']}",
            f"- Engine: {profile['assurance']['engine']}",
            f"- Human learning claims: {profile['assurance']['human_learning_claims']}",
            "",
        ]
    )
    return "\n".join(lines)


def paths_refer_to_same_file(left: Path, right: Path) -> bool:
    left_resolved = str(left.resolve(strict=False)).casefold()
    right_resolved = str(right.resolve(strict=False)).casefold()
    if left_resolved == right_resolved:
        return True
    try:
        return left.exists() and right.exists() and os.path.samefile(left, right)
    except OSError:
        return False


def validate_output_paths(
    pgn: Path,
    stockfish: Path | None,
    outputs: Iterable[Path | None],
    allow_output_outside_private: bool,
    allow_worktree_input: bool,
    allow_stdout: bool,
) -> None:
    selected = [output for output in outputs if output is not None]
    if not selected and not allow_stdout:
        raise ValueError(
            "refusing personal data on stdout; pass an output path under "
            "data/private or use --allow-stdout explicitly"
        )

    named_paths = [("input PGN", pgn)]
    if stockfish is not None:
        named_paths.append(("Stockfish executable", stockfish))
    named_paths.extend(
        [(f"output {index}", output) for index, output in enumerate(selected, start=1)]
    )
    for left_index, (left_name, left) in enumerate(named_paths):
        for right_name, right in named_paths[left_index + 1 :]:
            if paths_refer_to_same_file(left, right):
                raise ValueError(
                    f"{left_name} and {right_name} refer to the same file: {left}"
                )

    for output in selected:
        if output.is_symlink():
            raise ValueError(f"refusing a symlink output path: {output}")
        try:
            if output.exists() and output.stat().st_nlink != 1:
                raise ValueError(f"refusing a multiply linked output file: {output}")
        except OSError as error:
            raise ValueError(
                f"could not inspect output path safely: {output}"
            ) from error

    project_root = PRIVATE_OUTPUT_ROOT.parents[1].resolve(strict=False)
    private_root = PRIVATE_OUTPUT_ROOT.resolve(strict=False)
    resolved_pgn = pgn.resolve(strict=False)
    try:
        resolved_pgn.relative_to(project_root)
        input_is_in_worktree = True
    except ValueError:
        input_is_in_worktree = False
    try:
        resolved_pgn.relative_to(private_root)
        input_is_private = True
    except ValueError:
        input_is_private = False
    if input_is_in_worktree and not input_is_private and not allow_worktree_input:
        raise ValueError(
            "refusing a raw PGN inside the worktree outside data/private; "
            "move it outside the repository or use --allow-worktree-input explicitly"
        )

    if allow_output_outside_private:
        return
    for output in selected:
        resolved = output.resolve(strict=False)
        try:
            resolved.relative_to(private_root)
        except ValueError as error:
            raise ValueError(
                f"refusing personal output outside {PRIVATE_OUTPUT_ROOT}; "
                "use --allow-output-outside-private explicitly"
            ) from error


def write_outputs(
    profile: dict[str, Any],
    json_output: Path | None,
    markdown_output: Path | None,
    cards_output: Path | None,
) -> None:
    json_text = json.dumps(profile, indent=2, sort_keys=True) + "\n"
    markdown_text = render_markdown(profile)
    cards_text = None
    if cards_output is not None:
        cards_text = (
            json.dumps(build_diagnostic_cards(profile), indent=2, sort_keys=True) + "\n"
        )
    if json_output is None and markdown_output is None and cards_output is None:
        print(markdown_text, end="")
        return
    for output, content in (
        (json_output, json_text),
        (markdown_output, markdown_text),
        (cards_output, cards_text),
    ):
        if output is not None and content is not None:
            atomic_write_private_text(output, content)


def atomic_write_private_text(output: Path, content: str) -> None:
    """Atomically replace an artifact without following the destination path."""

    output.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{output.name}.", suffix=".tmp", dir=output.parent
    )
    temporary_path: Path | None = Path(temporary_name)
    try:
        os.fchmod(descriptor, 0o600)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            descriptor = -1
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_path, output)
        temporary_path = None
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def self_test() -> None:
    def expect_value_error(action: Any) -> None:
        try:
            action()
        except ValueError:
            return
        raise AssertionError("expected ValueError")

    sample = """[Event "Fixture one"]
[Site "Local"]
[Date "2026.01.01"]
[White "Learner"]
[Black "Other"]
[Result "1-0"]
[WhiteElo "500"]
[BlackElo "510"]
[TimeControl "600"]
[Termination "Learner won by checkmate"]

1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7# 1-0

[Event "Fixture two"]
[Site "Local"]
[Date "2026.01.02"]
[White "OtherTwo"]
[Black "Learner"]
[Result "1/2-1/2"]
[WhiteElo "520"]
[BlackElo "505"]
[TimeControl "180+2"]
[Termination "Game drawn by repetition"]

1. Nf3 Nf6 2. Ng1 Ng8 3. Nf3 Nf6 4. Ng1 Ng8 1/2-1/2
"""
    games = read_all_games(io.StringIO(sample))
    assert infer_player(games) == "Learner"
    first_game_id = canonical_game_id(games[0])
    original_rating = games[0].headers.get("WhiteElo")
    games[0].headers["WhiteElo"] = "999"
    assert canonical_game_id(games[0]) == first_game_id
    if original_rating is not None:
        games[0].headers["WhiteElo"] = original_rating
    assert canonical_game_id(games[1]) != first_game_id
    records = [
        inspect_game(game, index, "Learner") for index, game in enumerate(games, 1)
    ]
    checked = [record for record in records if record is not None]
    assert len(checked) == 2
    assert result_counter(checked) == {"win": 1, "loss": 0, "draw": 1, "unknown": 0}
    assert checked[0].player_color == chess.WHITE
    assert checked[1].player_color == chess.BLACK
    assert checked[0].opening_prefix == "e4 e5 Bc4 Nc6 Qh5 Nf6"
    assert checked[0].player_second_move == "Bc4"
    assert is_completed_non_abandoned(checked[0])
    checked[0].result = "unknown"
    assert not is_completed_non_abandoned(checked[0])
    checked[0].result = "win"
    assert parse_time_control("180+2") == (180, 2, 260)
    initial = chess.Board()
    assert effective_position_id(initial) == (
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -"
    )
    assert decision_loss(100, None, -150, None) == (250, None)
    assert decision_loss(MATE_SCORE - 3, 3, 500, None) == (
        10_000,
        "missed-forced-win",
    )
    assert decision_loss(-MATE_SCORE + 5, -5, -MATE_SCORE + 2, -2) == (
        0,
        "already-in-forced-loss",
    )
    repeated = replay_uci(
        chess.STARTING_FEN,
        ["g1f3", "g8f6", "f3g1", "f6g8"] * 2,
    )
    assert repeated.is_repetition(3)
    assert not chess.Board(repeated.fen()).is_repetition(3)
    replayed_candidate = replay_candidate_board(
        {
            "initial_fen": chess.STARTING_FEN,
            "history_uci": ["g1f3", "g8f6", "f3g1", "f6g8"] * 2,
            "fen": repeated.fen(en_passant="fen"),
        }
    )
    assert replayed_candidate.is_repetition(3)

    class RepetitionCheckingEngine:
        def __init__(self) -> None:
            self.id = {"name": "Stockfish Fixture"}
            self.options = {"Threads": None, "Hash": None, "Clear Hash": None}
            self.saw_repetition = False

        def configure(self, options: dict[str, Any]) -> None:
            assert options

        def analyse(
            self,
            board: chess.Board,
            limit: chess.engine.Limit,
            **options: Any,
        ) -> dict[str, Any]:
            assert limit.nodes == 100
            assert options.get("game") is not None
            self.saw_repetition = board.is_repetition(3)
            move = options.get("root_moves", [chess.Move.from_uci("e2e4")])[0]
            return {
                "score": chess.engine.PovScore(chess.engine.Cp(0), board.turn),
                "pv": [move],
            }

        def quit(self) -> None:
            pass

    fake_engine = RepetitionCheckingEngine()
    _, _, fake_confirmed = confirm_candidates(
        [
            {
                "initial_fen": chess.STARTING_FEN,
                "history_uci": ["g1f3", "g8f6", "f3g1", "f6g8"] * 2,
                "fen": repeated.fen(en_passant="fen"),
                "actual_uci": "e2e4",
            }
        ],
        Path("fixture-engine"),
        100,
        engine_factory=lambda _: fake_engine,
    )
    assert fake_engine.saw_repetition
    assert fake_confirmed == []
    generic_engine = RepetitionCheckingEngine()
    generic_engine.id = {"name": "Generic UCI Engine"}
    expect_value_error(lambda: configure_engine(generic_engine))
    incomplete_stockfish = RepetitionCheckingEngine()
    del incomplete_stockfish.options["Hash"]
    expect_value_error(lambda: configure_engine(incomplete_stockfish))

    card = {
        "kind": "engine-diagnostic-move",
        "occurrence": {
            "game_id": "fixture",
            "decision_ply": 1,
            "initial_fen": chess.STARTING_FEN,
            "history_uci": [],
        },
        "position": {
            "current_fen": chess.STARTING_FEN,
            "position_id": effective_position_id(chess.Board()),
        },
        "prompt": {
            "orientation": "white",
            "task": "select-move",
            "text": "What would you play?",
        },
        "answer": {"reference_move_uci": "e2e4"},
    }
    version = diagnostic_card_content_version(card)
    prose_change = json.loads(json.dumps(card))
    prose_change["prompt"]["text"] = "Choose a move."
    assert diagnostic_card_content_version(prose_change) == version
    answer_change = json.loads(json.dumps(card))
    answer_change["answer"]["reference_move_uci"] = "d2d4"
    assert diagnostic_card_content_version(answer_change) != version

    fixture_profile = {
        "assurance": {"engine": "bounded-stockfish-estimate"},
        "engine_analysis": {
            "analysis_algorithm_version": ENGINE_ANALYSIS_VERSION,
            "generated_at_utc": "2026-01-01T00:00:00+00:00",
            "source_snapshot": {"sha256": "11", "bytes": 2},
            "parser": {"python_chess_version": EXPECTED_CHESS_VERSION},
            "script_snapshot": {"file_name": "fixture.py", "sha256": "22"},
            "engine": {"name": "FixtureEngine"},
            "engine_binary": {"file_name": "fixture", "sha256": "00", "bytes": 1},
            "engine_options": {"Threads": 1, "Hash": 64},
            "search_protocol": {
                "same_root_comparison": True,
                "ucinewgame_before_each_root_search": True,
                "clear_hash_before_each_root_search": True,
            },
            "nodes_per_position": 20,
            "confirmation_nodes_per_position": 100,
            "loss_thresholds_cp_equivalent": LOSS_THRESHOLDS,
            "minimum_estimated_seconds": 600,
            "minimum_plies": 1,
            "maximum_games": None,
            "completed_result_required": True,
            "abandoned_games_included": False,
            "first_actionable_training_candidates": [
                {
                    "game_id": "fixture",
                    "ply": 1,
                    "initial_fen": chess.STARTING_FEN,
                    "history_uci": [],
                    "fen": chess.Board().fen(en_passant="fen"),
                    "position_id": effective_position_id(chess.Board()),
                    "actual_uci": "d2d4",
                    "color": "white",
                    "phase": "opening",
                    "confirmation": {
                        "best_uci": "e2e4",
                        "best_line_uci": ["e2e4"],
                        "played_line_uci": ["d2d4"],
                        "best_score_cp_equivalent": 20,
                        "played_score_cp_equivalent": -100,
                        "loss_cp_equivalent": 120,
                        "loss_bucket": "medium",
                        "mate_event": None,
                    },
                }
            ],
        },
    }
    fixture_bundle = build_diagnostic_cards(fixture_profile)
    assert len(fixture_bundle["cards"]) == 1
    fixture_card = fixture_bundle["cards"][0]
    assert fixture_card["content_version"].startswith("sha256:")
    assert fixture_card["evidence_version"].startswith("sha256:")
    assert fixture_bundle["analysis_config_version"].startswith("sha256:")
    evidence_change = json.loads(json.dumps(fixture_profile))
    evidence_change["engine_analysis"]["first_actionable_training_candidates"][0][
        "confirmation"
    ]["best_score_cp_equivalent"] = 21
    changed_bundle = build_diagnostic_cards(evidence_change)
    assert (
        changed_bundle["cards"][0]["content_version"] == fixture_card["content_version"]
    )
    assert (
        changed_bundle["cards"][0]["evidence_version"]
        != fixture_card["evidence_version"]
    )

    ambiguous = """[Event "Ambiguous one"]
[White "Alpha"]
[Black "Beta"]
[Result "1-0"]

1. e4 e5 1-0

[Event "Ambiguous two"]
[White "Beta"]
[Black "Alpha"]
[Result "0-1"]

1. d4 d5 0-1
"""
    try:
        infer_player(read_all_games(io.StringIO(ambiguous)))
    except ValueError as error:
        assert "Alpha" not in str(error)
        assert "Beta" not in str(error)
    else:
        raise AssertionError("expected ambiguous player inference")

    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "fixture.pgn"
        path.write_text(sample, encoding="utf-8")
        player, loaded, total, source_bytes = load_records(path, None)
        path.write_text("changed after snapshot", encoding="utf-8")
        profile = build_profile(path, player, loaded, total, source_bytes)
        assert profile["source"]["sha256"] == hashlib.sha256(source_bytes).hexdigest()
        assert profile["validation"]["games_without_parse_errors"] == 2
        assert profile["deliberate_sample"]["games"] == 1
        assert "Local player baseline: Learner" in render_markdown(profile)
        round_tripped_profile = json.loads(json.dumps(profile, sort_keys=True))
        assert render_markdown(round_tripped_profile) == render_markdown(profile)
        json_output = Path(directory) / "profile.json"
        markdown_output = Path(directory) / "profile.md"
        write_outputs(profile, json_output, markdown_output, None)
        assert (json_output.stat().st_mode & 0o777) == 0o600
        assert (markdown_output.stat().st_mode & 0o777) == 0o600
        protected = Path(directory) / "protected"
        swapped_output = Path(directory) / "swapped-output"
        protected.write_text("do not overwrite", encoding="utf-8")
        swapped_output.symlink_to(protected)
        atomic_write_private_text(swapped_output, "private replacement\n")
        assert protected.read_text(encoding="utf-8") == "do not overwrite"
        assert not swapped_output.is_symlink()
        assert swapped_output.read_text(encoding="utf-8") == "private replacement\n"

        duplicate_path = Path(directory) / "duplicates.pgn"
        duplicate_path.write_text(sample + "\n" + sample, encoding="utf-8")
        expect_value_error(lambda: load_records(duplicate_path, "Learner"))

        malformed_path = Path(directory) / "malformed.pgn"
        malformed_path.write_text(
            """[Event "Malformed"]
[White "Learner"]
[Black "Other"]
[Result "*"]

1. e4 e5 2. Bh6 *
""",
            encoding="utf-8",
        )
        expect_value_error(lambda: load_records(malformed_path, "Learner"))

        unrelated_malformed_path = Path(directory) / "unrelated-malformed.pgn"
        unrelated_malformed_path.write_text(
            sample
            + """

[Event "Unrelated malformed"]
[White "SecretOne"]
[Black "SecretTwo"]
[Result "*"]

1. e4 e5 2. Bh6 *
""",
            encoding="utf-8",
        )
        expect_value_error(lambda: load_records(unrelated_malformed_path, "Learner"))

        mismatched_result_path = Path(directory) / "mismatched-result.pgn"
        mismatched_result_path.write_text(
            """[Event "Mismatch"]
[White "Learner"]
[Black "Other"]
[Result "1-0"]

1. e4 e5 0-1
""",
            encoding="utf-8",
        )
        expect_value_error(lambda: load_records(mismatched_result_path, "Learner"))

        wildcard_result_path = Path(directory) / "wildcard-result.pgn"
        wildcard_result_path.write_text(
            """[Event "Wildcard mismatch"]
[White "Learner"]
[Black "Other"]
[Result "*"]

1. e4 e5 1-0
""",
            encoding="utf-8",
        )
        expect_value_error(lambda: load_records(wildcard_result_path, "Learner"))

        unrelated_game = """[Event "Unrelated duplicate"]
[White "SecretOne"]
[Black "SecretTwo"]
[Result "1-0"]

1. d4 d5 1-0
"""
        unrelated_duplicate_path = Path(directory) / "unrelated-duplicate.pgn"
        unrelated_duplicate_path.write_text(
            sample + "\n" + unrelated_game + "\n" + unrelated_game,
            encoding="utf-8",
        )
        expect_value_error(lambda: load_records(unrelated_duplicate_path, "Learner"))

        expect_value_error(
            lambda: validate_output_paths(path, None, (path,), True, False, False)
        )
        expect_value_error(
            lambda: validate_output_paths(
                path,
                None,
                (Path(directory) / "Same", Path(directory) / "same"),
                True,
                False,
                False,
            )
        )
        expect_value_error(
            lambda: validate_output_paths(path, None, (), False, False, False)
        )
        expect_value_error(
            lambda: validate_output_paths(
                path,
                None,
                (Path(directory) / "outside.json",),
                False,
                False,
                False,
            )
        )
        engine_path = Path(directory) / "stockfish"
        engine_path.write_bytes(b"fixture")
        with executable_snapshot(engine_path) as (snapshot_path, metadata):
            engine_path.write_bytes(b"changed")
            assert snapshot_path.read_bytes() == b"fixture"
            assert metadata["sha256"] == hashlib.sha256(b"fixture").hexdigest()
        expect_value_error(
            lambda: validate_output_paths(
                path,
                engine_path,
                (engine_path,),
                True,
                False,
                False,
            )
        )
        hardlink_source = Path(directory) / "hardlink-source"
        hardlink_output = Path(directory) / "hardlink-output"
        hardlink_source.write_bytes(b"shared")
        os.link(hardlink_source, hardlink_output)
        expect_value_error(
            lambda: validate_output_paths(
                path,
                None,
                (hardlink_output,),
                True,
                False,
                False,
            )
        )
        worktree_pgn = PRIVATE_OUTPUT_ROOT.parents[1] / "raw-self-test.pgn"
        expect_value_error(
            lambda: validate_output_paths(
                worktree_pgn,
                None,
                (PRIVATE_OUTPUT_ROOT / "self-test.json",),
                False,
                False,
                False,
            )
        )
        validate_output_paths(path, None, (), False, False, True)
        validate_output_paths(
            path,
            None,
            (PRIVATE_OUTPUT_ROOT / "self-test.json",),
            False,
            False,
            False,
        )
    print("player-games self-test passed")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pgn", nargs="?", type=Path)
    parser.add_argument("--player")
    parser.add_argument("--stockfish", type=Path)
    parser.add_argument("--nodes", type=int, default=DEFAULT_NODES)
    parser.add_argument(
        "--confirmation-nodes",
        type=int,
        default=DEFAULT_CONFIRMATION_NODES,
    )
    parser.add_argument(
        "--min-estimated-seconds",
        type=int,
        default=DEFAULT_MIN_ESTIMATED_SECONDS,
    )
    parser.add_argument("--min-plies", type=int, default=DEFAULT_MIN_PLIES)
    parser.add_argument("--include-abandoned", action="store_true")
    parser.add_argument("--max-games", type=int)
    parser.add_argument("--json-output", type=Path)
    parser.add_argument("--markdown-output", type=Path)
    parser.add_argument("--cards-output", type=Path)
    parser.add_argument(
        "--allow-output-outside-private",
        action="store_true",
        help="explicitly allow personal artifacts outside data/private",
    )
    parser.add_argument(
        "--allow-worktree-input",
        action="store_true",
        help="explicitly allow a raw PGN inside the worktree outside data/private",
    )
    parser.add_argument(
        "--allow-stdout",
        action="store_true",
        help="explicitly allow a personal Markdown profile on stdout",
    )
    parser.add_argument("--self-test", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        if args.self_test:
            self_test()
            return 0
        if args.pgn is None:
            raise ValueError("a PGN path is required unless --self-test is used")
        if args.nodes <= 0:
            raise ValueError("--nodes must be positive")
        if args.confirmation_nodes <= 0:
            raise ValueError("--confirmation-nodes must be positive")
        if args.max_games is not None and args.max_games <= 0:
            raise ValueError("--max-games must be positive")
        if args.min_plies < 0:
            raise ValueError("--min-plies must be nonnegative")
        validate_output_paths(
            args.pgn,
            args.stockfish,
            (args.json_output, args.markdown_output, args.cards_output),
            args.allow_output_outside_private,
            args.allow_worktree_input,
            args.allow_stdout,
        )
        player, records, total, source_bytes = load_records(args.pgn, args.player)
        profile = build_profile(
            args.pgn,
            player,
            records,
            total,
            source_bytes,
        )
        if args.stockfish is not None:
            add_engine_analysis(
                profile,
                records,
                args.stockfish,
                args.nodes,
                args.confirmation_nodes,
                args.min_estimated_seconds,
                args.min_plies,
                args.include_abandoned,
                args.max_games,
            )
        write_outputs(
            profile,
            args.json_output,
            args.markdown_output,
            args.cards_output,
        )
        return 0
    except (OSError, ValueError, chess.engine.EngineError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
