//! A local web GUI for the analysis engine.
//!
//!   cargo run --release --bin chess-web            # http://127.0.0.1:8000/
//!   cargo run --release --bin chess-web -- --port 9090 --hash 256 \
//!       --nnue nets/v2.nnue --uci "stockfish" --uci "lc0=lc0 --threads=2"
//!
//! Std-only, like the rest of the crate: a hand-rolled HTTP/1.1 server drives
//! the library engine directly and streams analysis to an embedded
//! single-page frontend over Server-Sent Events.
//!
//! Several engines can be offered side by side (selectable in the UI):
//!  - `pesto`     — the built-in engine with the handcrafted PeSTO eval;
//!  - `--nnue F`  — the built-in search with a trained quantized NNUE net;
//!  - `--uci CMD` — any external UCI engine, spawned as a subprocess (with
//!    native MultiPV). A `stockfish` on PATH is registered automatically.
//!
//! Endpoints:
//!   GET  /                  the app (embedded index.html / app.js / style.css)
//!   GET  /api/engines       the engine registry
//!   GET  /api/state         position, legal moves, SANs, outcome
//!   GET  /api/analyze       SSE: live engine lines (MultiPV via root exclusion)
//!   GET  /api/evalseries    SSE: one eval per ply of the game (for the graph)
//!   POST /api/pgn           import: PGN text -> start FEN + UCI move list
//!
//! Positions are passed statelessly as `fen` (or "startpos") + `moves` (UCI,
//! space-separated) + `at` (view index); the browser holds the game state.

mod http;
mod uci;

use chess::eval::nnue::QNnueEval;
use chess::eval::{MAX_PLY, is_mate, mate_in_moves};
use chess::{
    Analysis, Board, Color, Engine, Game, HandcraftedEval, Limits, Move, Outcome, RepetitionKey,
    SearchInfo,
};
use http::{
    Request, jarr, jopt, jstr, read_request, respond, respond_bad_request, respond_json,
    sse_event, start_sse,
};
use std::io::{BufReader, Read};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use uci::{GoParams, UciEngine, UciInfo};

const INDEX_HTML: &str = include_str!("index.html");
const APP_JS: &str = include_str!("app.js");
const STYLE_CSS: &str = include_str!("style.css");

// --- engine registry ---

#[derive(Clone)]
enum EngineKind {
    /// Built-in search with the handcrafted PeSTO evaluator.
    Pesto,
    /// Built-in search with a trained quantized NNUE net (path to the net).
    Nnue(String),
    /// External UCI engine subprocess (command + args).
    Uci(Vec<String>),
}

#[derive(Clone)]
struct EngineSpec {
    id: String,
    label: String,
    kind: EngineKind,
}

struct Config {
    hash_mb: usize,
    engines: Vec<EngineSpec>,
}

impl Config {
    /// The engine for a request's `engine` parameter (default: the first).
    fn engine(&self, req: &Request) -> &EngineSpec {
        req.param("engine")
            .and_then(|id| self.engines.iter().find(|e| e.id == id))
            .unwrap_or(&self.engines[0])
    }
}

/// `--uci` argument: either a bare command line, or `name=command line`.
/// The `name=` form is only recognized when the name carries no whitespace,
/// so `lc0 --weights=x.pb` parses as a command, not as a name.
fn parse_uci_flag(arg: &str) -> Result<EngineSpec, String> {
    let (name, cmd) = match arg.split_once('=') {
        Some((n, rest)) if !n.trim().contains(char::is_whitespace) && !rest.trim().is_empty() => {
            (Some(n.trim().to_string()), rest.trim())
        }
        _ => (None, arg.trim()),
    };
    let parts: Vec<String> = cmd.split_whitespace().map(|s| s.to_string()).collect();
    let exe = parts.first().ok_or("--uci needs a command")?;
    let name = name.unwrap_or_else(|| {
        Path::new(exe)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| exe.clone())
    });
    Ok(EngineSpec {
        id: format!("uci-{}", name.to_lowercase().replace(' ', "-")),
        label: name,
        kind: EngineKind::Uci(parts),
    })
}

/// Whether an executable is reachable through PATH.
fn on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

fn main() {
    let mut port: u16 = 8000;
    let mut hash_mb: usize = 128;
    let mut engines = vec![EngineSpec {
        id: "pesto".to_string(),
        label: "PeSTO (built-in)".to_string(),
        kind: EngineKind::Pesto,
    }];

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--port" => match args.next().and_then(|v| v.parse().ok()) {
                Some(p) => port = p,
                None => die("--port needs a number"),
            },
            "--hash" => match args.next().and_then(|v| v.parse().ok()) {
                Some(h) => hash_mb = h,
                None => die("--hash needs a size in MiB"),
            },
            "--nnue" => match args.next() {
                Some(path) => {
                    // Load once now: fail fast on a bad net, not mid-session.
                    if let Err(e) = QNnueEval::load(&path) {
                        die(&format!("--nnue {path}: {e}"));
                    }
                    let stem = Path::new(&path)
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    engines.push(EngineSpec {
                        id: format!("nnue-{stem}"),
                        label: format!("NNUE {stem} (built-in)"),
                        kind: EngineKind::Nnue(path),
                    });
                }
                None => die("--nnue needs a net file path"),
            },
            "--uci" => match args.next() {
                Some(arg) => match parse_uci_flag(&arg) {
                    Ok(spec) => engines.push(spec),
                    Err(e) => die(&e),
                },
                None => die("--uci needs a command (or name=command)"),
            },
            "--help" | "-h" => {
                println!(
                    "usage: chess-web [--port N] [--hash MiB] [--nnue net.nnue]... [--uci \"[name=]cmd args\"]...\n\
                     A stockfish found on PATH is registered automatically."
                );
                return;
            }
            other => die(&format!("unknown flag '{other}' (try --help)")),
        }
    }
    hash_mb = hash_mb.clamp(1, 4096);

    let have_stockfish = engines.iter().any(|e| {
        matches!(&e.kind, EngineKind::Uci(cmd) if cmd.first().is_some_and(|c| c.contains("stockfish")))
    });
    if !have_stockfish && on_path("stockfish") {
        engines.push(EngineSpec {
            id: "uci-stockfish".to_string(),
            label: "Stockfish".to_string(),
            kind: EngineKind::Uci(vec!["stockfish".to_string()]),
        });
    }

    let config = Arc::new(Config { hash_mb, engines });

    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => die(&format!("cannot bind 127.0.0.1:{port}: {e}")),
    };
    println!("chess-web: analysis GUI at http://127.0.0.1:{port}/  (hash {hash_mb} MiB)");
    for e in &config.engines {
        println!("  engine: {} ({})", e.label, e.id);
    }

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let config = config.clone();
        thread::spawn(move || handle_connection(stream, &config));
    }
}

