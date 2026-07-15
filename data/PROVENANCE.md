# Validation corpus provenance

These small TSV files are executable regression fixtures, not a claim that a
position or opening database has been formally verified. Every FEN has all six
standard fields. Move lists use lowercase UCI coordinate notation, and every
trace starts from a freshly parsed `GameState` with `prior := []`.

## Source identifiers

### `stockfish18-standard-suite`

Expected perft values are outputs pinned by the official Stockfish perft test
suite at Stockfish tag `sf_18`, commit
`cb3d4ee9b47d0c5aae855b12379378ea1439675c`. The source script is
<https://raw.githubusercontent.com/official-stockfish/Stockfish/cb3d4ee9b47d0c5aae855b12379378ea1439675c/tests/perft.sh>
and its SHA-256 is
`eb197e6d33e0b6b85592d4686103a113f61125a204f1f5a1786de8c7eb457f71`.
Stockfish is GPL-3.0; this repository records factual input/output vectors and
does not copy the script. Four-field suite positions were normalized by
appending the neutral counters `0 1`. Counts were checked locally with
Stockfish 18 on 2026-07-14.

### `local-stockfish18`

Small hand-selected positions evaluated locally with the same Stockfish 18
binary on 2026-07-14. These supplement, rather than purport to be part of, the
official standard suite.

### `pgn-fen-1994`

FEN semantics and the position after `1. e4` follow section 16 of the original
PGN/FEN specification dated 1994-03-12:
<https://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm>. SHA-256 of
the retrieved archival HTML is
`2c2445a8c2118a5603610364f8055b31db388e2f4cbc6bb70815bf38ee45de3f`.
Standard FEN records the raw `e3` target after `e2e4`; the separate effective
FEN column deliberately normalizes it to `-` because no legal capture exists.

### `uci-2006`

UCI spelling follows the original April 2006 protocol text at
<https://backscattering.de/chess/uci/2006-04.txt>, retrieved with SHA-256
`6740651e8b1a6f0020e1d9451dc4269ed76277b4fe0f954b387a31c6fb17925c`.
The protocol's `0000` null sentinel is not treated as a chess move.

### `lean-game-examples`, `lean-opening-theory`, `handcrafted-rules`

These are deliberately small, hand-authored fixtures that exercise the
project’s stated FIDE semantics: history-sensitive repetition, castling through
check, legal versus pinned en passant, promotion, the phase potential, and
exact versus repetition-only opening transpositions. Their expected fields
were recomputed by this validator and independently inspected; they do not have
an external empirical provenance.

The repetition identity follows FIDE Laws of Chess Article 9.2.3, in the
official English Laws approved 2022-08-07 and applied from 2023-01-01:
<https://handbook.fide.com/chapter/E012023>. In particular, the Article makes
legal en-passant availability and retained castling rights part of deciding
whether two occurrences are the same position.

`position_ids.tsv` is the cross-language identity contract. Its nine rows
exercise clock irrelevance, castling-right relevance, raw-but-ineffective
en-passant targets after `e2e4`, a pinned en-passant capture, and a genuinely
legal en-passant capture. Lean checks the rows with
`FEN.renderEffectiveEPD`; Rust checks the same bytes with
`Board::position_id`. Both the Lean trace validator and the Rust suite also
replay this legal history from the standard initial position:

```text
1.d4 e5 2.dxe5 Nf6 3.e4 Nxe4 4.Nf3 Nc5 5.Nc3 g6
6.Bf4 Bg7 7.Qd2 O-O 8.h3 Re8 9.a3 d5
10.Ng1 Nbd7 11.Nf3 Nb8 12.Ng1 Nbd7 13.Nf3 Nb8
```

After `9...d5`, `e5xd6 e.p.` would expose White's king on e1 to the rook
on e8, so it is illegal. The initial raw target and the two later occurrences
therefore make three equal FIDE positions, while the legality-insensitive
Polyglot convention assigns only the two later occurrences the same `u64`.
The regression asserts exact count three and simulated legacy count two.

## Pinned named-opening corpus

The vendored named-opening layer comes from
<https://github.com/lichess-org/chess-openings> at commit
`292fd0468068f58bb244f7fe1c3e573e493c3c53`, released under CC0. The pinned
`COPYING.txt` SHA-256 is
`a2010f343487d3f7618affe54f789f5487602331c0a8d03f49e9a7c547cf0499`.
Its five source TSV files contain 3,803 data rows; their respective SHA-256
hashes are:

- `a.tsv`: `41722fa3d44f294357326fe2ca1b956d9e56490b30efcfa68db61114c9df7e10`
- `b.tsv`: `28d5c2dfc3329d70e85be2a149d001a59e47c2176c9d2c6594eb3be88128a3fc`
- `c.tsv`: `e90f063b3a04f5fbb24425682b13f574141a266a1ba877974cdd9c6595a3d942`
- `d.tsv`: `58cad40b886bd499717eabcce281d4bfcf00eeadbdc00552f42042cf4aac50d2`
- `e.tsv`: `f1f8494f488f660e284f23527d5acfbeccdbbc3acc76e74f05d125f39d2f8a74`

The repository vendors the upstream generated `dist/all.tsv`, rather than
silently folding these rows into the hand-authored fixtures. The exact output
contains one header plus 3,803 rows (803,019 bytes) and has SHA-256
`fd710d16bf5cdd750a565ee1a6aba19eb2c7db7d74d7df961f6e00fb1cd04a6e`.
The generator is vendored with SHA-256
`323c90e60501f2ae55c7e28d76995036242421a921c67335d503c85d2fecf5e8`.

The pinned upstream workflow run `29186614085`, artifact `8258206920`, used
CPython 3.10.20 and `chess==1.11.2`. The artifact was independently regenerated
with Python 3.14.5 and the same chess package, producing byte-identical output.
The `chess-1.11.2.tar.gz` source distribution has SHA-256
`a8b43e5678fdb3000695bdaa573117ad683761e5ca38e591c4826eba6d25bb39`.
The generator uses python-chess's `legal` en-passant convention, matching this
project's effective EPD rendering. Full reproduction notes and the expiring
upstream-artifact identifier are in `data/lichess-openings/README.md`.

At validation time Lean checks the exact schema and aggregate counts, requires
all PGN/UCI/EPD fields to be unique, legally replays all 36,840 UCI plies from
the standard initial `GameState`, resolves each SAN token in lockstep, requires
SAN and UCI equality at every ply, and compares every endpoint with the four
effective EPD fields. Opening names are intentionally not treated as unique or
as position invariants.

The separate Lean opening-graph pass analyzes every prefix occurrence with the
proved exact `RepetitionKey`. It hard-checks 40,643 row-prefix occurrences,
8,646 distinct move histories, 7,848 repetition nodes, 570 non-singleton
fibres, excess 798, maximum fibre size eight, three depth-varying nodes, 7,921
raw-en-passant keys, and the complete non-singleton multiplicity distribution.
The derivation, concrete transpositions, label statistics, and independently
computed edge/SCC results are documented in
`data/lichess-openings/ANALYSIS.md`. The read-only reproducer
`data/lichess-openings/analyze.py` requires exactly `chess==1.11.2`, verifies
the pinned input hash, and hard-checks every empirical count in that report.
