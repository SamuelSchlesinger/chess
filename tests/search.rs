//! Engine validation: mate detection, PV legality/consistency, and tactical
//! solve rate on the downloaded Win-at-Chess suite (the search analogue of
//! perft for move generation).

use chess::eval::mate_in_moves;
use chess::{Board, Engine, Game, Limits};
use std::fs;

fn normalize_san(s: &str) -> String {
    s.replace('0', "O")
        .trim_end_matches(['+', '#', '!', '?'])
        .to_string()
}

#[test]
fn finds_mate_in_one() {
    // Black king g8 boxed by its own pawns; Ra8 is back-rank mate.
    let mut engine = Engine::new();
    let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R6K w - - 0 1").unwrap();
    let a = engine.analyze(&board, &Limits::depth(6));
    assert_eq!(board.san(a.best_move), "Ra8#", "score {}", a.score);
    assert_eq!(mate_in_moves(a.score), Some(1), "score {}", a.score);

    // Playing the PV must actually be checkmate.
    let mut g = Game::from_board(board);
    g.push(a.best_move);
    assert!(g.is_checkmate());
}

#[test]
fn finds_forced_mate_in_three() {
    // A known White-to-move forced mate in three (1.Na6+ ...). The engine must
    // report a 3-move mate, and playing out the principal variation must end in
    // checkmate.
    let mut engine = Engine::new();
    let board = Board::from_fen("1k5r/pP3ppp/3p2b1/1BN1n3/1Q2P3/P1B5/KP3P1P/7q w - - 1 0").unwrap();
    let a = engine.analyze(&board, &Limits::depth(10));
    assert_eq!(
        mate_in_moves(a.score),
        Some(3),
        "expected mate in 3, score {} bm {}",
        a.score,
        board.san(a.best_move)
    );

    let mut g = Game::from_board(board);
    for &mv in &a.pv {
        assert!(g.board().legal_moves().contains(mv), "illegal PV move {mv}");
        g.push(mv);
    }
    assert!(g.is_checkmate(), "PV did not end in checkmate: {:?}", a.pv);
}

#[test]
fn pv_is_a_legal_sequence() {
    let mut engine = Engine::new();
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    ] {
        let board = Board::from_fen(fen).unwrap();
        let a = engine.analyze(&board, &Limits::depth(8));
        let mut g = Game::from_board(board);
        for &mv in &a.pv {
            let legal = g.board().legal_moves();
            assert!(legal.contains(mv), "illegal PV move {mv} in {fen}");
            g.push(mv);
        }
        assert!(!a.pv.is_empty(), "no PV for {fen}");
    }
}

#[test]
fn search_is_deterministic() {
    let board = Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1").unwrap();
    let mut e1 = Engine::new();
    let mut e2 = Engine::new();
    let a1 = e1.analyze(&board, &Limits::depth(9));
    let a2 = e2.analyze(&board, &Limits::depth(9));
    assert_eq!(a1.best_move, a2.best_move);
    assert_eq!(a1.score, a2.score);
    assert_eq!(a1.nodes, a2.nodes, "node counts should match for equal searches");
}

#[test]
fn robustness_legal_best_move_over_many_positions() {
    // Search a broad set of real positions shallowly; the engine must never
    // panic and must always return a legal move (or none only when there are no
    // legal moves).
    let data = fs::read_to_string("tests/data/perft_vajolet.txt").unwrap();
    let mut engine = Engine::new();
    let mut checked = 0;
    for line in data.lines().step_by(13).take(400) {
        let fen = line.split(',').next().unwrap_or("").trim();
        let Ok(board) = Board::from_fen(fen) else {
            continue;
        };
        engine.new_game();
        let a = engine.analyze(&board, &Limits::depth(5));
        let legal = board.legal_moves();
        if !legal.is_empty() {
            assert!(
                legal.contains(a.best_move),
                "illegal best move {} in {fen}",
                a.best_move
            );
        }
        checked += 1;
    }
    assert!(checked > 100, "checked only {checked}");
}

#[test]
fn self_play_reaches_a_result() {
    // The engine plays both sides at a shallow depth: every move must be legal,
    // and the game must terminate (a result, or the ply cap) without panicking.
    use chess::Outcome;
    let mut game = Game::new();
    let mut engine = Engine::new();
    let mut plies = 0;
    while game.outcome() == Outcome::Ongoing && plies < 240 {
        let a = engine.analyze(game.board(), &Limits::depth(4));
        let legal = game.board().legal_moves();
        assert!(legal.contains(a.best_move), "illegal move at ply {plies}");
        game.push(a.best_move);
        plies += 1;
    }
    eprintln!("self-play ended after {plies} plies: {:?}", game.outcome());
    assert!(plies > 4, "game ended suspiciously fast");
}

/// Parse an EPD line into (fen, list-of-best-move-SANs).
fn parse_epd(line: &str) -> Option<(String, Vec<String>)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let bm_idx = line.find(" bm ")?;
    let fen = line[..bm_idx].trim().to_string();
    let rest = &line[bm_idx + 4..];
    let end = rest.find(';').unwrap_or(rest.len());
    let bms: Vec<String> = rest[..end]
        .split_whitespace()
        .map(normalize_san)
        .collect();
    Some((fen, bms))
}

fn solve_rate(path: &str, limits: &Limits, max: usize) -> (usize, usize) {
    let data = fs::read_to_string(path).unwrap_or_else(|_| panic!("read {path}"));
    let mut engine = Engine::new();
    let mut solved = 0;
    let mut total = 0;
    for line in data.lines().take(max) {
        let Some((fen, bms)) = parse_epd(line) else {
            continue;
        };
        let board = match Board::from_fen(&fen) {
            Ok(b) => b,
            Err(_) => continue,
        };
        engine.new_game();
        let a = engine.analyze(&board, limits);
        let got = normalize_san(&board.san(a.best_move));
        total += 1;
        if bms.iter().any(|b| b == &got) {
            solved += 1;
        }
    }
    (solved, total)
}

#[test]
fn wac_tactics_sample() {
    // A quick depth-limited pass over the first WAC positions.
    let (solved, total) = solve_rate("tests/data/epd/wac.epd", &Limits::depth(8), 30);
    eprintln!("WAC sample: solved {solved}/{total} at depth 8");
    assert!(
        solved * 100 >= total * 70,
        "expected >=70% on the WAC sample, got {solved}/{total}"
    );
}

#[test]
#[ignore = "full tactical suites; run with --ignored"]
fn wac_full_solve_rate() {
    let (solved, total) = solve_rate("tests/data/epd/wac.epd", &Limits::movetime(300), 1000);
    eprintln!("WAC full: solved {solved}/{total} at 300ms/move");
    assert!(solved * 100 >= total * 85, "WAC solve rate {solved}/{total}");
}