fn die(msg: &str) -> ! {
    eprintln!("chess-web: {msg}");
    std::process::exit(2);
}

fn handle_connection(stream: TcpStream, config: &Config) {
    let _ = stream.set_nodelay(true);
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut stream = stream;
    let Ok(req) = read_request(&mut reader) else {
        return;
    };

    let _ = match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/") | ("GET", "/index.html") => {
            respond(&mut stream, "200 OK", "text/html; charset=utf-8", INDEX_HTML.as_bytes())
        }
        ("GET", "/app.js") => respond(
            &mut stream,
            "200 OK",
            "application/javascript; charset=utf-8",
            APP_JS.as_bytes(),
        ),
        ("GET", "/style.css") => {
            respond(&mut stream, "200 OK", "text/css; charset=utf-8", STYLE_CSS.as_bytes())
        }
        ("GET", "/api/engines") => respond_json(&mut stream, &engines_json(config)),
        ("GET", "/api/state") => match handle_state(&req) {
            Ok(json) => respond_json(&mut stream, &json),
            Err(e) => respond_bad_request(&mut stream, &e),
        },
        ("GET", "/api/analyze") => handle_analyze(&mut stream, &req, config),
        ("GET", "/api/bestmove") => match handle_bestmove(&req, config) {
            Ok(json) => respond_json(&mut stream, &json),
            Err(e) => respond_bad_request(&mut stream, &e),
        },
        ("GET", "/api/evalseries") => handle_evalseries(&mut stream, &req, config),
        ("POST", "/api/pgn") => match handle_pgn(&req) {
            Ok(json) => respond_json(&mut stream, &json),
            Err(e) => respond_bad_request(&mut stream, &e),
        },
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    };
    let _ = stream.shutdown(Shutdown::Both);
}

fn engines_json(config: &Config) -> String {
    let items = config.engines.iter().map(|e| {
        format!(
            "{{\"id\":{},\"label\":{},\"external\":{}}}",
            jstr(&e.id),
            jstr(&e.label),
            matches!(e.kind, EngineKind::Uci(_)),
        )
    });
    format!(
        "{{\"engines\":{},\"default\":{}}}",
        jarr(items),
        jstr(&config.engines[0].id)
    )
}

// --- position plumbing ---

/// A request's game line: the validated move list plus a `Game` rewound to the
/// requested view index `at`.
struct Line {
    start_fen: String,
    ucis: Vec<String>,
    sans: Vec<String>,
    at: usize,
    game: Game,
}

fn line_from_req(req: &Request) -> Result<Line, String> {
    let fen = req.param("fen").filter(|s| !s.is_empty()).unwrap_or("startpos");
    let mut game = if fen == "startpos" {
        Game::new()
    } else {
        Game::from_fen(fen).map_err(|e| format!("bad FEN: {e:?}"))?
    };
    let start_fen = game.board().to_fen();

    let mut ucis = Vec::new();
    let mut sans = Vec::new();
    for tok in req
        .param("moves")
        .unwrap_or("")
        .split([' ', ','])
        .filter(|t| !t.is_empty())
    {
        let mv = game
            .board()
            .parse_uci(tok)
            .ok_or_else(|| format!("illegal move '{}' at ply {}", tok, ucis.len()))?;
        sans.push(game.board().san(mv));
        ucis.push(mv.to_uci());
        game.push(mv);
    }

    let at = req.num("at", ucis.len(), 0, ucis.len());
    for _ in at..ucis.len() {
        game.pop();
    }
    Ok(Line { start_fen, ucis, sans, at, game })
}

fn side_char(c: Color) -> &'static str {
    match c {
        Color::White => "w",
        Color::Black => "b",
    }
}

fn outcome_json(game: &Game) -> String {
    let (status, winner, reason) = match game.outcome() {
        Outcome::Ongoing => ("ongoing", None, None),
        Outcome::Checkmate { winner } => ("checkmate", Some(side_char(winner)), None),
        Outcome::Stalemate => ("draw", None, Some("stalemate")),
        Outcome::Draw(r) => (
            "draw",
            None,
            Some(match r {
                chess::DrawReason::FiftyMove => "fifty-move rule",
                chess::DrawReason::SeventyFiveMove => "seventy-five-move rule",
                chess::DrawReason::ThreefoldRepetition => "threefold repetition",
                chess::DrawReason::FivefoldRepetition => "fivefold repetition",
                chess::DrawReason::InsufficientMaterial => "insufficient material",
            }),
        ),
    };
    format!(
        "{{\"status\":{},\"winner\":{},\"reason\":{}}}",
        jstr(status),
        winner.map_or("null".to_string(), jstr),
        reason.map_or("null".to_string(), jstr),
    )
}

