//! Fast perft NPS harness for optimization iteration.
//!
//!   cargo run --release --example nps
//!   RUSTFLAGS="-C target-cpu=native" cargo run --release --example nps
//!
//! Reports nodes/sec per position and an aggregate, so a change can be judged in
//! seconds without a full criterion run. Warms the magic tables first.

use chess::Board;
use std::time::Instant;

fn main() {
    chess::magic::init();
    let cases = [
        ("startpos", "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 6u32),
        ("kiwipete", "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1", 5),
        ("position3", "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1", 6),
        ("midgame", "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10", 5),
    ];
    // Optional repeat count for steadier numbers.
    let reps: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    let mut total_nodes = 0u64;
    let mut total_secs = 0.0;
    for (name, fen, depth) in cases {
        let mut best = f64::INFINITY;
        let mut nodes = 0;
        for _ in 0..reps {
            let mut b = Board::from_fen(fen).unwrap();
            let t = Instant::now();
            nodes = b.perft(depth);
            best = best.min(t.elapsed().as_secs_f64());
        }
        let mnps = nodes as f64 / 1e6 / best;
        println!("{name:10} d{depth}: {nodes:>12} nodes  {best:7.4}s  {mnps:8.1} Mnps");
        total_nodes += nodes;
        total_secs += best;
    }
    println!(
        "{:-<10} aggregate: {total_nodes:>12} nodes  {total_secs:7.4}s  {:8.1} Mnps",
        "",
        total_nodes as f64 / 1e6 / total_secs
    );
}
