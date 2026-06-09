//! Fixed-node engine-vs-engine match harness with an Elo estimate.
//!
//! The honest measurement tool: it pits two engine configurations at a fixed
//! **node budget per move** (isolating evaluation/search *quality* from raw
//! nodes/sec), over many games with random openings and balanced colors, and
//! reports W-D-L and an Elo difference with an error bar.
//!
//! Each side is either the handcrafted PeSTO eval (default) or a trained NNUE
//! net (`--net-a PATH` / `--net-b PATH`), so once a net exists this measures W1
//! (net vs PeSTO). With both sides PeSTO it self-validates: a higher node budget
//! must score a clearly positive Elo.
//!
//! Usage:
//!   cargo run --release --bin play-match -- --games 200 --nodes-a 5000 \
//!       --nodes-b 5000 --net-a nets/v1.nnue --random-plies 8 --seed 1

use chess::{Board, Engine, Game, Limits, Move, NnueEval, Outcome};

const MAX_PLIES: u32 = 320;

type Player = Box<dyn FnMut(&Board, &[u64]) -> Move>;

fn make_player(net: Option<&str>, nodes: u64) -> Player {
    let limits = Limits::nodes(nodes);
    match net {
        Some(path) => {
            let eval = NnueEval::load(path).unwrap_or_else(|e| panic!("load {path}: {e}"));
            let mut engine = Engine::with_eval_and_tt(eval, 16);
            Box::new(move |board: &Board, hist: &[u64]| {
                engine.set_history(hist);
                engine.analyze(board, &limits).best_move
            })
        }
        None => {
            let mut engine = Engine::new();
            engine.resize_tt(16);
            Box::new(move |board: &Board, hist: &[u64]| {
                engine.set_history(hist);
                engine.analyze(board, &limits).best_move
            })
        }
    }
}

/// Result of one game from side A's perspective.
#[derive(Clone, Copy)]
enum GameResult {
    AWin,
    Draw,
    BWin,
}

fn play_game(a: &mut Player, b: &mut Player, a_is_white: bool, opening: &[Move]) -> GameResult {
    let mut game = Game::new();
    for &mv in opening {
        game.push(mv);
    }
    let mut plies = 0u32;
    let winner = loop {
        match game.outcome() {
            Outcome::Checkmate { winner } => break Some(winner),
            Outcome::Stalemate | Outcome::Draw(_) => break None,
            Outcome::Ongoing => {}
        }
        if plies >= MAX_PLIES {
            break None;
        }
        let stm = game.side_to_move();
        let a_to_move = (stm == chess::Color::White) == a_is_white;
        let keys = game.position_keys();
        let hist = &keys[..keys.len().saturating_sub(1)];
        let board = game.board().clone();
        let mv = if a_to_move {
            a(&board, hist)
        } else {
            b(&board, hist)
        };
        if mv.is_none() || !game.board().legal_moves().contains(mv) {
            // A losing/illegal move ends the game against the offender.
            return if a_to_move {
                GameResult::BWin
            } else {
                GameResult::AWin
            };
        }
        game.push(mv);
        plies += 1;
    };
    match winner {
        Some(c) => {
            let a_won = (c == chess::Color::White) == a_is_white;
            if a_won {
                GameResult::AWin
            } else {
                GameResult::BWin
            }
        }
        None => GameResult::Draw,
    }
}

/// A random opening line of `plies` legal moves (deterministic from `seed`).
fn random_opening(plies: u32, mut rng: u64) -> Vec<Move> {
    let mut game = Game::new();
    let mut out = Vec::new();
    for _ in 0..plies {
        let moves = game.board().legal_moves();
        if moves.is_empty() {
            break;
        }
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let mv = moves[(rng % moves.len() as u64) as usize];
        out.push(mv);
        game.push(mv);
        if game.outcome() != Outcome::Ongoing {
            out.pop();
            break;
        }
    }
    out
}