fn handle_state(req: &Request) -> Result<String, String> {
    let line = line_from_req(req)?;
    let board = line.game.board();

    let legal = jarr(board.legal_moves().iter().map(|&mv| {
        format!(
            "{{\"uci\":{},\"san\":{}}}",
            jstr(&mv.to_uci()),
            jstr(&board.san(mv))
        )
    }));
    let last_move = if line.at > 0 {
        jstr(&line.ucis[line.at - 1])
    } else {
        "null".to_string()
    };

    Ok(format!(
        "{{\"startFen\":{},\"moves\":{},\"sans\":{},\"at\":{},\"fen\":{},\"side\":{},\"check\":{},\"lastMove\":{},\"outcome\":{},\"legal\":{}}}",
        jstr(&line.start_fen),
        jarr(line.ucis.iter().map(|u| jstr(u))),
        jarr(line.sans.iter().map(|s| jstr(s))),
        line.at,
        jstr(&board.to_fen()),
        jstr(side_char(board.side_to_move())),
        board.in_check(),
        last_move,
        outcome_json(&line.game),
        legal,
    ))
}

// --- engine plumbing ---

/// A built-in engine: the library search over either evaluator.
enum BuiltIn {
    Pesto(Engine<HandcraftedEval>),
    Nnue(Engine<QNnueEval>),
}

impl BuiltIn {
    fn create(spec: &EngineSpec, hash_mb: usize) -> Result<BuiltIn, String> {
        match &spec.kind {
            EngineKind::Pesto => Ok(BuiltIn::Pesto(Engine::with_eval_and_tt(
                HandcraftedEval::new(),
                hash_mb,
            ))),
            EngineKind::Nnue(path) => Ok(BuiltIn::Nnue(Engine::with_eval_and_tt(
                QNnueEval::load(path)?,
                hash_mb,
            ))),
            EngineKind::Uci(_) => Err("not a built-in engine".to_string()),
        }
    }

    fn set_history(&mut self, keys: &[RepetitionKey]) {
        match self {
            BuiltIn::Pesto(e) => e.set_history(keys),
            BuiltIn::Nnue(e) => e.set_history(keys),
        }
    }

    fn stop_handle(&self) -> Arc<AtomicBool> {
        match self {
            BuiltIn::Pesto(e) => e.stop_handle(),
            BuiltIn::Nnue(e) => e.stop_handle(),
        }
    }

    fn analyze_excluding(
        &mut self,
        board: &Board,
        limits: &Limits,
        exclude: &[Move],
        on_info: impl FnMut(&SearchInfo),
    ) -> Analysis {
        match self {
            BuiltIn::Pesto(e) => e.analyze_excluding(board, limits, exclude, on_info),
            BuiltIn::Nnue(e) => e.analyze_excluding(board, limits, exclude, on_info),
        }
    }

    fn analyze(&mut self, board: &Board, limits: &Limits) -> Analysis {
        self.analyze_excluding(board, limits, &[], |_| {})
    }
}

/// One engine of each flavor is kept warm between requests, keyed by engine
/// id, so its hash table carries over as the user steps through a game.
/// Taken whole (not borrowed) so a slow request never blocks others; a
/// pooled engine of a *different* id is simply dropped (the user switched).
static BUILTIN_POOL: Mutex<Option<(String, BuiltIn)>> = Mutex::new(None);
static UCI_POOL: Mutex<Option<(String, UciEngine)>> = Mutex::new(None);

fn take_builtin(spec: &EngineSpec, hash_mb: usize) -> Result<BuiltIn, String> {
    if let Ok(mut slot) = BUILTIN_POOL.lock()
        && let Some((id, engine)) = slot.take()
        && id == spec.id
    {
        return Ok(engine);
    }
    BuiltIn::create(spec, hash_mb)
}

fn return_builtin(id: &str, engine: BuiltIn) {
    if let Ok(mut slot) = BUILTIN_POOL.lock() {
        *slot = Some((id.to_string(), engine));
    }
}

fn take_uci(spec: &EngineSpec, cmd: &[String], hash_mb: usize) -> Result<UciEngine, String> {
    if let Ok(mut slot) = UCI_POOL.lock()
        && let Some((id, mut engine)) = slot.take()
        && id == spec.id
        && engine.sync().is_ok()
    {
        return Ok(engine); // a dead process falls through and respawns
    }
    UciEngine::spawn(cmd, hash_mb)
}

fn return_uci(id: &str, engine: UciEngine) {
    if let Ok(mut slot) = UCI_POOL.lock() {
        *slot = Some((id.to_string(), engine));
    }
}

