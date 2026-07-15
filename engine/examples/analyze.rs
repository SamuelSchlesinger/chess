//! Library-API demo: analyze a position and print the result.
//!
//!   cargo run --release --example analyze                 # start position
//!   cargo run --release --example analyze "<FEN>"         # a given position
//!   cargo run --release --example analyze "<FEN>" 1500    # ... for 1500 ms

use chess::eval::mate_in_moves;
use chess::{Board, Engine, Limits};

fn main() {
    let mut args = std::env::args().skip(1);
    let fen = args.next();
    let ms: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1000);

    let board = match fen.as_deref() {
        None | Some("startpos") => Board::startpos(),
        Some(f) => Board::from_fen(f).expect("valid FEN"),
    };

    println!("{board}\n");

    let mut engine = Engine::new();
    let analysis = engine.analyze_with(&board, &Limits::movetime(ms), |info| {
        let score = match mate_in_moves(info.score) {
            Some(m) => format!("#{m}"),
            None => format!("{:+.2}", info.score as f64 / 100.0),
        };
        let pv: Vec<String> = info.pv.iter().map(|m| m.to_uci()).collect();
        println!(
            "depth {:2}  {:>7}  {:>10} nodes  {:>6} knps  {}",
            info.depth,
            score,
            info.nodes,
            info.nps / 1000,
            pv.join(" ")
        );
    });

    let best_san = board.san(analysis.best_move);
    let score = match mate_in_moves(analysis.score) {
        Some(m) => format!("mate in {m}"),
        None => format!("{:+.2}", analysis.score as f64 / 100.0),
    };
    println!(
        "\nbest move: {best_san} ({})   eval: {score}   {} nodes in {} ms",
        analysis.best_move.to_uci(),
        analysis.nodes,
        analysis.time_ms,
    );
}
