//! Opening Trainer — a "play like Stockfish" drill.
//!
//!   cargo run --release --bin chess-trainer
//!   cargo run --release --bin chess-trainer -- --port 9000 --engine "stockfish" --hash 256
//!
//! You start from move 1 and play one side. The opponent walks the main
//! theory of an embedded opening book; every move *you* play is graded by a
//! Stockfish engine (centipawn loss versus its own best move) and the browser
//! rewards you for behaving like the engine. Std-only, like the rest of the
//! crate: a hand-rolled HTTP/1.1 server (shared with chess-web) drives a warm
//! Stockfish subprocess and answers a single-page frontend over plain JSON.
//!
//! Endpoints (all GET, positions are stateless: `moves` = space/comma UCI):
//!   GET /                 the app (embedded index.html / app.js / style.css)
//!   GET /api/state        position, legal moves, outcome, book/opening info
//!   GET /api/grade        judge one move: cp-loss, grade, best move, book?
//!   GET /api/reply        the opponent's move: book main line, else SF best
//!   GET /api/eval         current eval + best move (the "hint" button)

// The HTTP and UCI plumbing is identical to chess-web's; share the source
// rather than fork it. (The trainer doesn't use the SSE helpers, hence the
// allow.)
#[path = "../chess-web/http.rs"]
#[allow(dead_code)]
mod http;
#[path = "../chess-web/uci.rs"]
#[allow(dead_code)]
mod uci;

mod book;

use chess::{Board, Color, Game, Move, Outcome, PieceType};
use http::{
    Request, jarr, jopt, jstr, read_request, respond, respond_bad_request, respond_json,
};
use std::io::BufReader;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Mutex;
use std::sync::OnceLock;
use std::thread;
use uci::{GoParams, UciEngine, UciInfo};

const INDEX_HTML: &str = include_str!("index.html");
const APP_JS: &str = include_str!("app.js");
const STYLE_CSS: &str = include_str!("style.css");

struct Config {
    /// External engine command line (default: `stockfish`).
    engine_cmd: Vec<String>,
    hash_mb: usize,
}

/// The single warm engine, kept alive between requests so its hash carries
/// over. Taken whole for the duration of a request (the trainer issues one
/// request at a time per move) and returned afterwards.
static ENGINE_POOL: Mutex<Option<UciEngine>> = Mutex::new(None);

fn take_engine(config: &Config) -> Result<UciEngine, String> {
    if let Ok(mut slot) = ENGINE_POOL.lock()
        && let Some(mut engine) = slot.take()
        && engine.sync().is_ok()
    {
        return Ok(engine); // a dead process falls through and respawns
    }
    UciEngine::spawn(&config.engine_cmd, config.hash_mb)
}

fn return_engine(engine: UciEngine) {
    if let Ok(mut slot) = ENGINE_POOL.lock() {
        *slot = Some(engine);
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

fn config() -> &'static Config {
    CONFIG.get_or_init(|| Config { engine_cmd: vec!["stockfish".to_string()], hash_mb: 128 })
}

fn main() {
    let mut port: u16 = 8001;
    let mut engine_cmd: Option<Vec<String>> = None;
    let mut hash_mb: usize = 128;

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
            "--engine" => match args.next() {
                Some(cmd) => {
                    let parts: Vec<String> =
                        cmd.split_whitespace().map(|s| s.to_string()).collect();
                    if parts.is_empty() {
                        die("--engine needs a command");
                    }
                    engine_cmd = Some(parts);
                }
                None => die("--engine needs a command (UCI engine, e.g. \"stockfish\")"),
            },
            "--help" | "-h" => {
                println!(
                    "usage: chess-trainer [--port N] [--hash MiB] [--engine \"cmd args\"]\n\
                     Defaults to a `stockfish` on PATH as the judge and opponent fallback."
                );
                return;
            }
            other => die(&format!("unknown flag '{other}' (try --help)")),
        }
    }
    hash_mb = hash_mb.clamp(1, 4096);

    let engine_cmd = engine_cmd.unwrap_or_else(|| vec!["stockfish".to_string()]);
    if !engine_on_path(&engine_cmd[0]) {
        die(&format!(
            "engine '{}' not found on PATH.\n  Install Stockfish (e.g. `brew install stockfish`) \
             or pass --engine \"/path/to/engine\".",
            engine_cmd[0]
        ));
    }

    // Initialize the shared config and fail fast on a bad book / dead engine.
    let _ = CONFIG.set(Config { engine_cmd: engine_cmd.clone(), hash_mb });
    let openings = book::openings();
    match take_engine(config()) {
        Ok(e) => return_engine(e),
        Err(e) => die(&format!("cannot start engine: {e}")),
    }

    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => die(&format!("cannot bind 127.0.0.1:{port}: {e}")),
    };
    println!("chess-trainer: opening drill at http://127.0.0.1:{port}/");
    println!("  judge/opponent engine: {}", engine_cmd.join(" "));
    println!("  opening book: {} lines", openings.len());

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        thread::spawn(move || handle_connection(stream));
    }
}