/// Run `on_disconnect` when the client goes away. SSE clients never send
/// bytes after the request, so any read completion (EOF or error) means
/// disconnect; the handler's final `shutdown` unblocks the read so the thread
/// always exits. `active` gates the callback: once the request is finished
/// the engine may already be serving someone else, and a late watcher firing
/// must not disturb it.
fn watch_disconnect(
    stream: &TcpStream,
    active: Arc<AtomicBool>,
    on_disconnect: impl FnOnce() + Send + 'static,
) {
    let Ok(mut read_half) = stream.try_clone() else {
        return;
    };
    thread::spawn(move || {
        let mut buf = [0u8; 512];
        loop {
            match read_half.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        if active.load(Ordering::Relaxed) {
            on_disconnect();
        }
    });
}

/// Engine score (side-to-move POV) -> White-POV `(cp, mate)` JSON fields.
fn white_score(stm: Color, score: i32) -> (Option<i32>, Option<i32>) {
    let sign = if stm == Color::White { 1 } else { -1 };
    match mate_in_moves(score) {
        Some(m) => (None, Some(sign * m)),
        None => (Some(sign * score), None),
    }
}

fn info_json(root: &Board, multipv: usize, info: &SearchInfo) -> String {
    // Render the PV in SAN by walking it; truncate defensively if a stale
    // table ever yields an illegal continuation.
    let mut b = root.clone();
    let mut pv_uci = Vec::new();
    let mut pv_san = Vec::new();
    for &mv in &info.pv {
        if !b.legal_moves().contains(mv) {
            break;
        }
        pv_uci.push(jstr(&mv.to_uci()));
        pv_san.push(jstr(&b.san(mv)));
        b.make_move(mv);
    }
    let (cp, mate) = white_score(root.side_to_move(), info.score);
    format!(
        "{{\"multipv\":{},\"depth\":{},\"seldepth\":{},\"cp\":{},\"mate\":{},\"nodes\":{},\"nps\":{},\"timeMs\":{},\"pv\":{},\"sanPv\":{}}}",
        multipv,
        info.depth,
        info.seldepth,
        jopt(cp),
        jopt(mate),
        info.nodes,
        info.nps,
        info.time_ms,
        jarr(pv_uci),
        jarr(pv_san),
    )
}

fn send_done(stream: &mut TcpStream, root: &Board, a: &Analysis) -> std::io::Result<()> {
    let (best, san) = if root.legal_moves().contains(a.best_move) {
        (jstr(&a.best_move.to_uci()), jstr(&root.san(a.best_move)))
    } else {
        ("null".to_string(), "null".to_string())
    };
    let (cp, mate) = white_score(root.side_to_move(), a.score);
    sse_event(
        stream,
        "done",
        &format!(
            "{{\"bestmove\":{},\"san\":{},\"cp\":{},\"mate\":{},\"depth\":{}}}",
            best,
            san,
            jopt(cp),
            jopt(mate),
            a.depth,
        ),
    )
}

// --- /api/bestmove ---

/// Synchronous best-move lookup for play-against-engine mode.
/// Returns `{"uci": "e2e4", "san": "e4"}` after searching for `movetime` ms.
fn handle_bestmove(req: &Request, config: &Config) -> Result<String, String> {
    let line = line_from_req(req)?;
    if line.game.outcome().is_over() {
        return Err("game is over".to_string());
    }
    let movetime: u64 = req.num("movetime", 1000, 100, 30_000);
    let spec = config.engine(req);
    let board = line.game.board().clone();

    let mv = match &spec.kind {
        EngineKind::Uci(cmd) => {
            let mut engine = take_uci(spec, cmd, config.hash_mb)
                .map_err(|e| format!("engine '{}': {e}", spec.label))?;
            let go = GoParams { multipv: 1, movetime, depth: 0 };
            let best_str = engine
                .search(&uci_position(&line), &go, |_| true)
                .map_err(|e| e)?
                .ok_or_else(|| "engine returned no move".to_string())?;
            let mv = board
                .parse_uci(&best_str)
                .ok_or_else(|| format!("engine returned illegal move: {best_str}"))?;
            return_uci(&spec.id, engine);
            mv
        }
        _ => {
            let mut engine = take_builtin(spec, config.hash_mb)?;
            let keys = line.game.position_keys();
            engine.set_history(&keys[..keys.len() - 1]);
            let a = engine.analyze(&board, &Limits::movetime(movetime));
            return_builtin(&spec.id, engine);
            if !board.legal_moves().contains(a.best_move) {
                return Err("engine has no legal move".to_string());
            }
            a.best_move
        }
    };

    Ok(format!(
        "{{\"uci\":{},\"san\":{}}}",
        jstr(&mv.to_uci()),
        jstr(&board.san(mv)),
    ))
}

// --- /api/analyze ---

fn handle_analyze(stream: &mut TcpStream, req: &Request, config: &Config) -> std::io::Result<()> {
    let line = match line_from_req(req) {
        Ok(l) => l,
        Err(e) => return respond_bad_request(stream, &e),
    };
    let multipv: usize = req.num("multipv", 1, 1, 8);
    let movetime: u64 = req.num("movetime", 0, 0, 3_600_000); // 0 = unlimited
    let depth_cap: i32 = req.num("depth", 0, 0, MAX_PLY as i32 - 2); // 0 = unlimited
    let spec = config.engine(req);

    // Acquire the engine before opening the stream so failures (bad external
    // command, unreadable net) surface as a proper HTTP error.
    enum Acquired {
        B(BuiltIn),
        U(UciEngine),
    }
    let acquired = match &spec.kind {
        EngineKind::Uci(cmd) => match take_uci(spec, cmd, config.hash_mb) {
            Ok(e) => Acquired::U(e),
            Err(e) => {
                return respond_bad_request(stream, &format!("engine '{}': {e}", spec.label));
            }
        },
        _ => match take_builtin(spec, config.hash_mb) {
            Ok(e) => Acquired::B(e),
            Err(e) => return respond_bad_request(stream, &e),
        },
    };

    start_sse(stream)?;

    if line.game.outcome().is_over() {
        match acquired {
            Acquired::B(e) => return_builtin(&spec.id, e),
            Acquired::U(e) => return_uci(&spec.id, e),
        }
        return sse_event(
            stream,
            "done",
            &format!("{{\"outcome\":{}}}", outcome_json(&line.game)),
        );
    }

    let active = Arc::new(AtomicBool::new(true));
    match acquired {
        Acquired::B(mut engine) => {
            let board = line.game.board().clone();
            let keys = line.game.position_keys();
            engine.set_history(&keys[..keys.len() - 1]);
            let stop = engine.stop_handle();
            stop.store(false, Ordering::Relaxed);
            {
                let stop = stop.clone();
                watch_disconnect(stream, active.clone(), move || {
                    stop.store(true, Ordering::Relaxed);
                });
            }
            let lines = multipv.min(board.legal_moves().len());
            let result = if lines <= 1 {
                analyze_single(stream, &mut engine, &board, movetime, depth_cap, &stop)
            } else {
                analyze_multi(stream, &mut engine, &board, lines, movetime, depth_cap, &stop)
            };
            active.store(false, Ordering::Relaxed);
            return_builtin(&spec.id, engine);
            result
        }
        Acquired::U(mut engine) => {
            let result = uci_analyze(stream, &mut engine, &line, multipv, movetime, depth_cap, &active);
            active.store(false, Ordering::Relaxed);
            match result {
                Ok(io_result) => {
                    return_uci(&spec.id, engine);
                    io_result
                }
                // The engine process broke; drop it so the next request respawns.
                Err(_) => Ok(()),
            }
        }
    }
}

/// Analysis through an external UCI engine: native MultiPV, same SSE schema.
/// `Err` means the engine process itself failed (the caller drops it);
/// `Ok(io::Result)` reflects the client stream.
fn uci_analyze(
    stream: &mut TcpStream,
    engine: &mut UciEngine,
    line: &Line,
    multipv: usize,
    movetime: u64,
    depth: i32,
    active: &Arc<AtomicBool>,
) -> Result<std::io::Result<()>, String> {
    let board = line.game.board().clone();
    {
        let stdin = engine.stdin_handle();
        watch_disconnect(stream, active.clone(), move || {
            let _ = uci::send_line(&stdin, "stop");
        });
    }

    let go = GoParams {
        multipv: multipv.min(board.legal_moves().len()).max(1),
        movetime,
        depth,
    };
    let mut last_best: Option<UciInfo> = None;
    let mut write_ok = true;
    let best = engine.search(&uci_position(line), &go, |info| {
        if info.multipv <= 1 {
            last_best = Some(info.clone());
        }
        if write_ok && sse_event(stream, "info", &uci_info_json(&board, info)).is_err() {
            write_ok = false;
        }
        write_ok
    })?;

    let (cp, mate, depth) = match &last_best {
        Some(i) => {
            let (cp, mate) = white_opt_score(board.side_to_move(), i.cp, i.mate);
            (cp, mate, i.depth)
        }
        None => (None, None, 0),
    };
    let (bm, san) = match best.and_then(|b| board.parse_uci(&b)) {
        Some(mv) => (jstr(&mv.to_uci()), jstr(&board.san(mv))),
        None => ("null".to_string(), "null".to_string()),
    };
    Ok(sse_event(
        stream,
        "done",
        &format!(
            "{{\"bestmove\":{bm},\"san\":{san},\"cp\":{},\"mate\":{},\"depth\":{depth}}}",
            jopt(cp),
            jopt(mate),
        ),
    ))
}

/// `position ...` command for the line's view index.
fn uci_position(line: &Line) -> String {
    let mut cmd = format!("position fen {}", line.start_fen);
    if line.at > 0 {
        cmd += " moves ";
        cmd += &line.ucis[..line.at].join(" ");
    }
    cmd
}

/// UCI engine score (side-to-move POV, already in moves for mate) -> White POV.
fn white_opt_score(stm: Color, cp: Option<i32>, mate: Option<i32>) -> (Option<i32>, Option<i32>) {
    let sign = if stm == Color::White { 1 } else { -1 };
    (cp.map(|v| sign * v), mate.map(|v| sign * v))
}

fn uci_info_json(root: &Board, info: &UciInfo) -> String {
    // Walk the engine's PV from the root, truncating at the first move we
    // cannot legally parse, so pv and sanPv stay aligned.
    let mut b = root.clone();
    let mut pv_uci = Vec::new();
    let mut pv_san = Vec::new();
    for m in &info.pv {
        let Some(mv) = b.parse_uci(m) else { break };
        pv_uci.push(jstr(&mv.to_uci()));
        pv_san.push(jstr(&b.san(mv)));
        b.make_move(mv);
    }
    let (cp, mate) = white_opt_score(root.side_to_move(), info.cp, info.mate);
    format!(
        "{{\"multipv\":{},\"depth\":{},\"seldepth\":{},\"cp\":{},\"mate\":{},\"nodes\":{},\"nps\":{},\"timeMs\":{},\"pv\":{},\"sanPv\":{}}}",
        info.multipv,
        info.depth,
        info.seldepth,
        jopt(cp),
        jopt(mate),
        info.nodes,
        info.nps,
        info.time_ms,
        jarr(pv_uci),
        jarr(pv_san),
    )
}

fn analyze_single(
    stream: &mut TcpStream,
    engine: &mut BuiltIn,
    board: &Board,
    movetime: u64,
    depth_cap: i32,
    stop: &Arc<AtomicBool>,
) -> std::io::Result<()> {
    let limits = Limits {
        depth: (depth_cap > 0).then_some(depth_cap),
        movetime: (movetime > 0).then_some(movetime),
        infinite: movetime == 0,
        ..Default::default()
    };
    let analysis = engine.analyze_excluding(board, &limits, &[], |info| {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        if sse_event(stream, "info", &info_json(board, 1, info)).is_err() {
            stop.store(true, Ordering::Relaxed);
        }
    });
    send_done(stream, board, &analysis)
}

/// MultiPV emulation: depth-stepped rounds. Each round searches line 1
/// normally, then lines 2..k with the better root moves excluded; the shared
/// transposition table makes each re-search of shallower depths nearly free.
fn analyze_multi(
    stream: &mut TcpStream,
    engine: &mut BuiltIn,
    board: &Board,
    lines: usize,
    movetime: u64,
    depth_cap: i32,
    stop: &Arc<AtomicBool>,
) -> std::io::Result<()> {
    let start = Instant::now();
    let max_depth = if depth_cap > 0 { depth_cap } else { MAX_PLY as i32 - 2 };
    let mut best: Option<Analysis> = None;

    'rounds: for d in 4..=max_depth {
        let mut excluded: Vec<Move> = Vec::new();
        let mut all_mate = true;
        for k in 1..=lines {
            if stop.load(Ordering::Relaxed) {
                break 'rounds;
            }
            let budget = if movetime > 0 {
                let elapsed = start.elapsed().as_millis() as u64;
                if elapsed >= movetime {
                    break 'rounds;
                }
                Some(movetime - elapsed)
            } else {
                None
            };
            let limits = Limits {
                depth: Some(d),
                movetime: budget,
                ..Default::default()
            };
            let a = engine.analyze_excluding(board, &limits, &excluded, |_| {});
            if stop.load(Ordering::Relaxed) {
                break 'rounds;
            }

            let info = SearchInfo {
                depth: a.depth,
                seldepth: a.seldepth,
                score: a.score,
                nodes: a.nodes,
                time_ms: a.time_ms,
                nps: (a.nodes as u128 * 1000).checked_div(a.time_ms.max(1)).unwrap_or(0) as u64,
                hashfull: 0,
                pv: a.pv.clone(),
            };
            if sse_event(stream, "info", &info_json(board, k, &info)).is_err() {
                stop.store(true, Ordering::Relaxed);
                break 'rounds;
            }

            if !is_mate(a.score) {
                all_mate = false;
            }
            if k == 1 {
                best = Some(a.clone());
            }
            excluded.push(a.best_move);
        }
        // Every line is a forced mate: deeper rounds change nothing.
        if all_mate {
            break;
        }
    }

    match best {
        Some(a) => send_done(stream, board, &a),
        None => sse_event(stream, "done", "{}"),
    }
}

