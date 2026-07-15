//! End-to-end validation on real games (public PGN datasets from python-chess):
//! parse each game's SAN, play every move, assert legality, round-trip the SAN
//! through our own generator, and confirm checkmate where the PGN marks `#`.

use chess::{Color, Game, Outcome};
use std::collections::HashMap;
use std::fs;

struct PgnGame {
    headers: HashMap<String, String>,
    sans: Vec<String>,
}

/// Remove `{comments}` and `(variations)` (handles nesting).
fn strip_comments(s: &str) -> String {
    let mut out = String::new();
    let mut brace = 0i32;
    let mut paren = 0i32;
    for c in s.chars() {
        match c {
            '{' => brace += 1,
            '}' => brace = (brace - 1).max(0),
            '(' => paren += 1,
            ')' => paren = (paren - 1).max(0),
            _ if brace == 0 && paren == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Turn a movetext token into a SAN move, or `None` for move numbers / results.
fn clean_token(tok: &str) -> Option<String> {
    let t = tok.trim();
    if t.is_empty() || t.starts_with('$') {
        return None;
    }
    if matches!(t, "1-0" | "0-1" | "1/2-1/2" | "*") {
        return None;
    }
    // Strip a leading move number like "12." / "12..." possibly glued to a move.
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 {
        let mut j = i;
        while j < bytes.len() && bytes[j] == b'.' {
            j += 1;
        }
        if j > i {
            let rest = &t[j..];
            return if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };
        }
    }
    Some(t.to_string())
}

fn parse_pgn(content: &str) -> Vec<PgnGame> {
    let mut games = Vec::new();
    let mut headers = HashMap::new();
    let mut movetext = String::new();
    let mut in_moves = false;

    let flush = |headers: &mut HashMap<String, String>, movetext: &mut String, games: &mut Vec<PgnGame>| {
        if headers.is_empty() && movetext.trim().is_empty() {
            return;
        }
        let cleaned = strip_comments(movetext);
        let sans: Vec<String> = cleaned.split_whitespace().filter_map(clean_token).collect();
        games.push(PgnGame {
            headers: std::mem::take(headers),
            sans,
        });
        movetext.clear();
    };

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            // A header after move text means a new game began.
            if in_moves {
                flush(&mut headers, &mut movetext, &mut games);
                in_moves = false;
            }
            if let Some(rest) = line.strip_prefix('[') {
                let rest = rest.trim_end_matches(']');
                if let Some((key, val)) = rest.split_once(' ') {
                    headers.insert(key.to_string(), val.trim().trim_matches('"').to_string());
                }
            }
        } else if !line.is_empty() {
            in_moves = true;
            movetext.push_str(line);
            movetext.push(' ');
        }
    }
    flush(&mut headers, &mut movetext, &mut games);
    games
}

fn play_file(path: &str) -> usize {
    let content = fs::read_to_string(path).unwrap_or_else(|_| panic!("read {path}"));
    let games = parse_pgn(&content);
    assert!(!games.is_empty(), "no games parsed from {path}");
    let mut total_moves = 0;

    for game in &games {
        let mut g = if let Some(fen) = game.headers.get("FEN") {
            Game::from_fen(fen).unwrap_or_else(|e| panic!("bad setup FEN in {path}: {e}"))
        } else {
            Game::new()
        };

        for (i, san) in game.sans.iter().enumerate() {
            // Null-move markers (SCID "Z0", "--", "@@@@") aren't real moves;
            // stop validating this game at that point.
            if matches!(san.as_str(), "Z0" | "--" | "@@@@") {
                break;
            }
            let board_fen = g.board().to_fen();
            let mv = g.board().parse_san(san).unwrap_or_else(|| {
                panic!("illegal/unparsable SAN '{san}' (move {}) in {path} at {board_fen}", i + 1)
            });

            // Our generator must reproduce the same SAN (round-trip).
            let regenerated = g.board().san(mv);
            assert_eq!(
                normalize(&regenerated),
                normalize(san),
                "SAN round-trip mismatch in {path}: pgn='{san}' ours='{regenerated}' at {board_fen}"
            );

            let last = i + 1 == game.sans.len();
            let mover = g.side_to_move();
            g.push(mv);
            total_moves += 1;

            // If the PGN marks checkmate, our outcome must agree.
            if san.ends_with('#') {
                assert!(last, "'#' should be the final move in {path}");
                match g.outcome() {
                    Outcome::Checkmate { winner } => assert_eq!(
                        winner, mover,
                        "checkmate winner mismatch in {path}"
                    ),
                    other => panic!("expected checkmate after {san} in {path}, got {other:?}"),
                }
            } else if san.ends_with('+') {
                assert!(g.board().in_check(), "'+' but not in check in {path} after {san}");
            }
        }
    }
    let _ = Color::White;
    total_moves
}

fn normalize(san: &str) -> String {
    san.replace('0', "O")
        .trim_end_matches(['+', '#', '!', '?'])
        .to_string()
}

#[test]
fn play_public_pgn_games() {
    let files = [
        "tests/data/molinari-bordais-1979.pgn",
        "tests/data/kasparov-deep-blue-1997.pgn",
        "tests/data/nepomniachtchi-liren-game1.pgn",
        "tests/data/anastasian-lewis.pgn",
    ];
    let mut total = 0;
    for f in files {
        if std::path::Path::new(f).exists() {
            total += play_file(f);
        }
    }
    eprintln!("played {total} half-moves across public PGN games");
    assert!(total > 100, "expected to play a meaningful number of moves");
}