fn die(msg: &str) -> ! {
    eprintln!("chess-trainer: {msg}");
    std::process::exit(2);
}

fn engine_on_path(name: &str) -> bool {
    // Absolute / relative paths are checked directly.
    if name.contains('/') {
        return std::path::Path::new(name).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

fn handle_connection(stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(read_half);
    let mut stream = stream;
    let Ok(req) = read_request(&mut reader) else {
        return;
    };

    let cfg = config();
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
        ("GET", "/api/state") => json_or_400(&mut stream, handle_state(&req)),
        ("GET", "/api/grade") => json_or_400(&mut stream, handle_grade(&req, cfg)),
        ("GET", "/api/reply") => json_or_400(&mut stream, handle_reply(&req, cfg)),
        ("GET", "/api/eval") => json_or_400(&mut stream, handle_eval(&req, cfg)),
        _ => respond(&mut stream, "404 Not Found", "text/plain", b"not found"),
    };
    let _ = stream.shutdown(Shutdown::Both);
}

fn json_or_400(stream: &mut TcpStream, r: Result<String, String>) -> std::io::Result<()> {
    match r {
        Ok(json) => respond_json(stream, &json),
        Err(e) => respond_bad_request(stream, &e),
    }
}

// --- position plumbing (always from the start position) ---

/// Parse the `moves` query (space/comma separated UCI) into a validated game.
fn game_from_moves(req: &Request) -> Result<(Game, Vec<String>), String> {
    let mut game = Game::new();
    let mut ucis = Vec::new();
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
        ucis.push(mv.to_uci());
        game.push(mv);
    }
    Ok((game, ucis))
}

