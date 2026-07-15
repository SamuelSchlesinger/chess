#!/usr/bin/env python3
"""Validate and rank the research routes under explicit weight profiles.

The input scores are research judgments, not measurements.  This script makes
the arithmetic and sensitivity analysis reproducible; it does not make the
judgments objective.

Run from any directory:

    python3 research/novel-chess-theory/certified-chess-knowledge/data/rank_candidates.py

Pass ``--check`` to compare the rendered report with ``ranking-output.txt``.
"""

from __future__ import annotations

import argparse
import csv
from pathlib import Path


HERE = Path(__file__).resolve().parent
INPUT = HERE / "candidate_scores.csv"
EXPECTED = HERE / "ranking-output.txt"
CRITERIA = (
    "novelty",
    "usefulness",
    "lean_leverage",
    "data_access",
    "falsifying_pilot",
)
PROFILES = {
    "equal": dict.fromkeys(CRITERIA, 1),
    "player-first": {
        "novelty": 1,
        "usefulness": 3,
        "lean_leverage": 1,
        "data_access": 1,
        "falsifying_pilot": 2,
    },
    "novelty-first": {
        "novelty": 3,
        "usefulness": 2,
        "lean_leverage": 1,
        "data_access": 1,
        "falsifying_pilot": 1,
    },
    "Lean-first": {
        "novelty": 1,
        "usefulness": 2,
        "lean_leverage": 3,
        "data_access": 1,
        "falsifying_pilot": 1,
    },
}


def load_scores() -> list[tuple[str, dict[str, int]]]:
    with INPUT.open(newline="", encoding="utf-8") as handle:
        reader = csv.DictReader(handle)
        expected = ["candidate", *CRITERIA]
        if reader.fieldnames != expected:
            raise ValueError(f"expected columns {expected}, got {reader.fieldnames}")
        rows: list[tuple[str, dict[str, int]]] = []
        seen: set[str] = set()
        for line, row in enumerate(reader, start=2):
            name = row["candidate"].strip()
            if not name or name in seen:
                raise ValueError(f"line {line}: blank or duplicate candidate {name!r}")
            seen.add(name)
            scores = {criterion: int(row[criterion]) for criterion in CRITERIA}
            for criterion, score in scores.items():
                if not 1 <= score <= 5:
                    raise ValueError(
                        f"line {line}: {criterion} score {score} is outside 1..5"
                    )
            rows.append((name, scores))
    return rows


def render(rows: list[tuple[str, dict[str, int]]]) -> str:
    lines = [
        "Scores are ordinal research judgments on a 1..5 scale.",
        "Weighted values are divided by the sum of weights (maximum 5.000).",
    ]
    for profile, weights in PROFILES.items():
        denominator = sum(weights.values())
        ranked = []
        for name, scores in rows:
            numerator = sum(scores[key] * weights[key] for key in CRITERIA)
            ranked.append((numerator / denominator, name))
        ranked.sort(key=lambda pair: (-pair[0], pair[1]))
        lines.append("")
        lines.append(f"{profile}:")
        lines.extend(
            f"  {place}. {name}: {value:.3f}"
            for place, (value, name) in enumerate(ranked, start=1)
        )
    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    report = render(load_scores())
    if args.check:
        expected = EXPECTED.read_text(encoding="utf-8")
        if report != expected:
            raise SystemExit("ranking-output.txt is stale; run without --check to inspect")
        print("ranking-output.txt matches validated scores")
    else:
        print(report, end="")


if __name__ == "__main__":
    main()
