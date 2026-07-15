#!/usr/bin/env python3
"""Check crude labeled-placement ceilings used in the classification note.

The count for ``k`` distinct labeled pieces is ``2 * P(64, k)``: placements
on distinct squares, times side to move.  It deliberately ignores legality,
symmetry, clocks, castling rights, and en-passant state, so it is an upper bound
only for the stripped-down position families described in the note.
"""

import argparse
from math import perm
from pathlib import Path


EXPECTED = {
    3: 499_968,
    4: 30_498_048,
    5: 1_829_882_880,
}
OUTPUT = Path(__file__).resolve().parent / "state-space-output.txt"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    actual = {pieces: 2 * perm(64, pieces) for pieces in EXPECTED}
    if actual != EXPECTED:
        raise SystemExit(f"unexpected arithmetic: {actual}")
    report = "".join(
        f"{pieces} labeled pieces plus side to move: {positions:,}\n"
        for pieces, positions in actual.items()
    )
    if args.check:
        if report != OUTPUT.read_text(encoding="utf-8"):
            raise SystemExit("state-space-output.txt is stale")
        print("state-space-output.txt matches validated bounds")
    else:
        print(report, end="")


if __name__ == "__main__":
    main()
