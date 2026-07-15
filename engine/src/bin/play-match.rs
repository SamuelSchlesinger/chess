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

use chess::{
    Board, Engine, Game, HandcraftedEval, Limits, Mcts, Move, NnueEval, Outcome,
    PolicyValueNet, RepetitionKey,
};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const MAX_PLIES: u32 = 320;

type Player = Box<dyn FnMut(&Board, &[RepetitionKey]) -> Move>;

/// An external UCI engine (e.g. Stockfish) driven at a fixed node budget.
struct ExternalEngine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    nodes: u64,
}

impl ExternalEngine {
    fn spawn(path: &str, nodes: u64) -> ExternalEngine {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {path}: {e}"));
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut e = ExternalEngine {
            child,
            stdin,
            stdout,
            nodes,
        };
        e.send("uci");
        e.wait_for("uciok");
        e.send("setoption name Threads value 1");
        e.send("setoption name Hash value 16");
        e.send("isready");
        e.wait_for("readyok");
        e
    }

    fn send(&mut self, cmd: &str) {
        writeln!(self.stdin, "{cmd}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn wait_for(&mut self, token: &str) {
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }
            if line.trim_start().starts_with(token) {
                return;
            }
        }
    }

    /// Ask for the best move in `fen` at the fixed node budget (UCI long form).
    fn best_move(&mut self, fen: &str) -> Option<String> {
        self.send(&format!("position fen {fen}"));
        self.send(&format!("go nodes {}", self.nodes));
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).unwrap_or(0) == 0 {
                return None;
            }
            if let Some(rest) = line.trim_start().strip_prefix("bestmove ") {
                return rest.split_whitespace().next().map(|s| s.to_string());
            }
        }
    }
}

impl Drop for ExternalEngine {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.wait();
    }
}

fn make_player(
    engine: Option<&str>,
    net: Option<&str>,
    az: Option<&str>,
    az_eval: Option<&str>,
    mcts_sims: Option<u32>,
    nodes: u64,
) -> Player {
    let limits = Limits::nodes(nodes);
    let sims_az = mcts_sims.unwrap_or(400);
    if let Some(path) = az_eval {
        // The hybrid arm: alpha-beta search over the AZ net's value head.
        let eval = chess::eval::AzValueEval::load(path).unwrap_or_else(|e| panic!("load {path}: {e}"));
        let mut engine = Engine::with_eval_and_tt(eval, 16);
        return Box::new(move |board: &Board, hist: &[RepetitionKey]| {
            engine.set_history(hist);
            engine.analyze(board, &limits).best_move
        });
    }
    if let Some(path) = az {
        // Policy+value MCTS (the AlphaZero arm).
        let mut m = Mcts::new(PolicyValueNet::load(path).unwrap_or_else(|e| panic!("load {path}: {e}")));
        return Box::new(move |b: &Board, _h: &[RepetitionKey]| m.search(b, sims_az).0);
    }
    if let Some(sims) = mcts_sims {
        // PUCT MCTS (value-only spike) over the chosen evaluator.
        if let Some(path) = net {
            let mut m = Mcts::value(NnueEval::load(path).unwrap_or_else(|e| panic!("load {path}: {e}")));
            return Box::new(move |b: &Board, _h: &[RepetitionKey]| m.search(b, sims).0);
        }
        let mut m = Mcts::value(HandcraftedEval::new());
        return Box::new(move |b: &Board, _h: &[RepetitionKey]| m.search(b, sims).0);
    }
    if let Some(path) = engine {
        let mut ext = ExternalEngine::spawn(path, nodes);
        Box::new(move |board: &Board, _hist: &[RepetitionKey]| {
            let fen = board.to_fen();
            match ext.best_move(&fen) {
                Some(mv) => board.parse_uci(&mv).unwrap_or(Move::NONE),
                None => Move::NONE,
            }
        })
    } else if let Some(path) = net {
        // Quantized incremental NNUE — the production inference path.
        let eval = chess::eval::nnue::QNnueEval::load(path).unwrap_or_else(|e| panic!("load {path}: {e}"));
        let mut engine = Engine::with_eval_and_tt(eval, 16);
        Box::new(move |board: &Board, hist: &[RepetitionKey]| {
            engine.set_history(hist);
            engine.analyze(board, &limits).best_move
        })
    } else {
        let mut engine = Engine::new();
        engine.resize_tt(16);
        Box::new(move |board: &Board, hist: &[RepetitionKey]| {
            engine.set_history(hist);
            engine.analyze(board, &limits).best_move
        })
    }
}

fn label(engine: &Option<String>, net: &Option<String>, mcts: Option<u32>) -> String {
    let base = engine
        .clone()
        .or_else(|| net.clone())
        .unwrap_or_else(|| "PeSTO".to_string());
    if mcts.is_some() {
        format!("MCTS[{base}]")
    } else {
        base
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
    let label_a = cfg.az_eval_a.clone().map(|p| format!("AB[AZval:{p}]"))
        .or_else(|| cfg.az_a.clone().map(|p| format!("AZ[{p}]")))
        .unwrap_or_else(|| label(&cfg.engine_a, &cfg.net_a, cfg.mcts_a));
    let label_b = cfg.az_eval_b.clone().map(|p| format!("AB[AZval:{p}]"))
        .or_else(|| cfg.az_b.clone().map(|p| format!("AZ[{p}]")))
        .unwrap_or_else(|| label(&cfg.engine_b, &cfg.net_b, cfg.mcts_b));
    let budget = |mcts: Option<u32>, nodes: u64| match mcts {
        Some(s) => format!("{s} sims"),
        None => format!("{nodes} nodes"),
    };
    eprintln!(
        "play-match: {} games, A={} @ {} vs B={} @ {}, {} random opening plies",
        cfg.games,
        label_a,
        budget(cfg.mcts_a, cfg.nodes_a),
        label_b,
        budget(cfg.mcts_b, cfg.nodes_b),
        cfg.random_plies,
    );

    let mut a = make_player(cfg.engine_a.as_deref(), cfg.net_a.as_deref(), cfg.az_a.as_deref(), cfg.az_eval_a.as_deref(), cfg.mcts_a, cfg.nodes_a);
    let mut b = make_player(cfg.engine_b.as_deref(), cfg.net_b.as_deref(), cfg.az_b.as_deref(), cfg.az_eval_b.as_deref(), cfg.mcts_b, cfg.nodes_b);

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
        label_a,
        label_b,
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
    engine_a: Option<String>,
    engine_b: Option<String>,
    mcts_a: Option<u32>,
    mcts_b: Option<u32>,
    az_a: Option<String>,
    az_b: Option<String>,
    az_eval_a: Option<String>,
    az_eval_b: Option<String>,
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
        engine_a: None,
        engine_b: None,
        mcts_a: None,
        mcts_b: None,
        az_a: None,
        az_b: None,
        az_eval_a: None,
        az_eval_b: None,
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
            "--engine-a" => cfg.engine_a = Some(v.clone()),
            "--engine-b" => cfg.engine_b = Some(v.clone()),
            "--mcts-a" => cfg.mcts_a = v.parse().ok(),
            "--mcts-b" => cfg.mcts_b = v.parse().ok(),
            "--az-a" => cfg.az_a = Some(v.clone()),
            "--az-b" => cfg.az_b = Some(v.clone()),
            "--az-eval-a" => cfg.az_eval_a = Some(v.clone()),
            "--az-eval-b" => cfg.az_eval_b = Some(v.clone()),
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
