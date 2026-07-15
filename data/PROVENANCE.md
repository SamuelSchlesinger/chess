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
project's stated FIDE semantics: history-sensitive repetition, castling through
check, legal versus pinned en passant, promotion, the phase potential, and
exact versus repetition-only opening transpositions. Their expected fields
were recomputed by this validator and independently inspected; they do not have
an external empirical provenance.

## Opening-data expansion

For a larger named-opening layer, the intended upstream is
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

Those raw upstream files are not silently folded into the hand-authored
fixtures here. A future derived corpus should pin the Python and
`python-chess` versions, generator hash, output hash, and the generator's
`legal` en-passant convention.