// --- /api/evalseries ---

/// Evaluate every position of the line (for the game-analysis graph), one SSE
/// event per ply. The single reused engine makes consecutive positions cheap.
fn handle_evalseries(stream: &mut TcpStream, req: &Request, config: &Config) -> std::io::Result<()> {
    let line = match line_from_req(req) {
        Ok(l) => l,
        Err(e) => return respond_bad_request(stream, &e),
    };
    let per_ms: u64 = req.num("movetime", 150, 20, 10_000);
    let spec = config.engine(req);

    match &spec.kind {
        EngineKind::Uci(cmd) => {
            let mut engine = match take_uci(spec, cmd, config.hash_mb) {
                Ok(e) => e,
                Err(e) => {
                    return respond_bad_request(stream, &format!("engine '{}': {e}", spec.label));
                }
            };
            start_sse(stream)?;
            let active = Arc::new(AtomicBool::new(true));
            let stop = Arc::new(AtomicBool::new(false));
            {
                let stop = stop.clone();
                let stdin = engine.stdin_handle();
                watch_disconnect(stream, active.clone(), move || {
                    stop.store(true, Ordering::Relaxed);
                    let _ = uci::send_line(&stdin, "stop");
                });
            }

            let mut game = Game::from_fen(&line.start_fen).expect("own FEN round-trips");
            let mut broken = false;
            for idx in 0..=line.ucis.len() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if idx > 0 {
                    game.push_uci(&line.ucis[idx - 1]);
                }
                let event = if game.outcome().is_over() {
                    terminal_event_json(idx, &game)
                } else {
                    let mut pos = format!("position fen {}", line.start_fen);
                    if idx > 0 {
                        pos += " moves ";
                        pos += &line.ucis[..idx].join(" ");
                    }
                    let go = GoParams { multipv: 1, movetime: per_ms, depth: 0 };
                    let mut last: Option<UciInfo> = None;
                    let best = match engine.search(&pos, &go, |i| {
                        if i.multipv <= 1 {
                            last = Some(i.clone());
                        }
                        true
                    }) {
                        Ok(b) => b,
                        Err(_) => {
                            broken = true;
                            break;
                        }
                    };
                    let (cp, mate) = match &last {
                        Some(i) => white_opt_score(game.side_to_move(), i.cp, i.mate),
                        None => (None, None),
                    };
                    let best_mv = best.and_then(|b| game.board().parse_uci(&b));
                    eval_event_json(idx, &game, cp, mate, best_mv)
                };
                if sse_event(stream, "eval", &event).is_err() {
                    break;
                }
            }
            let _ = sse_event(stream, "done", "{}");
            active.store(false, Ordering::Relaxed);
            if !broken {
                return_uci(&spec.id, engine);
            }
            Ok(())
        }
        _ => {
            let mut engine = match take_builtin(spec, config.hash_mb) {
                Ok(e) => e,
                Err(e) => return respond_bad_request(stream, &e),
            };
            start_sse(stream)?;
            let active = Arc::new(AtomicBool::new(true));
            let stop = engine.stop_handle();
            stop.store(false, Ordering::Relaxed);
            {
                let stop = stop.clone();
                watch_disconnect(stream, active.clone(), move || {
                    stop.store(true, Ordering::Relaxed);
                });
            }

            let mut game = Game::from_fen(&line.start_fen).expect("own FEN round-trips");
            for idx in 0..=line.ucis.len() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                if idx > 0 {
                    game.push_uci(&line.ucis[idx - 1]);
                }
                let event = if game.outcome().is_over() {
                    terminal_event_json(idx, &game)
                } else {
                    let keys = game.position_keys();
                    engine.set_history(&keys[..keys.len() - 1]);
                    let a = engine.analyze(game.board(), &Limits::movetime(per_ms));
                    let (cp, mate) = white_score(game.side_to_move(), a.score);
                    let best = game
                        .board()
                        .legal_moves()
                        .contains(a.best_move)
                        .then_some(a.best_move);
                    eval_event_json(idx, &game, cp, mate, best)
                };
                if sse_event(stream, "eval", &event).is_err() {
                    break;
                }
            }
            let _ = sse_event(stream, "done", "{}");
            active.store(false, Ordering::Relaxed);
            return_builtin(&spec.id, engine);
            Ok(())
        }
    }
}

