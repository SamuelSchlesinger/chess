//! A UCI client for driving external engines (Stockfish, lc0, ...) behind the
//! same SSE schema as the built-in engine. One persistent process per engine
//! keeps its hash warm across requests; stdin is shared behind a mutex so the
//! client-disconnect watcher can inject `stop` while the request thread is
//! blocked reading the engine's stdout.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

pub struct UciEngine {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: BufReader<ChildStdout>,
}

/// One parsed `info ... pv ...` line.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct UciInfo {
    pub depth: i32,
    pub seldepth: i32,
    pub multipv: usize,
    /// Score from the engine's (side-to-move) perspective.
    pub cp: Option<i32>,
    pub mate: Option<i32>,
    pub nodes: u64,
    pub nps: u64,
    pub time_ms: u64,
    pub pv: Vec<String>,
}

/// What a `go` should be bounded by (all optional = infinite).
pub struct GoParams {
    pub multipv: usize,
    pub movetime: u64, // 0 = none
    pub depth: i32,    // 0 = none
}

impl UciEngine {
    pub fn spawn(cmd: &[String], hash_mb: usize) -> Result<UciEngine, String> {
        let (exe, args) = cmd.split_first().ok_or("empty engine command")?;
        let mut child = Command::new(exe)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn '{exe}': {e}"))?;
        let stdin = Arc::new(Mutex::new(child.stdin.take().ok_or("no stdin")?));
        let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        let mut e = UciEngine { child, stdin, stdout };
        e.send("uci")?;
        e.wait_for("uciok")?;
        e.send(&format!("setoption name Hash value {hash_mb}"))?;
        e.sync()?;
        Ok(e)
    }

    /// A handle the disconnect watcher can use to `stop` a running search.
    pub fn stdin_handle(&self) -> Arc<Mutex<ChildStdin>> {
        self.stdin.clone()
    }

    pub fn send(&mut self, cmd: &str) -> Result<(), String> {
        send_line(&self.stdin, cmd)
    }

    /// `isready` round-trip; also drains any stale output.
    pub fn sync(&mut self) -> Result<(), String> {
        self.send("isready")?;
        self.wait_for("readyok")
    }

    fn wait_for(&mut self, token: &str) -> Result<(), String> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err(format!("engine exited waiting for '{token}'"));
            }
            if line.trim_start().starts_with(token) {
                return Ok(());
            }
        }
    }

    /// Run one search and stream parsed PV infos to `on_info`. The callback
    /// returns `false` to stop the search (e.g. the SSE write failed). Blocks
    /// until the engine answers `bestmove`; an external `stop` injected via
    /// [`UciEngine::stdin_handle`] also resolves through here.
    pub fn search(
        &mut self,
        position: &str,
        go: &GoParams,
        mut on_info: impl FnMut(&UciInfo) -> bool,
    ) -> Result<Option<String>, String> {
        self.send(&format!("setoption name MultiPV value {}", go.multipv))?;
        self.send(position)?;
        let mut cmd = String::from("go");
        if go.depth > 0 {
            cmd += &format!(" depth {}", go.depth);
        }
        if go.movetime > 0 {
            cmd += &format!(" movetime {}", go.movetime);
        }
        if go.depth == 0 && go.movetime == 0 {
            cmd += " infinite";
        }
        self.send(&cmd)?;

        let mut stopped = false;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                return Err("engine exited mid-search".to_string());
            }
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("bestmove") {
                return Ok(rest.split_whitespace().next().map(|s| s.to_string()));
            }
            if let Some(info) = parse_info(t)
                && !stopped
                && !on_info(&info)
            {
                stopped = true;
                self.send("stop")?;
            }
        }
    }
}

