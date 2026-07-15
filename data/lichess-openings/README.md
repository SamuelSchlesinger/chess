# Pinned Lichess opening corpus

`all.tsv` is the generated distribution of
[`lichess-org/chess-openings`](https://github.com/lichess-org/chess-openings)
at commit `292fd0468068f58bb244f7fe1c3e573e493c3c53`. It contains one header
and 3,803 data rows with columns:

```text
eco  name  pgn  uci  epd
```

The source columns are CC0. `COPYING.txt` is the upstream CC0 1.0 text.
`gen.py` is the exact upstream generator used for this pinned derivation.

## Reproduction

The pinned upstream workflow artifact was produced by run `29186614085`
(artifact `8258206920`) with CPython `3.10.20` and `chess==1.11.2`.
That artifact expires on 2026-10-10, which is why it is vendored here.

The committed output was also independently regenerated on 2026-07-14 with:

- upstream revision: `292fd0468068f58bb244f7fe1c3e573e493c3c53`;
- `gen.py` SHA-256:
  `323c90e60501f2ae55c7e28d76995036242421a921c67335d503c85d2fecf5e8`;
- Python `3.14.5` (the output was byte-identical across the Python versions);
- package `chess==1.11.2`;
- `chess-1.11.2.tar.gz` SHA-256:
  `a8b43e5678fdb3000695bdaa573117ad683761e5ca38e591c4826eba6d25bb39`.

The command, from a checkout of the pinned upstream revision, was:

```text
python bin/gen.py a.tsv b.tsv c.tsv d.tsv e.tsv > dist/all.tsv
```

The result is byte-for-byte identical to the pinned upstream workflow
artifact and has SHA-256:

```text
fd710d16bf5cdd750a565ee1a6aba19eb2c7db7d74d7df961f6e00fb1cd04a6e
```

The EPD field has four fields: placement, side to move, castling rights, and an
en-passant target only when an en-passant capture is legal. It therefore uses
the project's effective, not raw, en-passant convention. EPD does not contain
the halfmove clock, fullmove number, or `GameState` history.

## Lean validation

`lake exe chess_validate` validates all 3,803 rows and 36,840 plies. It checks
the exact schema and pinned aggregate counts, legally replays every UCI line
from `Initial.game`, resolves the corresponding SAN token in the same evolving
position, requires exact SAN/UCI move equality at every ply, and compares the
replayed endpoint with the supplied effective EPD. The replay reconstructs
clocks and complete prior-position history; the four-field EPD is never used as
a substitute for complete game state.