/// `position startpos moves ...` for a UCI engine.
fn uci_position(ucis: &[String]) -> String {
    if ucis.is_empty() {
        "position startpos".to_string()
    } else {
        format!("position startpos moves {}", ucis.join(" "))
    }
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

// --- scoring helpers ---

/// A monotone, POV-symmetric scalar from a (cp, mate) score. Mate scores
/// dominate centipawns; a sooner mate is worth more. Negating the inputs
/// negates the output, so a position evaluated from the opponent's side
/// flips cleanly with a single sign change.
fn score_value(cp: Option<i32>, mate: Option<i32>) -> i64 {
    match mate {
        Some(m) if m > 0 => 1_000_000 - m as i64,
        Some(m) => -1_000_000 - m as i64, // m < 0: being mated; sooner = worse
        None => cp.unwrap_or(0) as i64,
    }
}

/// (cp, mate) as seen from White, given the side that just moved was `mover`.
fn to_white(mover: Color, cp: Option<i32>, mate: Option<i32>) -> (Option<i32>, Option<i32>) {
    // A move by `mover` leaves `mover`'s opponent to move; (cp, mate) here are
    // already in `mover`'s POV. Flip to White if mover is Black.
    let sign = if mover == Color::White { 1 } else { -1 };
    (cp.map(|v| sign * v), mate.map(|v| sign * v))
}

fn piece_val(pt: PieceType) -> i32 {
    match pt {
        PieceType::Pawn => 100,
        PieceType::Knight => 320,
        PieceType::Bishop => 330,
        PieceType::Rook => 500,
        PieceType::Queen => 900,
        PieceType::King => 20_000,
    }
}

/// A best move that invests material the engine still likes: a (minor-or-more)
/// piece left capturable on its destination square for a net material deficit,
/// while the post-move eval stays in the mover's favour. A deliberately
/// conservative heuristic — it only ever upgrades a reward, never a penalty.
fn is_brilliant(before: &Board, played: Move, played_value_mover: i64) -> bool {
    if played_value_mover < 50 {
        return false; // engine must still clearly favour the mover
    }
    let Some(moved) = before.piece_at(played.from()) else {
        return false;
    };
    let v_moved = piece_val(moved.piece_type);
    if v_moved < 300 {
        return false; // ignore pawn "sacs": too noisy to call brilliant
    }
    let v_cap = if played.is_capture() {
        before.piece_at(played.to()).map(|p| piece_val(p.piece_type)).unwrap_or(0)
    } else {
        0
    };
    let mut after = before.clone();
    after.make_move(played);
    // Can the opponent capture the piece we just moved, on its new square?
    let recapturable = after.legal_moves().iter().any(|&m| {
        m.to() == played.to() && m.is_capture() && after.piece_at(m.from()).is_some()
    });
    // We invest at least ~2 points of material that the opponent can grab.
    recapturable && (v_cap - v_moved) <= -200
}

fn piece_name(pt: PieceType) -> &'static str {
    match pt {
        PieceType::Pawn => "pawn",
        PieceType::Knight => "knight",
        PieceType::Bishop => "bishop",
        PieceType::Rook => "rook",
        PieceType::Queen => "queen",
        PieceType::King => "king",
    }
}

/// Search limits from the request. A `depth` (fixed-depth, stable cp-loss)
/// takes precedence over `movetime`; a generous wall-clock cap keeps a
/// pathological position from hanging the server.
fn go_limits(multipv: usize, depth: i32, movetime: u64) -> GoParams {
    if depth > 0 {
        GoParams { multipv, depth, movetime: 4_000 }
    } else {
        GoParams { multipv, depth: 0, movetime }
    }
}

/// A human reason a sub-par move is sub-par, framed as the opponent's
/// *punishment* (so it teaches the lesson without revealing the move the
/// trainee should have found). `after` is the position after the played move;
/// `refute` is the engine's best line from there (UCI), opponent to move.
fn build_reason(after: &Board, refute: &[String], grade: &str, cp_loss: i64) -> String {
    if !matches!(grade, "inaccuracy" | "mistake" | "blunder") {
        return String::new();
    }
    let Some(first) = refute.first().and_then(|u| after.parse_uci(u)) else {
        return "It quietly worsens your position.".to_string();
    };
    let san = after.san(first);
    let fuci = first.to_uci();
    let dest = &fuci[2..4]; // destination square, e.g. "d2"
    let victim = after.piece_at(first.to()).map(|p| p.piece_type);
    let mut nb = after.clone();
    nb.make_move(first);
    let gives_check = nb.in_check();

    if first.is_capture() {
        match victim {
            Some(pt) if piece_val(pt) >= 100 => {
                format!("Allows {san} — winning your {} on {dest}.", piece_name(pt))
            }
            _ => format!("Allows {san}, winning material."),
        }
    } else if gives_check {
        format!("Allows {san} — your king gets exposed.")
    } else if cp_loss > 300 {
        format!("Lets the opponent in with {san} — a near-winning initiative.")
    } else {
        format!("Passive — {san} gives the opponent an easy, comfortable game.")
    }
}

// --- /api/state ---