fn terminal_event_json(idx: usize, game: &Game) -> String {
    format!(
        "{{\"idx\":{},\"side\":{},\"cp\":null,\"mate\":null,\"terminal\":{}}}",
        idx,
        jstr(side_char(game.side_to_move())),
        outcome_json(game),
    )
}

fn eval_event_json(
    idx: usize,
    game: &Game,
    cp: Option<i32>,
    mate: Option<i32>,
    best: Option<Move>,
) -> String {
    let (best, san) = match best {
        Some(mv) => (jstr(&mv.to_uci()), jstr(&game.board().san(mv))),
        None => ("null".to_string(), "null".to_string()),
    };
    format!(
        "{{\"idx\":{},\"side\":{},\"cp\":{},\"mate\":{},\"best\":{},\"bestSan\":{}}}",
        idx,
        jstr(side_char(game.side_to_move())),
        jopt(cp),
        jopt(mate),
        best,
        san,
    )
}

// --- /api/pgn ---

fn handle_pgn(req: &Request) -> Result<String, String> {
    let text = String::from_utf8_lossy(&req.body);
    let (start_fen, tokens) = parse_pgn_game(&text);

    let mut game = match &start_fen {
        Some(f) => Game::from_fen(f).map_err(|e| format!("bad FEN tag: {e:?}"))?,
        None => Game::new(),
    };
    let mut ucis = Vec::new();
    let mut sans = Vec::new();
    for tok in tokens {
        let mv = game.board().parse_san(&tok).ok_or_else(|| {
            format!(
                "unparseable or illegal move '{}' (after {} plies)",
                tok,
                ucis.len()
            )
        })?;
        sans.push(game.board().san(mv));
        ucis.push(mv.to_uci());
        game.push(mv);
    }
    if ucis.is_empty() && start_fen.is_none() {
        return Err("no moves found in PGN".to_string());
    }

    Ok(format!(
        "{{\"fen\":{},\"moves\":{},\"sans\":{}}}",
        jstr(start_fen.as_deref().unwrap_or("startpos")),
        jarr(ucis.iter().map(|u| jstr(u))),
        jarr(sans.iter().map(|s| jstr(s))),
    ))
}

