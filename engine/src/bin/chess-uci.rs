//! A UCI (Universal Chess Interface) front-end for the engine.
//!
//! Speaks enough of the protocol to be driven by any chess GUI or by
//! `cutechess-cli` for engine-vs-engine play and tactical testing:
//! `uci`, `isready`, `ucinewgame`, `setoption name Hash value N`, `position`,
//! `go` (depth / movetime / wtime+btime / nodes / infinite), `stop`, `quit`.
//!
//! The search runs on a background thread so `stop` can interrupt it; the engine
//! is moved into the thread and handed back when the search finishes.

use chess::eval::mate_in_moves;
use chess::{Analysis, Board, Engine, Limits, SearchInfo};
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

fn main() {
    let stdin = io::stdin();
    let mut engine: Option<Engine> = Some(Engine::new());
    let mut search: Option<JoinHandle<Engine>> = None;
    let stop = engine.as_ref().unwrap().stop_handle();

    // Current position and the Zobrist keys of all positions preceding it.
    let mut board = Board::startpos();
    let mut history: Vec<u64> = Vec::new();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        let mut parts = line.split_whitespace();
        let Some(cmd) = parts.next() else { continue };

        match cmd {
            "uci" => {
                println!("id name chess");
                println!("id author chess crate");
                println!("option name Hash type spin default 16 min 1 max 4096");
                println!("uciok");
            }
            "isready" => {
                println!("readyok");
            }
            "ucinewgame" => {
                join(&mut search, &mut engine, &stop);
                if let Some(e) = engine.as_mut() {
                    e.new_game();
                }
            }
            "setoption" => {
                // setoption name <Name> value <Value>
                let rest = line.to_lowercase();
                if rest.contains("name hash")
                    && let Some(v) = parse_after(&rest, "value")
                {
                    join(&mut search, &mut engine, &stop);
                    if let Some(e) = engine.as_mut() {
                        e.resize_tt(v as usize);
                    }
                }
            }
            "position" => {
                join(&mut search, &mut engine, &stop);
                (board, history) = parse_position(line);
            }
            "go" => {
                join(&mut search, &mut engine, &stop);
                let limits = parse_go(line);
                let mut e = engine.take().expect("engine available");
                e.set_history(&history);
                stop.store(false, Ordering::Relaxed);
                let pos = board.clone();
                search = Some(thread::spawn(move || {
                    let analysis = e.analyze_with(&pos, &limits, print_info);
                    print_bestmove(&analysis);
                    e
                }));
            }
            "stop" => {
                stop.store(true, Ordering::Relaxed);
            }
            "quit" => {
                stop.store(true, Ordering::Relaxed);
                join(&mut search, &mut engine, &stop);
                break;
            }
            _ => {}
        }
    }
}

/// Stop and join a running search thread, restoring the engine. Signalling stop
/// first is essential: an unbounded `go infinite` search never returns on its
/// own, so joining without it would deadlock the whole UCI loop.
fn join(
    search: &mut Option<JoinHandle<Engine>>,
    engine: &mut Option<Engine>,
    stop: &Arc<AtomicBool>,
) {
    if let Some(handle) = search.take() {
        stop.store(true, Ordering::Relaxed);
        if let Ok(e) = handle.join() {
            *engine = Some(e);
        }
    }
}

fn print_info(info: &SearchInfo) {
    let score = format_score(info.score);
    let pv: Vec<String> = info.pv.iter().map(|m| m.to_uci()).collect();
    println!(
        "info depth {} seldepth {} score {} nodes {} nps {} time {} hashfull {} pv {}",
        info.depth,
        info.seldepth,
        score,
        info.nodes,
        info.nps,
        info.time_ms,
        info.hashfull,
        pv.join(" ")
    );
    let _ = io::stdout().flush();
}

fn print_bestmove(a: &Analysis) {
    match a.ponder {
        Some(p) => println!("bestmove {} ponder {}", a.best_move.to_uci(), p.to_uci()),
        None => println!("bestmove {}", a.best_move.to_uci()),
    }
    let _ = io::stdout().flush();
}

fn format_score(score: i32) -> String {
    match mate_in_moves(score) {
        Some(m) => format!("mate {m}"),
        None => format!("cp {score}"),
    }
}

fn parse_after(s: &str, key: &str) -> Option<u64> {
    let mut it = s.split_whitespace();
    while let Some(t) = it.next() {
        if t == key {
            return it.next().and_then(|v| v.parse().ok());
        }
    }
    None
}

/// Parse `position [startpos | fen <fen>] [moves ...]` into a board + history.
fn parse_position(line: &str) -> (Board, Vec<u64>) {
    let mut board = Board::startpos();
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1;
    if i < tokens.len() {
        if tokens[i] == "startpos" {
            i += 1;
        } else if tokens[i] == "fen" {
            // The FEN is the next up-to-6 tokens (until "moves").
            let start = i + 1;
            let mut end = start;
            while end < tokens.len() && tokens[end] != "moves" {
                end += 1;
            }
            let fen = tokens[start..end].join(" ");
            if let Ok(b) = Board::from_fen(&fen) {
                board = b;
            }
            i = end;
        }
    }

    let mut history = Vec::new();
    if i < tokens.len() && tokens[i] == "moves" {
        i += 1;
        for &mv in &tokens[i..] {
            if let Some(m) = board.parse_uci(mv) {
                history.push(board.hash());
                board.make_move(m);
            }
        }
    }
    (board, history)
}

fn parse_go(line: &str) -> Limits {
    let mut limits = Limits::default();
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1;
    while i < tokens.len() {
        let val = tokens.get(i + 1);
        match tokens[i] {
            "depth" => limits.depth = val.and_then(|v| v.parse().ok()),
            "nodes" => limits.nodes = val.and_then(|v| v.parse().ok()),
            "movetime" => limits.movetime = val.and_then(|v| v.parse().ok()),
            "wtime" => limits.wtime = val.and_then(|v| v.parse().ok()),
            "btime" => limits.btime = val.and_then(|v| v.parse().ok()),
            "winc" => limits.winc = val.and_then(|v| v.parse().ok()),
            "binc" => limits.binc = val.and_then(|v| v.parse().ok()),
            "movestogo" => limits.movestogo = val.and_then(|v| v.parse().ok()),
            "infinite" => limits.infinite = true,
            _ => {}
        }
        i += 1;
    }
    limits
}
