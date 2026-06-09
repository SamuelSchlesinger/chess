//! Self-play training-data generator.
//!
//! Plays games with the engine at a fixed node budget, starting from a short
//! random opening for diversity, and records *quiet* positions (not in check,
//! best move not a capture/promotion) labeled with both the search evaluation
//! (centipawns, White's perspective) and the eventual game result (WDL). These
//! two targets are what a value/NNUE net trains on.
//!
//! Output: one or more shard files of fixed-size 37-byte little-endian records:
//! ```text
//!   bytes  0..34   Packed position (34-byte canonical form)
//!   bytes 34..36   i16 score   (centipawns, White's perspective, clamped)
//!   byte     36    i8  result  (+1 White win, 0 draw, -1 Black win)
//! ```
//! Concatenate the shards for the full dataset; record count = filesize / 37.
//!
//! Usage:
//!   cargo run --release --bin gen-data -- --games 100000 --out data/selfplay \
//!       --nodes 5000 --threads 12 --seed 1 --random-plies 8
//!
//! It is deterministic given (seed, games, threads): game `g` uses seed
//! `splitmix(seed, g)`, so datasets are reproducible.

use chess::{Engine, Game, Limits, Outcome};
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

const RECORD_LEN: usize = 37;
const SCORE_CLAMP: i32 = 10_000;
/// Adjudicate a win once |eval| exceeds this for several consecutive plies.
const ADJUDICATE_CP: i32 = 2500;
const ADJUDICATE_PLIES: u32 = 4;
const MAX_PLIES: u32 = 320;

struct Config {
    games: u64,
    out: String,
    nodes: u64,
    threads: usize,
    seed: u64,
    random_plies: u32,
}

