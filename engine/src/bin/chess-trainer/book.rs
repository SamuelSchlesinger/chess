//! A small embedded opening book: named main lines of the most common
//! openings, used to drive the *opponent* (the side the trainee is not
//! playing) along real theory. Stockfish, not the book, grades the trainee.
//!
//! Lines are authored in SAN and compiled to UCI once at startup. Matching is
//! by move-sequence prefix: the opponent always answers with the next move of
//! the earliest-listed line still consistent with the game, so play follows a
//! single coherent main line that narrows into a specific opening as the
//! trainee makes choices.

use chess::Game;
use std::sync::OnceLock;

/// (display name, SAN main line). Earlier entries are treated as "more main":
/// when several lines share the moves played so far, the opponent follows the
/// first one listed. Order them most-popular-first within each first move.
const LINES: &[(&str, &str)] = &[
    // 1.e4 e5
    ("Ruy Lopez", "e4 e5 Nf3 Nc6 Bb5 a6 Ba4 Nf6 O-O Be7 Re1 b5 Bb3 d6 c3 O-O h3 Na5 Bc2 c5 d4 Qc7"),
    ("Italian Game", "e4 e5 Nf3 Nc6 Bc4 Bc5 c3 Nf6 d3 d6 O-O O-O Re1 a6 a4 Ba7 h3 h6"),
    ("Scotch Game", "e4 e5 Nf3 Nc6 d4 exd4 Nxd4 Nf6 Nxc6 bxc6 e5 Qe7 Qe2 Nd5 c4 Ba6"),
    ("Petrov Defense", "e4 e5 Nf3 Nf6 Nxe5 d6 Nf3 Nxe4 d4 d5 Bd3 Nc6 O-O Be7 c4 Nb4"),
    // 1.e4 c5 — Sicilian
    ("Sicilian Najdorf", "e4 c5 Nf3 d6 d4 cxd4 Nxd4 Nf6 Nc3 a6 Be2 e5 Nb3 Be7 O-O O-O Be3 Be6"),
    ("Sicilian Sveshnikov", "e4 c5 Nf3 Nc6 d4 cxd4 Nxd4 Nf6 Nc3 e5 Ndb5 d6 Bg5 a6 Na3 b5 Nd5 Be7"),
    ("Sicilian Taimanov", "e4 c5 Nf3 e6 d4 cxd4 Nxd4 Nc6 Nc3 Qc7 Be2 a6 O-O Nf6 Be3 Bb4"),
    // other 1.e4
    ("French Defense", "e4 e6 d4 d5 Nc3 Nf6 Bg5 Be7 e5 Nfd7 Bxe7 Qxe7 f4 O-O Nf3 c5"),
    ("Caro-Kann Defense", "e4 c6 d4 d5 Nc3 dxe4 Nxe4 Bf5 Ng3 Bg6 h4 h6 Nf3 Nd7 h5 Bh7 Bd3 Bxd3"),
    ("Scandinavian Defense", "e4 d5 exd5 Qxd5 Nc3 Qa5 d4 Nf6 Nf3 c6 Bc4 Bf5 Bd2 e6"),
    ("Pirc Defense", "e4 d6 d4 Nf6 Nc3 g6 Nf3 Bg7 Be2 O-O O-O c6 a4 Nbd7"),
    ("Alekhine Defense", "e4 Nf6 e5 Nd5 d4 d6 Nf3 Bg4 Be2 e6 O-O Be7 c4 Nb6"),
    // 1.d4 d5 — Queen's Pawn / Gambit
    ("Queen's Gambit Declined", "d4 d5 c4 e6 Nc3 Nf6 Bg5 Be7 e3 O-O Nf3 h6 Bh4 b6 cxd5 Nxd5"),
    ("Slav Defense", "d4 d5 c4 c6 Nf3 Nf6 Nc3 dxc4 a4 Bf5 e3 e6 Bxc4 Bb4 O-O O-O"),
    ("Queen's Gambit Accepted", "d4 d5 c4 dxc4 Nf3 Nf6 e3 e6 Bxc4 c5 O-O a6 dxc5 Qxd1"),
    // 1.d4 Nf6 — Indian defenses
    ("King's Indian Defense", "d4 Nf6 c4 g6 Nc3 Bg7 e4 d6 Nf3 O-O Be2 e5 O-O Nc6 d5 Ne7"),
    ("Nimzo-Indian Defense", "d4 Nf6 c4 e6 Nc3 Bb4 e3 O-O Bd3 d5 Nf3 c5 O-O Nc6 a3 Bxc3"),
    ("Queen's Indian Defense", "d4 Nf6 c4 e6 Nf3 b6 g3 Ba6 b3 Bb4 Bd2 Be7 Bg2 c6 Nc3 d5"),
    ("Grunfeld Defense", "d4 Nf6 c4 g6 Nc3 d5 cxd5 Nxd5 e4 Nxc3 bxc3 Bg7 Nf3 c5 Rb1 O-O"),
    // 1.c4 / 1.Nf3 flank
    ("English Opening", "c4 e5 Nc3 Nf6 Nf3 Nc6 g3 d5 cxd5 Nxd5 Bg2 Nb6 O-O Be7 a3 O-O"),
    ("Reti Opening", "Nf3 d5 c4 c6 b3 Nf6 g3 Bf5 Bg2 e6 O-O Nbd7 Bb2 Bd6"),
];

