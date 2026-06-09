//! AlphaZero-style self-play game generator.
//!
//! Plays games with policy-guided MCTS using a [`PolicyValueNet`] (or a random
//! net for generation 0), recording for every position the **MCTS visit policy**
//! (the improved policy target) and, once the game ends, the **outcome** (the
//! value target). Early moves are sampled from the visit distribution for
//! exploration; later moves are the most-visited (argmax).
//!
//! Output: variable-length little-endian records (concatenate shards):
//! ```text
//!   [packed 34][result_white i8][n u8] then n × ([move_index u16][visits u16])
//! ```
//! `result_white` is the game result from White's perspective (+1/0/-1).

use chess::{Game, Mcts, Outcome, PolicyValueNet};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_PLIES: u32 = 240;
const TEMP_PLIES: u32 = 24; // sample (explore) before this ply, argmax after
const ADJ_PLIES: u32 = 6;
const ADJ_VALUE: f32 = 0.92; // |MCTS root value| above this for ADJ_PLIES -> adjudicate

struct Cfg {
    games: u64,
    out: String,
    net: Option<String>,
    sims: u32,
    threads: usize,
    seed: u64,
    save_net: Option<String>,
}

fn main() {
    let cfg = parse();
    let net = Arc::new(match &cfg.net {
        Some(p) => PolicyValueNet::load(p).unwrap_or_else(|e| panic!("load {p}: {e}")),
        None => PolicyValueNet::random(cfg.seed),
    });
    if let Some(p) = &cfg.save_net {
        if let Some(d) = std::path::Path::new(p).parent() {
            let _ = std::fs::create_dir_all(d);
        }
        net.save(p).expect("save net");
        eprintln!("saved generation net to {p}");
    }
    eprintln!(
        "selfplay: {} games, {} sims/move, net={}, {} threads",
        cfg.games,
        cfg.sims,
        cfg.net.as_deref().unwrap_or("random"),
        cfg.threads
    );
    if let Some(d) = std::path::Path::new(&cfg.out).parent() {
        let _ = std::fs::create_dir_all(d);
    }

    let cfg = Arc::new(cfg);
    let next = Arc::new(AtomicU64::new(0));
    let positions = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();
    let mut handles = Vec::new();
    for t in 0..cfg.threads {
        let (cfg, net, next, positions, done) =
            (cfg.clone(), net.clone(), next.clone(), positions.clone(), done.clone());
        handles.push(std::thread::spawn(move || {
            use std::io::Write;
            let mut mcts = Mcts::new(net);
            let mut out = std::io::BufWriter::new(
                std::fs::File::create(format!("{}.part{}", cfg.out, t)).unwrap(),
            );
            let mut buf = Vec::new();
            loop {
                let g = next.fetch_add(1, Ordering::Relaxed);
                if g >= cfg.games {
                    break;
                }
                let n = play(&mut mcts, splitmix(cfg.seed, g), cfg.sims, &mut buf);
                out.write_all(&buf).unwrap();
                buf.clear();
                positions.fetch_add(n as u64, Ordering::Relaxed);
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if t == 0 && d.is_multiple_of(50) {
                    let s = start.elapsed().as_secs_f64();
                    eprintln!(
                        "  {d} games, {} positions, {:.1} games/s",
                        positions.load(Ordering::Relaxed),
                        d as f64 / s
                    );
                }
            }
            out.flush().unwrap();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    eprintln!(
        "done: {} games, {} positions in {:.0}s",
        done.load(Ordering::Relaxed),
        positions.load(Ordering::Relaxed),
        start.elapsed().as_secs_f64()
    );
}

/// (packed position bytes, MCTS visit policy as (move_index, visits) pairs).
type Recorded = ([u8; 34], Vec<(u16, u16)>);

fn play(mcts: &mut Mcts<Arc<PolicyValueNet>>, mut rng: u64, sims: u32, out: &mut Vec<u8>) -> usize {
    let mut game = Game::new();
    let mut recorded: Vec<Recorded> = Vec::new();
    let mut result: i8 = 0;
    let mut adj = 0u32;
    let mut adj_sign = 0i8;
    let mut ply = 0u32;

    loop {
        match game.outcome() {
            Outcome::Checkmate { winner } => {
                result = if winner == chess::Color::White { 1 } else { -1 };
                break;
            }
            Outcome::Stalemate | Outcome::Draw(_) => break,
            Outcome::Ongoing => {}
        }
        if ply >= MAX_PLIES {
            break;
        }

        let board = game.board().clone();
        let (_best, dist) = mcts.search(&board, sims);
        let total: u32 = dist.iter().map(|&(_, n)| n).sum();
        if total == 0 {
            break;
        }

        // Record the position + visit policy.
        let policy: Vec<(u16, u16)> = dist
            .iter()
            .map(|&(mv, n)| (chess::eval::policyvalue::move_index(mv) as u16, n as u16))
            .collect();
        recorded.push((board.pack().bytes, policy));

        let stm_white = board.side_to_move() == chess::Color::White;

        // Pick the move: sample early (explore), argmax late.
        let mv = if ply < TEMP_PLIES {
            sample(&dist, &mut rng)
        } else {
            dist.iter().max_by_key(|&&(_, n)| n).map(|&(m, _)| m).unwrap()
        };

        // Crude adjudication: if one move dominates strongly for several plies.
        let dom = dist.iter().map(|&(_, n)| n).max().unwrap() as f32 / total as f32;
        let sign = if dom > ADJ_VALUE {
            if stm_white { 1 } else { -1 }
        } else {
            0
        };
        if sign != 0 && sign == adj_sign {
            adj += 1;
            if adj >= ADJ_PLIES {
                result = adj_sign;
                break;
            }
        } else {
            adj_sign = sign;
            adj = if sign != 0 { 1 } else { 0 };
        }

        game.push(mv);
        ply += 1;
    }

    for (packed, policy) in &recorded {
        out.extend_from_slice(packed);
        out.push(result as u8);
        out.push(policy.len().min(255) as u8);
        for &(mi, n) in policy.iter().take(255) {
            out.extend_from_slice(&mi.to_le_bytes());
            out.extend_from_slice(&n.to_le_bytes());
        }
    }
    recorded.len()
}

/// Sample a move proportional to visit counts.
fn sample(dist: &[(chess::Move, u32)], rng: &mut u64) -> chess::Move {
    let total: u32 = dist.iter().map(|&(_, n)| n).sum();
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    let mut r = (*rng % total.max(1) as u64) as u32;
    for &(mv, n) in dist {
        if r < n {
            return mv;
        }
        r -= n;
    }
    dist[0].0
}

fn splitmix(seed: u64, i: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_mul(i.wrapping_add(1));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) | 1
}

fn parse() -> Cfg {
    let mut c = Cfg {
        games: 2000,
        out: "data/sp_az".into(),
        net: None,
        sims: 200,
        threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
        seed: 1,
        save_net: None,
    };
    let a: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < a.len() {
        let v = &a[i + 1];
        match a[i].as_str() {
            "--games" => c.games = v.parse().unwrap_or(c.games),
            "--out" => c.out = v.clone(),
            "--net" => c.net = Some(v.clone()),
            "--sims" => c.sims = v.parse().unwrap_or(c.sims),
            "--threads" => c.threads = v.parse().unwrap_or(c.threads),
            "--seed" => c.seed = v.parse().unwrap_or(c.seed),
            "--save-net" => c.save_net = Some(v.clone()),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    c
}