/// First game of a PGN: optional FEN tag + cleaned SAN tokens. Strips tag
/// pairs, `{...}` comments (multi-line), `(...)` variations (nested), `;`
/// line comments, `%` escape lines, NAGs, move numbers, and results.
fn parse_pgn_game(text: &str) -> (Option<String>, Vec<String>) {
    let mut fen = None;
    let mut movetext = String::new();
    let mut seen_moves = false;
    let mut brace = 0i32;

    'lines: for raw in text.lines() {
        let trimmed = raw.trim_start();
        if brace == 0 {
            if trimmed.starts_with('%') {
                continue;
            }
            if trimmed.starts_with('[') {
                if seen_moves {
                    break; // next game's headers
                }
                if let Some(rest) = trimmed.strip_prefix("[FEN")
                    && let Some(first) = rest.find('"')
                    && let Some(last) = rest.rfind('"')
                    && last > first
                {
                    fen = Some(rest[first + 1..last].to_string());
                }
                continue;
            }
        }
        for c in raw.chars() {
            match c {
                '{' => brace += 1,
                '}' => brace = (brace - 1).max(0),
                ';' if brace == 0 => {
                    movetext.push(' ');
                    continue 'lines;
                }
                _ if brace == 0 => {
                    if !c.is_whitespace() {
                        seen_moves = true;
                    }
                    movetext.push(c);
                }
                _ => {}
            }
        }
        movetext.push(' ');
    }

    // Strip (nested) variations, then tokenize.
    let mut depth = 0i32;
    let cleaned: String = movetext
        .chars()
        .filter(|&c| match c {
            '(' => {
                depth += 1;
                false
            }
            ')' => {
                depth = (depth - 1).max(0);
                false
            }
            _ => depth == 0,
        })
        .collect();
    let tokens = cleaned.split_whitespace().filter_map(clean_token).collect();
    (fen, tokens)
}

