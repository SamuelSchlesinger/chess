# Test data provenance

These fixtures are factual oracles (node counts, hash keys, public game records)
used to validate the library. They are data, not code.

| File | Source | What it is |
|------|--------|------------|
| `perft_vajolet.txt` | Vajolet engine test suite | 6,838 positions as `FEN,d1..d6` perft node counts |
| `perft_landmarks.txt` | Chess Programming Wiki ("Perft Results") | the 6 canonical positions incl. Kiwipete |
| `zobrist_polyglot.txt` | Polyglot book format reference (`hardy.uhasselt.be/Toga/book_format.html`), via python-chess test vectors | 9 `FEN;key` Polyglot Zobrist references |
| `*.pgn` | Public game records, via python-chess `data/pgn/` | real games for SAN / full-game validation |

The Polyglot `Random64` constants embedded in `src/zobrist_table.rs` are the
standard, widely-reproduced Polyglot opening-book hashing table (verified by its
first entry `0x9d39247e33776d41` and the 9 reference keys above).