fn handle_state(req: &Request) -> Result<String, String> {
    let (game, ucis) = game_from_moves(req)?;
    let board = game.board();

    let legal = jarr(board.legal_moves().iter().map(|&mv| {
        format!("{{\"uci\":{},\"san\":{}}}", jstr(&mv.to_uci()), jstr(&board.san(mv)))
    }));
    let last_move = ucis.last().map_or("null".to_string(), |u| jstr(u));

    let opening = book::opening_name(&ucis);
    let in_book = book::book_reply(&ucis).is_some();

    Ok(format!(
        "{{\"moves\":{},\"ply\":{},\"fen\":{},\"side\":{},\"check\":{},\"lastMove\":{},\
          \"outcome\":{},\"legal\":{},\"opening\":{},\"inBook\":{}}}",
        jarr(ucis.iter().map(|u| jstr(u))),
        ucis.len(),
        jstr(&board.to_fen()),
        jstr(side_char(board.side_to_move())),
        board.in_check(),
        last_move,
        outcome_json(&game),
        legal,
        opening.map_or("null".to_string(), |s| jstr(&s)),
        in_book,
    ))
}

// --- /api/grade ---

fn handle_grade(req: &Request, cfg: &Config) -> Result<String, String> {
    let (game, ucis) = game_from_moves(req)?;
    if game.outcome().is_over() {
        return Err("game is already over".to_string());
    }
    let board = game.board().clone();
    let mover = board.side_to_move();
    let move_uci = req.param("move").ok_or("missing 'move'")?;
    let played = board
        .parse_uci(move_uci)
        .filter(|&m| board.legal_moves().contains(m))
        .ok_or_else(|| format!("illegal move '{move_uci}'"))?;
    let played_uci = played.to_uci();
    let played_san = board.san(played);

    let movetime: u64 = req.num("movetime", 500, 100, 8_000);
    let depth: i32 = req.num("depth", 0, 0, 30);
    let multipv: usize = req.num("multipv", 3, 1, 5);

    let mut engine = take_engine(cfg).map_err(|e| format!("engine: {e}"))?;
    let result = grade_inner(&mut engine, &board, &ucis, played, depth, movetime, multipv);
    // A broken engine process is dropped (not returned to the pool) so the
    // next request respawns it.
    if result.is_ok() {
        return_engine(engine);
    }
    let g = result?;

    // The position *after* the move (for the move list / outcome reward, and
    // for explaining the refutation in human terms).
    let mut after_board = board.clone();
    after_board.make_move(played);
    let mut after = game;
    after.push(played);
    let in_book = book::is_book_line(&ucis, &played_uci);

    let reason = build_reason(&after_board, &g.refute, g.grade, g.cp_loss);
    let (eval_w_cp, eval_w_mate) = to_white(mover, g.played_cp, g.played_mate);

    // Note: the engine's *preferred* move is deliberately NOT returned — the
    // trainee learns from `reason` (the punishment), not the answer.
    Ok(format!(
        "{{\"grade\":{},\"cpLoss\":{},\"matched\":{},\"book\":{},\"playedSan\":{},\
          \"evalCp\":{},\"evalMate\":{},\"reason\":{},\"outcome\":{}}}",
        jstr(g.grade),
        g.cp_loss,
        g.matched,
        in_book,
        jstr(&played_san),
        jopt(eval_w_cp),
        jopt(eval_w_mate),
        jstr(&reason),
        outcome_json(&after),
    ))
}

struct Grade {
    grade: &'static str,
    cp_loss: i64,
    matched: bool,
    played_cp: Option<i32>,
    played_mate: Option<i32>,
    /// The engine's best line from the position after the played move (UCI,
    /// opponent to move) — the refutation, used only to explain the mistake.
    refute: Vec<String>,
}