impl Drop for UciEngine {
    fn drop(&mut self) {
        let _ = send_line(&self.stdin, "quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn send_line(stdin: &Arc<Mutex<ChildStdin>>, cmd: &str) -> Result<(), String> {
    let mut s = stdin.lock().map_err(|_| "stdin poisoned")?;
    writeln!(s, "{cmd}").and_then(|_| s.flush()).map_err(|e| e.to_string())
}

/// Parse an `info` line carrying a PV. Returns `None` for periodic
/// (`currmove`, string, pv-less) lines and aspiration `lowerbound` /
/// `upperbound` reports, which would make the displayed score jump around.
pub fn parse_info(line: &str) -> Option<UciInfo> {
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("info") {
        return None;
    }
    let mut info = UciInfo { multipv: 1, ..Default::default() };
    let mut has_pv = false;
    while let Some(tok) = tokens.next() {
        match tok {
            "depth" => info.depth = tokens.next()?.parse().ok()?,
            "seldepth" => info.seldepth = tokens.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "multipv" => info.multipv = tokens.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "score" => match tokens.next()? {
                "cp" => info.cp = tokens.next().and_then(|v| v.parse().ok()),
                "mate" => info.mate = tokens.next().and_then(|v| v.parse().ok()),
                _ => return None,
            },
            "lowerbound" | "upperbound" => return None,
            "nodes" => info.nodes = tokens.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "nps" => info.nps = tokens.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "time" => info.time_ms = tokens.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "string" => return None,
            "pv" => {
                info.pv = tokens.by_ref().map(|s| s.to_string()).collect();
                has_pv = true;
            }
            _ => {}
        }
    }
    (has_pv && (info.cp.is_some() || info.mate.is_some())).then_some(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stockfish_info_lines() {
        let i = parse_info(
            "info depth 20 seldepth 28 multipv 2 score cp 35 nodes 1234567 nps 2500000 \
             hashfull 250 tbhits 0 time 494 pv e2e4 e7e5 g1f3",
        )
        .unwrap();
        assert_eq!(i.depth, 20);
        assert_eq!(i.multipv, 2);
        assert_eq!(i.cp, Some(35));
        assert_eq!(i.mate, None);
        assert_eq!(i.nodes, 1_234_567);
        assert_eq!(i.time_ms, 494);
        assert_eq!(i.pv, vec!["e2e4", "e7e5", "g1f3"]);

        let m = parse_info("info depth 12 score mate -3 nodes 5 nps 5 time 1 pv h7h8").unwrap();
        assert_eq!(m.mate, Some(-3));
        assert_eq!(m.cp, None);
    }

    #[test]
    fn skips_non_pv_and_bound_lines() {
        assert_eq!(parse_info("info depth 5 currmove e2e4 currmovenumber 1"), None);
        assert_eq!(
            parse_info("info depth 9 score cp 50 lowerbound nodes 100 time 2 pv e2e4"),
            None
        );
        assert_eq!(parse_info("info string NNUE evaluation using nn.nnue"), None);
        assert_eq!(parse_info("bestmove e2e4"), None);
    }

    /// Protocol round-trip against a real engine, skipped when none is
    /// installed.
    #[test]
    fn drives_real_stockfish_if_present() {
        let cmd = vec!["stockfish".to_string()];
        let mut e = match UciEngine::spawn(&cmd, 16) {
            Ok(e) => e,
            Err(_) => {
                eprintln!("stockfish not installed; skipping");
                return;
            }
        };
        let mut infos = 0;
        let best = e
            .search(
                "position startpos moves e2e4",
                &GoParams { multipv: 2, movetime: 0, depth: 10 },
                |i| {
                    infos += 1;
                    assert!(!i.pv.is_empty());
                    true
                },
            )
            .unwrap();
        assert!(infos >= 2, "expected multipv info lines, got {infos}");
        let best = best.expect("bestmove");
        let board = chess::Board::startpos();
        let board2 = {
            let mut b = board.clone();
            b.make_move(b.parse_uci("e2e4").unwrap());
            b
        };
        assert!(board2.parse_uci(&best).is_some(), "illegal bestmove {best}");

        // A second search on the same process must resync cleanly.
        e.sync().unwrap();
        let best2 = e
            .search("position startpos", &GoParams { multipv: 1, movetime: 200, depth: 0 }, |_| true)
            .unwrap();
        assert!(best2.is_some());
    }
}