/// Movetext token -> SAN move, or `None` for move numbers / NAGs / results.
fn clean_token(tok: &str) -> Option<String> {
    let t = tok.trim();
    if t.is_empty() || t.starts_with('$') || matches!(t, "1-0" | "0-1" | "1/2-1/2" | "*") {
        return None;
    }
    // Strip a leading move number like "12." / "12..." possibly glued to a move.
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 {
        let mut j = i;
        while j < bytes.len() && bytes[j] == b'.' {
            j += 1;
        }
        if j > i || i == bytes.len() {
            let rest = &t[j..];
            return if rest.is_empty() {
                None
            } else {
                Some(rest.to_string())
            };
        }
    }
    Some(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(query: &[(&str, &str)]) -> Request {
        Request {
            method: "GET".to_string(),
            path: "/api/state".to_string(),
            query: query
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    #[test]
    fn state_reports_sans_and_legal_moves() {
        let r = req(&[("fen", "startpos"), ("moves", "e2e4 e7e5 g1f3")]);
        let json = handle_state(&r).unwrap();
        assert!(json.contains("\"sans\":[\"e4\",\"e5\",\"Nf3\"]"), "{json}");
        assert!(json.contains("\"side\":\"b\""), "{json}");
        assert!(json.contains("\"lastMove\":\"g1f3\""), "{json}");
        assert!(json.contains("\"uci\":\"b8c6\""), "{json}");
    }

    #[test]
    fn state_rewinds_to_at() {
        let r = req(&[("moves", "e2e4 e7e5"), ("at", "1")]);
        let json = handle_state(&r).unwrap();
        // Viewing after 1.e4: black to move, last move e2e4, but full SAN list kept.
        assert!(json.contains("\"side\":\"b\""), "{json}");
        assert!(json.contains("\"lastMove\":\"e2e4\""), "{json}");
        assert!(json.contains("\"sans\":[\"e4\",\"e5\"]"), "{json}");
    }

    #[test]
    fn state_rejects_illegal_moves_and_bad_fen() {
        assert!(handle_state(&req(&[("moves", "e2e5")])).is_err());
        assert!(handle_state(&req(&[("fen", "not a fen")])).is_err());
    }

    #[test]
    fn state_reports_checkmate() {
        let r = req(&[("moves", "f2f3 e7e5 g2g4 d8h4")]);
        let json = handle_state(&r).unwrap();
        assert!(json.contains("\"status\":\"checkmate\""), "{json}");
        assert!(json.contains("\"winner\":\"b\""), "{json}");
        assert!(json.contains("\"legal\":[]"), "{json}");
    }

    #[test]
    fn pgn_import_full_featured() {
        let pgn = r#"%escape line ignored
[Event "Test"]
[FEN "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"]

1. e4 {a comment
spanning lines} e5 (1... c5 2. Nf3) 2. Nf3! $14 Nc6 ; line comment
3. Bb5 1-0
[Event "Second game must be ignored"]
1. d4 d5 *
"#;
        let r = Request {
            method: "POST".to_string(),
            path: "/api/pgn".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            body: pgn.as_bytes().to_vec(),
        };
        let json = handle_pgn(&r).unwrap();
        assert!(
            json.contains("\"moves\":[\"e2e4\",\"e7e5\",\"g1f3\",\"b8c6\",\"f1b5\"]"),
            "{json}"
        );
        assert!(json.contains("\"sans\":[\"e4\",\"e5\",\"Nf3\",\"Nc6\",\"Bb5\"]"), "{json}");
    }

    #[test]
    fn pgn_import_rejects_garbage() {
        let r = Request {
            method: "POST".to_string(),
            path: "/api/pgn".to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            body: b"1. e4 Kxe7".to_vec(),
        };
        assert!(handle_pgn(&r).is_err());
    }

    #[test]
    fn clean_token_strips_numbers_and_nags() {
        assert_eq!(clean_token("12."), None);
        assert_eq!(clean_token("12...Nf6"), Some("Nf6".to_string()));
        assert_eq!(clean_token("1.e4"), Some("e4".to_string()));
        assert_eq!(clean_token("$14"), None);
        assert_eq!(clean_token("1/2-1/2"), None);
        assert_eq!(clean_token("O-O-O+"), Some("O-O-O+".to_string()));
    }

    #[test]
    fn uci_flag_forms() {
        let plain = parse_uci_flag("stockfish").unwrap();
        assert_eq!(plain.id, "uci-stockfish");
        assert!(matches!(&plain.kind, EngineKind::Uci(c) if c == &["stockfish"]));

        let named = parse_uci_flag("sf-big=/opt/sf/stockfish --threads 4").unwrap();
        assert_eq!(named.label, "sf-big");
        assert!(
            matches!(&named.kind, EngineKind::Uci(c) if c == &["/opt/sf/stockfish", "--threads", "4"])
        );

        // An '=' inside an argument is not a name separator.
        let lc0 = parse_uci_flag("lc0 --weights=net.pb").unwrap();
        assert_eq!(lc0.label, "lc0");
        assert!(matches!(&lc0.kind, EngineKind::Uci(c) if c == &["lc0", "--weights=net.pb"]));

        assert!(parse_uci_flag("   ").is_err());
    }

    #[test]
    fn white_score_flips_for_black() {
        assert_eq!(white_score(Color::White, 50), (Some(50), None));
        assert_eq!(white_score(Color::Black, 50), (Some(-50), None));
        let mate3 = chess::eval::MATE - 5; // mate in 3 plies -> 3 moves
        assert_eq!(white_score(Color::Black, mate3).1, Some(-3));
    }
}