fn grade_inner(
    engine: &mut UciEngine,
    board: &Board,
    ucis: &[String],
    played: Move,
    depth: i32,
    movetime: u64,
    multipv: usize,
) -> Result<Grade, String> {
    use std::collections::BTreeMap;

    // Analyse the position the move was played from (mover's POV).
    let mut infos: BTreeMap<usize, UciInfo> = BTreeMap::new();
    engine.search(&uci_position(ucis), &go_limits(multipv, depth, movetime), |i| {
        infos.insert(i.multipv, i.clone());
        true
    })?;

    let best = infos.get(&1).cloned().ok_or("engine returned no analysis")?;
    let best_uci = best
        .pv
        .first()
        .and_then(|s| board.parse_uci(s))
        .map(|m| m.to_uci())
        .unwrap_or_default();
    let best_value = score_value(best.cp, best.mate);

    let played_uci = played.to_uci();
    let matched = played_uci == best_uci;

    // The played move's eval (mover's POV) and the opponent's refutation line.
    // If the move is one of the multipv lines, its PV already contains the
    // continuation (drop the move itself); otherwise search the resulting
    // position and flip the sign.
    let played_info = infos.values().find(|i| {
        i.pv.first().and_then(|s| board.parse_uci(s)).map(|m| m.to_uci()) == Some(played_uci.clone())
    });
    let (played_cp, played_mate, played_value, refute) = if let Some(info) = played_info {
        let refute = info.pv.iter().skip(1).cloned().collect();
        (info.cp, info.mate, score_value(info.cp, info.mate), refute)
    } else {
        let mut child = ucis.to_vec();
        child.push(played_uci.clone());
        let mut last: Option<UciInfo> = None;
        engine.search(&uci_position(&child), &go_limits(1, depth, movetime), |i| {
            if i.multipv <= 1 {
                last = Some(i.clone());
            }
            true
        })?;
        match last {
            // child position is the opponent's POV; negate to mover's POV.
            Some(i) => (i.cp.map(|c| -c), i.mate.map(|m| -m), -score_value(i.cp, i.mate), i.pv),
            None => (None, None, best_value, Vec::new()), // terminal after move
        }
    };

    let cp_loss = (best_value - played_value).max(0).min(100_000);

    // Tightened bands: in the opening, good moves cluster within ~30cp of best,
    // so the windows are deliberately strict (cp-loss is stable at fixed depth).
    let mut grade = if matched || cp_loss <= 8 {
        "best"
    } else if cp_loss <= 25 {
        "excellent"
    } else if cp_loss <= 50 {
        "great"
    } else if cp_loss <= 90 {
        "good"
    } else if cp_loss <= 150 {
        "inaccuracy"
    } else if cp_loss <= 300 {
        "mistake"
    } else {
        "blunder"
    };
    if matched && is_brilliant(board, played, played_value) {
        grade = "brilliant";
    }

    Ok(Grade { grade, cp_loss, matched, played_cp, played_mate, refute })
}

// --- /api/reply (opponent move) ---

fn handle_reply(req: &Request, cfg: &Config) -> Result<String, String> {
    let (game, ucis) = game_from_moves(req)?;
    if game.outcome().is_over() {
        return Ok(format!("{{\"over\":true,\"outcome\":{}}}", outcome_json(&game)));
    }
    let board = game.board();

    // In book: play the main-line continuation.
    if let Some(reply) = book::book_reply(&ucis)
        && let Some(mv) = board.parse_uci(&reply.uci)
        && board.legal_moves().contains(mv)
    {
        return Ok(format!(
            "{{\"uci\":{},\"san\":{},\"book\":true,\"opening\":{},\"over\":false}}",
            jstr(&mv.to_uci()),
            jstr(&board.san(mv)),
            jstr(&reply.name),
        ));
    }

    // Out of book: Stockfish picks for the opponent.
    let movetime: u64 = req.num("movetime", 500, 100, 8_000);
    let depth: i32 = req.num("depth", 0, 0, 30);
    let mut engine = take_engine(cfg).map_err(|e| format!("engine: {e}"))?;
    let best = engine.search(&uci_position(&ucis), &go_limits(1, depth, movetime), |_| true);
    let best = match best {
        Ok(b) => {
            return_engine(engine);
            b
        }
        Err(e) => return Err(format!("engine: {e}")),
    };
    let mv = best
        .and_then(|b| board.parse_uci(&b))
        .filter(|&m| board.legal_moves().contains(m))
        .ok_or("engine returned no legal move")?;
    Ok(format!(
        "{{\"uci\":{},\"san\":{},\"book\":false,\"opening\":null,\"over\":false}}",
        jstr(&mv.to_uci()),
        jstr(&board.san(mv)),
    ))
}

// --- /api/eval (the hint / pre-move peek) ---