pub struct Opening {
    pub name: String,
    pub ucis: Vec<String>,
}

pub struct BookReply {
    pub uci: String,
    pub name: String,
}

/// Compile the SAN lines into UCI sequences once. A malformed line is a
/// programming error in this file, so it panics loudly at startup.
pub fn openings() -> &'static [Opening] {
    static BOOK: OnceLock<Vec<Opening>> = OnceLock::new();
    BOOK.get_or_init(|| {
        LINES
            .iter()
            .map(|&(name, sans)| {
                let mut game = Game::new();
                let mut ucis = Vec::new();
                for tok in sans.split_whitespace() {
                    let san = tok.trim_end_matches(['+', '#', '!', '?']);
                    let mv = game.board().parse_san(san).unwrap_or_else(|| {
                        panic!("book line '{name}': bad SAN '{tok}' after {} plies", ucis.len())
                    });
                    ucis.push(mv.to_uci());
                    game.push(mv);
                }
                Opening { name: name.to_string(), ucis }
            })
            .collect()
    })
}

/// `prefix` is the leading subsequence of `seq` (or equal).
fn is_prefix(prefix: &[String], seq: &[String]) -> bool {
    seq.len() >= prefix.len() && seq[..prefix.len()] == *prefix
}

fn common_prefix_len(a: &[String], b: &[String]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// The opponent's reply for the line played so far: the next move of the
/// earliest-listed opening that the game still follows. `None` once the game
/// has left the book (no listed line continues these exact moves).
pub fn book_reply(moves: &[String]) -> Option<BookReply> {
    openings()
        .iter()
        .find(|o| o.ucis.len() > moves.len() && is_prefix(moves, &o.ucis))
        .map(|o| BookReply { uci: o.ucis[moves.len()].clone(), name: o.name.clone() })
}

/// Whether playing `played` after `moves` keeps the game inside some book line
/// (i.e. the trainee found a theory move).
pub fn is_book_line(moves: &[String], played: &str) -> bool {
    let mut seq = moves.to_vec();
    seq.push(played.to_string());
    openings().iter().any(|o| is_prefix(&seq, &o.ucis))
}

/// The name of the opening best matching the moves played (by longest common
/// prefix). Returns `None` until at least two moves identify something — and
/// keeps naming the opening even a move or two after the trainee leaves book.
pub fn opening_name(moves: &[String]) -> Option<String> {
    let mut best: Option<(usize, &Opening)> = None;
    for o in openings() {
        let cl = common_prefix_len(&o.ucis, moves);
        if best.is_none_or(|(b, _)| cl > b) {
            best = Some((cl, o));
        }
    }
    match best {
        Some((cl, o)) if cl >= 2 => Some(o.name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seq(ucis: &[&str]) -> Vec<String> {
        ucis.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn every_line_is_legal_san() {
        // Forces compilation of all lines; panics on a bad token.
        let book = openings();
        assert_eq!(book.len(), LINES.len());
        for o in book {
            assert!(o.ucis.len() >= 8, "line '{}' too short: {:?}", o.name, o.ucis);
        }
    }

    #[test]
    fn opponent_opens_with_main_line_first_move() {
        // With no moves, the opponent (as White) plays the first listed line's
        // opening move: Ruy Lopez -> 1.e4.
        let r = book_reply(&[]).unwrap();
        assert_eq!(r.uci, "e2e4");
    }

    #[test]
    fn opponent_follows_the_chosen_sicilian() {
        // 1.e4 c5 -> the opponent answers 2.Nf3 (main Sicilian move order).
        let r = book_reply(&seq(&["e2e4", "c7c5"])).unwrap();
        assert_eq!(r.uci, "g1f3");
        assert!(r.name.contains("Sicilian"));
    }

    #[test]
    fn book_line_detection() {
        assert!(is_book_line(&seq(&["e2e4"]), "e7e5")); // Ruy/Italian/...
        assert!(!is_book_line(&seq(&["e2e4"]), "h7h5")); // nonsense
    }

    #[test]
    fn out_of_book_has_no_reply() {
        // 1.a3 a6 — no line; opponent falls back to the engine.
        assert!(book_reply(&seq(&["a2a3", "a7a6"])).is_none());
    }

    #[test]
    fn opening_named_after_a_couple_moves() {
        assert_eq!(opening_name(&seq(&["e2e4"])), None); // too early to name
        let n = opening_name(&seq(&["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"])).unwrap();
        assert!(n.contains("Ruy"), "{n}");
    }
}