fn main() {
    let cfg = parse_args();
    eprintln!(
        "gen-data: {} games, {} nodes/move, {} threads, seed {}, {} random opening plies",
        cfg.games, cfg.nodes, cfg.threads, cfg.seed, cfg.random_plies
    );
    if let Some(dir) = std::path::Path::new(&cfg.out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let cfg = Arc::new(cfg);
    let next_game = Arc::new(AtomicU64::new(0));
    let total_positions = Arc::new(AtomicU64::new(0));
    let total_games = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();

    let mut handles = Vec::new();
    for t in 0..cfg.threads {
        let cfg = cfg.clone();
        let next_game = next_game.clone();
        let total_positions = total_positions.clone();
        let total_games = total_games.clone();
        handles.push(std::thread::spawn(move || {
            let path = format!("{}.part{}", cfg.out, t);
            let file = std::fs::File::create(&path).expect("create shard");
            let mut writer = BufWriter::with_capacity(1 << 20, file);
            let mut engine = Engine::new();
            engine.resize_tt(16);
            let mut buf = Vec::new();

            loop {
                let g = next_game.fetch_add(1, Ordering::Relaxed);
                if g >= cfg.games {
                    break;
                }
                let n = play_game(&mut engine, splitmix(cfg.seed, g), &cfg, &mut buf);
                for rec in buf.drain(..) {
                    writer.write_all(&rec).expect("write record");
                }
                total_positions.fetch_add(n as u64, Ordering::Relaxed);
                let done = total_games.fetch_add(1, Ordering::Relaxed) + 1;
                if t == 0 && done.is_multiple_of(200) {
                    let secs = start.elapsed().as_secs_f64();
                    let pos = total_positions.load(Ordering::Relaxed);
                    eprintln!(
                        "  {done} games, {pos} positions, {:.0} pos/s, {:.1} games/s",
                        pos as f64 / secs,
                        done as f64 / secs
                    );
                }
            }
            writer.flush().expect("flush");
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let secs = start.elapsed().as_secs_f64();
    let pos = total_positions.load(Ordering::Relaxed);
    eprintln!(
        "done: {} games, {} positions in {:.1}s ({:.0} pos/s). Shards: {}.part0..{}",
        total_games.load(Ordering::Relaxed),
        pos,
        secs,
        pos as f64 / secs,
        cfg.out,
        cfg.threads - 1
    );
}

/// Play one self-play game; append its quiet-position records to `out`.
/// Returns the number of positions recorded.
fn play_game(engine: &mut Engine, mut rng: u64, cfg: &Config, out: &mut Vec<[u8; RECORD_LEN]>) -> usize {
    engine.new_game();
    let mut game = Game::new();

    // Random opening for diversity.
    for _ in 0..cfg.random_plies {
        let moves = game.board().legal_moves();
        if moves.is_empty() {
            return 0;
        }
        let pick = (next_rand(&mut rng) % moves.len() as u64) as usize;
        game.push(moves[pick]);
    }
    if game.outcome() != Outcome::Ongoing {
        return 0;
    }

    // (packed-position, white-perspective-score) for quiet positions.
    let mut pending: Vec<(chess::Packed, i16)> = Vec::new();
    let mut adj_count = 0u32;
    let mut adj_winner = 0i8;
    let result: i8; // White's perspective: +1/0/-1
    let limits = Limits::nodes(cfg.nodes);

    let mut ply = 0u32;
    loop {
        match game.outcome() {
            Outcome::Checkmate { winner } => {
                result = if winner == chess::Color::White { 1 } else { -1 };
                break;
            }
            Outcome::Stalemate | Outcome::Draw(_) => {
                result = 0;
                break;
            }
            Outcome::Ongoing => {}
        }
        if ply >= MAX_PLIES {
            result = 0;
            break;
        }

        let board = game.board();
        let keys = game.position_keys();
        engine.set_history(&keys[..keys.len().saturating_sub(1)]);
        let analysis = engine.analyze(board, &limits);

        // White-perspective score.
        let stm = board.side_to_move();
        let mut score = analysis.score;
        if stm == chess::Color::Black {
            score = -score;
        }
        let score = score.clamp(-SCORE_CLAMP, SCORE_CLAMP);

        // Record only quiet positions: not in check, best move not a capture or
        // promotion (those carry tactical noise the static eval shouldn't model).
        let quiet = !board.in_check()
            && !analysis.best_move.is_capture()
            && !analysis.best_move.is_promotion();
        if quiet {
            pending.push((board.pack(), score as i16));
        }

        // Adjudication: a decisive, stable eval ends the game early.
        let winner_sign = if score > ADJUDICATE_CP {
            1
        } else if score < -ADJUDICATE_CP {
            -1
        } else {
            0
        };
        if winner_sign != 0 && winner_sign as i8 == adj_winner {
            adj_count += 1;
            if adj_count >= ADJUDICATE_PLIES {
                result = adj_winner;
                break;
            }
        } else {
            adj_winner = winner_sign as i8;
            adj_count = if winner_sign != 0 { 1 } else { 0 };
        }

        game.push(analysis.best_move);
        ply += 1;
    }

    for (packed, score) in &pending {
        let mut rec = [0u8; RECORD_LEN];
        rec[0..34].copy_from_slice(&packed.bytes);
        rec[34..36].copy_from_slice(&score.to_le_bytes());
        rec[36] = result as u8;
        out.push(rec);
    }
    pending.len()
}

// --- tiny deterministic PRNG ---

fn splitmix(seed: u64, index: u64) -> u64 {
    let mut z = seed
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_mul(index.wrapping_add(1));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn next_rand(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn parse_args() -> Config {
    let mut cfg = Config {
        games: 1000,
        out: "data/selfplay".to_string(),
        nodes: 5000,
        threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
        seed: 1,
        random_plies: 8,
    };
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        let val = args.get(i + 1);
        let parsed = |c: &mut u64| {
            if let Some(v) = val.and_then(|v| v.parse().ok()) {
                *c = v;
            }
        };
        match args[i].as_str() {
            "--games" => parsed(&mut cfg.games),
            "--nodes" => parsed(&mut cfg.nodes),
            "--seed" => parsed(&mut cfg.seed),
            "--out" => {
                if let Some(v) = val {
                    cfg.out = v.clone();
                }
            }
            "--threads" => {
                if let Some(v) = val.and_then(|v| v.parse().ok()) {
                    cfg.threads = v;
                }
            }
            "--random-plies" => {
                if let Some(v) = val.and_then(|v| v.parse().ok()) {
                    cfg.random_plies = v;
                }
            }
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    cfg
}
