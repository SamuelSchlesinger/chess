//! Re-label positions with Stockfish evaluations (strong distillation targets).
//!
//! Reads the 37-byte records produced by `gen-data` (packed position + our
//! eval + WDL), evaluates each position with Stockfish at a fixed node budget
//! across parallel SF processes, and writes records with the **Stockfish** eval
//! (centipawns, White's perspective) replacing ours, keeping the game WDL:
//! `[packed 34][i16 sf_cp white][i8 wdl]`. A net distilling these gets a static
//! eval far stronger than our handcrafted one — the path past W1.
//!
//! Usage:
//!   cargo run --release --bin label-sf -- --in data/sp --out data/sp_sf \
//!       --sf /opt/homebrew/bin/stockfish --nodes 25000 --threads 12

use chess::Packed;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const RECORD_LEN: usize = 37;
const MATE_CP: i32 = 10_000;
const SCORE_CLAMP: i32 = 10_000;

struct Sf {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    nodes: u64,
}

impl Sf {
    fn spawn(path: &str, nodes: u64) -> Sf {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {path}: {e}"));
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut sf = Sf { child, stdin, stdout, nodes };
        sf.cmd("uci");
        sf.wait("uciok");
        sf.cmd("setoption name Threads value 1");
        sf.cmd("setoption name Hash value 16");
        sf.cmd("isready");
        sf.wait("readyok");
        sf
    }

    fn cmd(&mut self, c: &str) {
        writeln!(self.stdin, "{c}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn wait(&mut self, token: &str) {
        let mut line = String::new();
        while self.stdout.read_line(&mut line).unwrap_or(0) > 0 {
            if line.trim_start().starts_with(token) {
                return;
            }
            line.clear();
        }
    }

    /// Stockfish eval for `fen` in centipawns, White's perspective.
    fn eval_white(&mut self, fen: &str, stm_white: bool) -> Option<i16> {
        self.cmd(&format!("position fen {fen}"));
        self.cmd(&format!("go nodes {}", self.nodes));
        let mut last: Option<i32> = None;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).unwrap_or(0) == 0 {
                return None;
            }
            let t = line.trim_start();
            if let Some(cp) = parse_score(t) {
                last = Some(cp);
            }
            if t.starts_with("bestmove") {
                break;
            }
        }
        let cp_stm = last?;
        let cp_white = if stm_white { cp_stm } else { -cp_stm };
        Some(cp_white.clamp(-SCORE_CLAMP, SCORE_CLAMP) as i16)
    }
}

impl Drop for Sf {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let _ = self.child.wait();
    }
}

/// Parse `... score cp N ...` or `... score mate N ...` from an info line.
fn parse_score(line: &str) -> Option<i32> {
    let idx = line.find("score ")?;
    let mut it = line[idx + 6..].split_whitespace();
    match it.next()? {
        "cp" => it.next()?.parse::<i32>().ok(),
        "mate" => {
            let n: i32 = it.next()?.parse().ok()?;
            Some(if n >= 0 { MATE_CP - n } else { -MATE_CP - n })
        }
        _ => None,
    }
}

fn main() {
    let cfg = parse_args();
    let records = load_records(&cfg.input);
    if records.is_empty() {
        eprintln!("no records under {}*", cfg.input);
        std::process::exit(1);
    }
    eprintln!(
        "label-sf: {} positions, SF @ {} nodes, {} threads",
        records.len(),
        cfg.nodes,
        cfg.threads
    );
    if let Some(dir) = std::path::Path::new(&cfg.output).parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let records = Arc::new(records);
    let cursor = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicU64::new(0));
    let start = std::time::Instant::now();
    let chunk = 256u64;

    let mut handles = Vec::new();
    for t in 0..cfg.threads {
        let records = records.clone();
        let cursor = cursor.clone();
        let done = done.clone();
        let cfg = cfg.clone();
        let start = start;
        handles.push(std::thread::spawn(move || {
            let mut sf = Sf::spawn(&cfg.sf, cfg.nodes);
            let path = format!("{}.part{}", cfg.output, t);
            let mut out = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
            loop {
                let begin = cursor.fetch_add(chunk, Ordering::Relaxed);
                if begin >= records.len() as u64 {
                    break;
                }
                let end = (begin + chunk).min(records.len() as u64);
                for i in begin..end {
                    let rec = &records[i as usize];
                    let mut bytes = [0u8; 34];
                    bytes.copy_from_slice(&rec[0..34]);
                    let packed = Packed { bytes };
                    let stm_white = packed.side_to_move() == chess::Color::White;
                    let fen = packed.unpack().to_fen();
                    if let Some(cp) = sf.eval_white(&fen, stm_white) {
                        let mut o = *rec;
                        o[34..36].copy_from_slice(&cp.to_le_bytes());
                        out.write_all(&o).unwrap();
                    }
                }
                let n = done.fetch_add(end - begin, Ordering::Relaxed) + (end - begin);
                if t == 0 {
                    let s = start.elapsed().as_secs_f64();
                    eprintln!(
                        "  {n}/{} labeled, {:.0} pos/s, eta {:.0}s",
                        records.len(),
                        n as f64 / s,
                        (records.len() as f64 - n as f64) / (n as f64 / s).max(1.0)
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
        "done: {} positions in {:.0}s. Shards: {}.part0..{}",
        done.load(Ordering::Relaxed),
        start.elapsed().as_secs_f64(),
        cfg.output,
        cfg.threads - 1
    );
}

fn load_records(prefix: &str) -> Vec<[u8; RECORD_LEN]> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    if std::path::Path::new(prefix).is_file() {
        paths.push(prefix.into());
    } else if let Some(dir) = std::path::Path::new(prefix).parent() {
        let stem = std::path::Path::new(prefix)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().starts_with(&stem) {
                    paths.push(e.path());
                }
            }
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        if let Ok(bytes) = std::fs::read(&p) {
            for r in bytes.chunks_exact(RECORD_LEN) {
                let mut a = [0u8; RECORD_LEN];
                a.copy_from_slice(r);
                out.push(a);
            }
        }
    }
    out
}

#[derive(Clone)]
struct Config {
    input: String,
    output: String,
    sf: String,
    nodes: u64,
    threads: usize,
}

fn parse_args() -> Config {
    let mut cfg = Config {
        input: "data/sp".into(),
        output: "data/sp_sf".into(),
        sf: "/opt/homebrew/bin/stockfish".into(),
        nodes: 25000,
        threads: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
    };
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < args.len() {
        let v = &args[i + 1];
        match args[i].as_str() {
            "--in" => cfg.input = v.clone(),
            "--out" => cfg.output = v.clone(),
            "--sf" => cfg.sf = v.clone(),
            "--nodes" => cfg.nodes = v.parse().unwrap_or(cfg.nodes),
            "--threads" => cfg.threads = v.parse().unwrap_or(cfg.threads),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    cfg
}
