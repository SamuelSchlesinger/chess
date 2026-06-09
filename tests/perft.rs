//! Move-generation correctness via perft, checked against downloaded suites:
//!  * `tests/data/perft_landmarks.txt` — the 6 canonical CPW positions.
//!  * `tests/data/perft_vajolet.txt`   — 6838 positions (FEN,d1..d6).

use chess::Board;
use std::fs;

/// Parse a `FEN,n1,n2,...` line into (fen, expected-per-depth).
fn parse_line(line: &str) -> Option<(String, Vec<u64>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut parts = line.split(',');
    let fen = parts.next()?.trim().to_string();
    let counts: Vec<u64> = parts.filter_map(|p| p.trim().parse().ok()).collect();
    Some((fen, counts))
}

#[test]
fn perft_landmarks_fast() {
    // Run each landmark up to the deepest level that stays cheap.
    const NODE_BUDGET: u64 = 5_000_000;
    let data = fs::read_to_string("tests/data/perft_landmarks.txt").unwrap();
    for line in data.lines() {
        let Some((fen, counts)) = parse_line(line) else {
            continue;
        };
        let board = Board::from_fen(&fen).unwrap_or_else(|e| panic!("bad fen {fen}: {e}"));
        for (i, &expected) in counts.iter().enumerate() {
            if expected > NODE_BUDGET {
                break;
            }
            let depth = (i + 1) as u32;
            let got = board.clone().perft(depth);
            assert_eq!(got, expected, "fen={fen} depth={depth}");
        }
    }
}

#[test]
fn perft_suite_depth3() {
    // Broad coverage: every position in the big suite, validated to depth 3
    // (cheap per position, ~6800 positions).
    let path = "tests/data/perft_vajolet.txt";
    let Ok(data) = fs::read_to_string(path) else {
        eprintln!("skipping: {path} not present");
        return;
    };
    let mut checked = 0usize;
    for line in data.lines() {
        let Some((fen, counts)) = parse_line(line) else {
            continue;
        };
        if counts.len() < 3 {
            continue;
        }
        let board = match Board::from_fen(&fen) {
            Ok(b) => b,
            Err(e) => panic!("bad fen {fen}: {e}"),
        };
        for depth in 1..=3u32 {
            let expected = counts[(depth - 1) as usize];
            let got = board.clone().perft(depth);
            assert_eq!(got, expected, "fen={fen} depth={depth}");
        }
        checked += 1;
    }
    eprintln!("perft_suite_depth3: validated {checked} positions to depth 3");
    assert!(checked > 1000, "expected the full suite, only saw {checked}");
}

#[test]
#[ignore = "deep perft; run with --ignored"]
fn perft_landmarks_deep() {
    let data = fs::read_to_string("tests/data/perft_landmarks.txt").unwrap();
    for line in data.lines() {
        let Some((fen, counts)) = parse_line(line) else {
            continue;
        };
        let board = Board::from_fen(&fen).unwrap();
        for (i, &expected) in counts.iter().enumerate() {
            let depth = (i + 1) as u32;
            let got = board.clone().perft(depth);
            assert_eq!(got, expected, "fen={fen} depth={depth}");
            eprintln!("{fen} depth {depth}: {got} ✓");
        }
    }
}

#[test]
#[ignore = "deep perft over the whole suite; run with --ignored"]
fn perft_suite_depth5() {
    let path = "tests/data/perft_vajolet.txt";
    let data = fs::read_to_string(path).unwrap();
    for line in data.lines() {
        let Some((fen, counts)) = parse_line(line) else {
            continue;
        };
        let board = Board::from_fen(&fen).unwrap();
        let depth = counts.len().min(5) as u32;
        let expected = counts[(depth - 1) as usize];
        let got = board.clone().perft(depth);
        assert_eq!(got, expected, "fen={fen} depth={depth}");
    }
}