fn main() {
    let cfg = parse_args();
    eprintln!(
        "play-match: {} games, A={} @ {} nodes vs B={} @ {} nodes, {} random opening plies",
        cfg.games,
        cfg.net_a.as_deref().unwrap_or("PeSTO"),
        cfg.nodes_a,
        cfg.net_b.as_deref().unwrap_or("PeSTO"),
        cfg.nodes_b,
        cfg.random_plies,
    );

    let mut a = make_player(cfg.net_a.as_deref(), cfg.nodes_a);
    let mut b = make_player(cfg.net_b.as_deref(), cfg.nodes_b);

    let (mut w, mut d, mut l) = (0u32, 0u32, 0u32);
    let start = std::time::Instant::now();
    for g in 0..cfg.games {
        // Each opening is played twice with colors swapped (a paired game),
        // which cancels first-move and opening imbalance.
        let opening = random_opening(cfg.random_plies, splitmix(cfg.seed, g / 2));
        let a_is_white = g % 2 == 0;
        match play_game(&mut a, &mut b, a_is_white, &opening) {
            GameResult::AWin => w += 1,
            GameResult::Draw => d += 1,
            GameResult::BWin => l += 1,
        }
        if (g + 1) % 20 == 0 {
            eprintln!(
                "  {}/{}  +{w} ={d} -{l}  {}",
                g + 1,
                cfg.games,
                elo_line(w, d, l)
            );
        }
    }

    let secs = start.elapsed().as_secs_f64();
    println!(
        "\nA (={}) vs B (={}): +{w} ={d} -{l}  over {} games in {:.1}s",
        cfg.net_a.as_deref().unwrap_or("PeSTO"),
        cfg.net_b.as_deref().unwrap_or("PeSTO"),
        w + d + l,
        secs
    );
    println!("{}", elo_line(w, d, l));
}

/// Elo difference (A − B) with a 95% error bar, from W/D/L.
fn elo_line(w: u32, d: u32, l: u32) -> String {
    let n = (w + d + l) as f64;
    if n == 0.0 {
        return "Elo n/a".to_string();
    }
    let score = (w as f64 + 0.5 * d as f64) / n;
    let elo = |s: f64| -400.0 * (1.0 / s - 1.0).log10();
    // Standard deviation of the score, then a 95% (1.96σ) band on Elo.
    let w_r = w as f64 / n;
    let l_r = l as f64 / n;
    let dev = ((w_r * (1.0 - score).powi(2)
        + (d as f64 / n) * (0.5 - score).powi(2)
        + l_r * (0.0 - score).powi(2))
        / n)
        .sqrt();
    let lo = (score - 1.96 * dev).clamp(1e-6, 1.0 - 1e-6);
    let hi = (score + 1.96 * dev).clamp(1e-6, 1.0 - 1e-6);
    if score <= 0.0 || score >= 1.0 {
        return format!("score {:.1}%  Elo ±∞", score * 100.0);
    }
    format!(
        "score {:.1}%  Elo {:+.0}  [{:+.0}, {:+.0}]",
        score * 100.0,
        elo(score),
        elo(lo),
        elo(hi),
    )
}

fn splitmix(seed: u64, index: u64) -> u64 {
    let mut z = seed
        .wrapping_add(0x9E37_79B9_7F4A_7C15)
        .wrapping_mul(index.wrapping_add(1));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) | 1
}

struct Config {
    games: u64,
    nodes_a: u64,
    nodes_b: u64,
    net_a: Option<String>,
    net_b: Option<String>,
    random_plies: u32,
    seed: u64,
}

fn parse_args() -> Config {
    let mut cfg = Config {
        games: 100,
        nodes_a: 5000,
        nodes_b: 5000,
        net_a: None,
        net_b: None,
        random_plies: 8,
        seed: 1,
    };
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < args.len() {
        let v = &args[i + 1];
        match args[i].as_str() {
            "--games" => cfg.games = v.parse().unwrap_or(cfg.games),
            "--nodes-a" => cfg.nodes_a = v.parse().unwrap_or(cfg.nodes_a),
            "--nodes-b" => cfg.nodes_b = v.parse().unwrap_or(cfg.nodes_b),
            "--net-a" => cfg.net_a = Some(v.clone()),
            "--net-b" => cfg.net_b = Some(v.clone()),
            "--random-plies" => cfg.random_plies = v.parse().unwrap_or(cfg.random_plies),
            "--seed" => cfg.seed = v.parse().unwrap_or(cfg.seed),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    cfg
}