fn handle_eval(req: &Request, cfg: &Config) -> Result<String, String> {
    let (game, ucis) = game_from_moves(req)?;
    if game.outcome().is_over() {
        return Ok(format!("{{\"over\":true,\"outcome\":{}}}", outcome_json(&game)));
    }
    let board = game.board().clone();
    let mover = board.side_to_move();
    let movetime: u64 = req.num("movetime", 400, 100, 8_000);
    let depth: i32 = req.num("depth", 0, 0, 30);

    let mut engine = take_engine(cfg).map_err(|e| format!("engine: {e}"))?;
    let mut last: Option<UciInfo> = None;
    let res = engine.search(
        &uci_position(&ucis),
        &go_limits(1, depth, movetime),
        |i| {
            if i.multipv <= 1 {
                last = Some(i.clone());
            }
            true
        },
    );
    let best = match res {
        Ok(b) => {
            return_engine(engine);
            b
        }
        Err(e) => return Err(format!("engine: {e}")),
    };
    let best_mv = best.and_then(|b| board.parse_uci(&b));
    let (best_uci, best_san) = match best_mv {
        Some(m) => (jstr(&m.to_uci()), jstr(&board.san(m))),
        None => ("null".to_string(), "null".to_string()),
    };
    let (cp, mate) = match &last {
        Some(i) => to_white(mover, i.cp, i.mate),
        None => (None, None),
    };
    Ok(format!(
        "{{\"over\":false,\"cp\":{},\"mate\":{},\"best\":{{\"uci\":{},\"san\":{}}}}}",
        jopt(cp),
        jopt(mate),
        best_uci,
        best_san,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(query: &[(&str, &str)]) -> Request {
        Request {
            method: "GET".to_string(),
            path: "/api/state".to_string(),
            query: query.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            body: Vec::new(),
        }
    }

    #[test]
    fn state_reports_book_and_opening() {
        let json = handle_state(&req(&[("moves", "e2e4 e7e5 g1f3 b8c6 f1b5")])).unwrap();
        assert!(json.contains("\"inBook\":true"), "{json}");
        assert!(json.contains("Ruy"), "{json}");
        assert!(json.contains("\"side\":\"b\""), "{json}");
    }

    #[test]
    fn state_out_of_book_after_offbeat_move() {
        // 1. a3 is not in any book line.
        let json = handle_state(&req(&[("moves", "a2a3")])).unwrap();
        assert!(json.contains("\"inBook\":false"), "{json}");
    }

    #[test]
    fn score_value_is_pov_symmetric() {
        assert_eq!(score_value(Some(50), None), -score_value(Some(-50), None));
        // mate in 2 beats mate in 5; flipping signs flips the order.
        assert!(score_value(None, Some(2)) > score_value(None, Some(5)));
        assert_eq!(score_value(None, Some(3)), -score_value(None, Some(-3)));
    }

    #[test]
    fn to_white_flips_for_black_mover() {
        assert_eq!(to_white(Color::White, Some(40), None), (Some(40), None));
        assert_eq!(to_white(Color::Black, Some(40), None), (Some(-40), None));
    }

    #[test]
    fn brilliant_only_for_a_winning_material_offer() {
        // A safe developing move (Ng1-f3) is not a sacrifice: nothing can
        // capture the knight, so it is never "brilliant".
        let start = Board::startpos();
        let dev = start.parse_uci("g1f3").unwrap();
        assert!(!is_brilliant(&start, dev, 100));

        // Nc3xd5 gives a knight (320) for a pawn (100); the queen can recapture
        // on d5 (net -220). With the engine still favouring the mover, that is
        // the kind of sound investment we celebrate.
        let board = Board::from_fen("rnbqkbnr/ppp1pppp/8/3p4/8/2N5/PPPPPPPP/R1BQKBNR w KQkq - 0 1")
            .unwrap();
        let sac = board.parse_uci("c3d5").unwrap();
        assert!(is_brilliant(&board, sac, 120));
        // But not if the engine says the mover is worse afterwards.
        assert!(!is_brilliant(&board, sac, -50));
    }
}
