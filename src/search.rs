//! The search: iterative-deepening negamax with alpha-beta, a transposition
//! table, quiescence, move ordering, null-move pruning, late-move reductions,
//! check extensions, and aspiration windows.
//!
//! The engine is generic over the [`Evaluator`], so the same search drives the
//! handcrafted eval today and an NNUE evaluator later.

use crate::board::Board;
use crate::eval::{
    DRAW, Evaluator, HandcraftedEval, INFINITY, MATE, MATE_IN_MAX, MAX_PLY, is_mate,
};
use crate::moves::{Move, MoveList};
use crate::tt::{Bound, Tt};
use crate::types::{Color, PieceType};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Piece values for MVV-LVA move ordering (P,N,B,R,Q,K).
const PIECE_VALUE: [i32; 6] = [100, 320, 330, 500, 900, 10000];

/// Search limits. Construct with the helpers, or set fields directly.
#[derive(Clone, Default)]
pub struct Limits {
    pub depth: Option<i32>,
    pub nodes: Option<u64>,
    pub movetime: Option<u64>,
    pub wtime: Option<u64>,
    pub btime: Option<u64>,
    pub winc: Option<u64>,
    pub binc: Option<u64>,
    pub movestogo: Option<u32>,
    pub infinite: bool,
}

impl Limits {
    pub fn depth(d: i32) -> Limits {
        Limits {
            depth: Some(d),
            ..Default::default()
        }
    }
    pub fn movetime(ms: u64) -> Limits {
        Limits {
            movetime: Some(ms),
            ..Default::default()
        }
    }
    pub fn nodes(n: u64) -> Limits {
        Limits {
            nodes: Some(n),
            ..Default::default()
        }
    }
    pub fn infinite() -> Limits {
        Limits {
            infinite: true,
            ..Default::default()
        }
    }
}

/// Per-iteration search information (for UCI `info` output).
#[derive(Clone, Debug)]
pub struct SearchInfo {
    pub depth: i32,
    pub seldepth: i32,
    pub score: i32,
    pub nodes: u64,
    pub time_ms: u128,
    pub nps: u64,
    pub hashfull: usize,
    pub pv: Vec<Move>,
}

/// The final result of a search.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub best_move: Move,
    pub ponder: Option<Move>,
    pub score: i32,
    pub depth: i32,
    pub seldepth: i32,
    pub nodes: u64,
    pub time_ms: u128,
    pub pv: Vec<Move>,
}

/// Boxed because the history and PV tables are large (~96 KiB).
struct SearchData {
    killers: [[Move; 2]; MAX_PLY],
    /// History heuristic, indexed `[color][from][to]`.
    history: [[[i32; 64]; 64]; 2],
    /// Triangular principal-variation table.
    pv: [[Move; MAX_PLY]; MAX_PLY],
    pv_len: [usize; MAX_PLY],
}

impl SearchData {
    fn boxed() -> Box<SearchData> {
        Box::new(SearchData {
            killers: [[Move::NONE; 2]; MAX_PLY],
            history: [[[0; 64]; 64]; 2],
            pv: [[Move::NONE; MAX_PLY]; MAX_PLY],
            pv_len: [0; MAX_PLY],
        })
    }

    fn clear_heuristics(&mut self) {
        self.killers = [[Move::NONE; 2]; MAX_PLY];
        self.history = [[[0; 64]; 64]; 2];
    }
}

/// A chess analysis engine.
pub struct Engine<E: Evaluator = HandcraftedEval> {
    tt: Tt,
    eval: E,
    data: Box<SearchData>,
    stop: Arc<AtomicBool>,
    /// Position keys preceding the search root (for repetition detection).
    root_history: Vec<u64>,

    // Per-search scratch.
    nodes: u64,
    seldepth: i32,
    stopped: bool,
    start: Instant,
    soft_deadline: Option<Instant>,
    hard_deadline: Option<Instant>,
    node_limit: Option<u64>,
    /// Position keys along the current search path (root_history + tree).
    path: Vec<u64>,
}

impl Default for Engine<HandcraftedEval> {
    fn default() -> Self {
        Engine::new()
    }
}

impl Engine<HandcraftedEval> {
    /// A new engine with the handcrafted evaluator and a 16 MiB table.
    pub fn new() -> Engine<HandcraftedEval> {
        Engine::with_eval_and_tt(HandcraftedEval::new(), 16)
    }
}

impl<E: Evaluator> Engine<E> {
    pub fn with_eval_and_tt(eval: E, tt_mb: usize) -> Engine<E> {
        Engine {
            tt: Tt::new(tt_mb),
            eval,
            data: SearchData::boxed(),
            stop: Arc::new(AtomicBool::new(false)),
            root_history: Vec::new(),
            nodes: 0,
            seldepth: 0,
            stopped: false,
            start: Instant::now(),
            soft_deadline: None,
            hard_deadline: None,
            node_limit: None,
            path: Vec::new(),
        }
    }

    /// Shared stop flag — set it from another thread to abort the search.
    pub fn stop_handle(&self) -> Arc<AtomicBool> {
        self.stop.clone()
    }

    /// Resize the transposition table (MiB) and clear it.
    pub fn resize_tt(&mut self, mb: usize) {
        self.tt.resize(mb);
    }

    /// Clear the transposition table and search heuristics (UCI `ucinewgame`).
    pub fn new_game(&mut self) {
        self.tt.clear();
        self.data.clear_heuristics();
    }

    /// Seed the positions that preceded the root (their Zobrist keys), so the
    /// search can recognise repetitions with the actual game history.
    pub fn set_history(&mut self, keys: &[u64]) {
        self.root_history.clear();
        self.root_history.extend_from_slice(keys);
    }

    /// Analyze `board` to the given limits, ignoring per-iteration info.
    pub fn analyze(&mut self, board: &Board, limits: &Limits) -> Analysis {
        self.analyze_with(board, limits, |_| {})
    }

    /// Analyze `board`, invoking `on_info` after each completed depth.
    pub fn analyze_with(
        &mut self,
        board: &Board,
        limits: &Limits,
        mut on_info: impl FnMut(&SearchInfo),
    ) -> Analysis {
        self.prepare(board, limits);
        let mut root = board.clone();

        let mut best_move = Move::NONE;
        let mut best_score = 0;
        let mut last_pv: Vec<Move> = Vec::new();
        let max_depth = limits.depth.unwrap_or(MAX_PLY as i32 - 2).min(MAX_PLY as i32 - 2);

        let mut prev_score = 0;
        for depth in 1..=max_depth {
            let score = self.search_root(&mut root, depth, prev_score);

            // An aborted iteration's results are unreliable past depth 1.
            if self.stopped && depth > 1 {
                break;
            }

            best_score = score;
            prev_score = score;
            last_pv = self.collect_pv();
            if let Some(&mv) = last_pv.first() {
                best_move = mv;
            }

            let info = self.make_info(depth, score, &last_pv);
            on_info(&info);

            // Stop conditions between iterations.
            if let Some(sd) = self.soft_deadline
                && Instant::now() >= sd
            {
                break;
            }
            if is_mate(score) {
                // Found a forced mate; no need to search deeper.
                break;
            }
        }

        // Fallback: ensure we always return a legal move.
        if best_move == Move::NONE {
            let moves = root.legal_moves();
            if !moves.is_empty() {
                best_move = moves[0];
            }
        }

        Analysis {
            best_move,
            ponder: last_pv.get(1).copied(),
            score: best_score,
            depth: max_depth.min(self.completed_depth()),
            seldepth: self.seldepth,
            nodes: self.nodes,
            time_ms: self.start.elapsed().as_millis(),
            pv: last_pv,
        }
    }

    fn completed_depth(&self) -> i32 {
        self.data.pv_len[0] as i32
    }

    fn prepare(&mut self, board: &Board, limits: &Limits) {
        self.nodes = 0;
        self.seldepth = 0;
        self.stopped = false;
        // NOTE: we deliberately do NOT clear the shared `stop` atomic here. The
        // search runs on a worker thread; the *driver* clears it before starting
        // (the UCI loop does so on the main thread before spawning). Clearing it
        // here would race with a `stop` arriving immediately after `go` and could
        // silently drop it (hanging an otherwise-unbounded `go infinite`).
        self.start = Instant::now();
        self.node_limit = limits.nodes;
        self.tt.new_generation();
        self.eval.refresh(board);
        // `path` holds the keys of every position up to and INCLUDING the
        // current node; seed it with the game history plus the root position.
        self.path.clear();
        self.path.extend_from_slice(&self.root_history);
        self.path.push(board.hash());

        // Time management.
        self.soft_deadline = None;
        self.hard_deadline = None;
        if let Some(mt) = limits.movetime {
            let d = Duration::from_millis(mt);
            self.soft_deadline = Some(self.start + d);
            self.hard_deadline = Some(self.start + d);
        } else if !limits.infinite {
            let (time, inc) = match board.side_to_move() {
                Color::White => (limits.wtime, limits.winc),
                Color::Black => (limits.btime, limits.binc),
            };
            if let Some(t) = time {
                let mtg = limits.movestogo.unwrap_or(30).max(1) as u64;
                let inc = inc.unwrap_or(0);
                let reserve = t.min(50);
                let alloc = (t / mtg + inc).min(t - reserve.min(t));
                let alloc = alloc.max(1);
                self.soft_deadline = Some(self.start + Duration::from_millis(alloc));
                let hard = (alloc * 4).min(t.saturating_sub(reserve)).max(alloc);
                self.hard_deadline = Some(self.start + Duration::from_millis(hard));
            }
        }
    }

    fn search_root(&mut self, board: &mut Board, depth: i32, prev: i32) -> i32 {
        if depth >= 5 {
            // Aspiration window around the previous score.
            let mut delta = 25;
            let mut alpha = (prev - delta).max(-INFINITY);
            let mut beta = (prev + delta).min(INFINITY);
            loop {
                let score = self.negamax(board, depth, alpha, beta, 0, true);
                if self.stopped {
                    return score;
                }
                if score <= alpha {
                    beta = (alpha + beta) / 2;
                    alpha = (score - delta).max(-INFINITY);
                } else if score >= beta {
                    beta = (score + delta).min(INFINITY);
                } else {
                    return score;
                }
                delta += delta / 2;
            }
        } else {
            self.negamax(board, depth, -INFINITY, INFINITY, 0, true)
        }
    }

    fn negamax(
        &mut self,
        board: &mut Board,
        mut depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: usize,
        null_ok: bool,
    ) -> i32 {
        // Ply ceiling: check extensions could otherwise drive `ply` past the
        // fixed-size killer/PV tables and panic. Bail to a static eval.
        if ply >= MAX_PLY - 1 {
            return self.eval.evaluate(board);
        }
        self.data.pv_len[ply] = 0;
        let is_root = ply == 0;
        let is_pv = beta - alpha > 1;

        // Periodic stop check.
        if self.nodes & 2047 == 0 && self.should_stop() {
            self.stopped = true;
        }
        if self.stopped {
            return 0;
        }

        if !is_root {
            // Draw detection.
            if self.is_repetition(board)
                || board.halfmove_clock() >= 100
                || board.is_insufficient_material()
            {
                return DRAW;
            }
            // Mate-distance pruning.
            alpha = alpha.max(-MATE + ply as i32);
            beta = beta.min(MATE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        let in_check = board.in_check();
        if in_check {
            depth += 1; // check extension
        }

        if depth <= 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        self.nodes += 1;
        if ply > self.seldepth as usize {
            self.seldepth = ply as i32;
        }

        // Transposition table probe.
        let key = board.hash();
        let mut tt_move = Move::NONE;
        if let Some(d) = self.tt.probe(key) {
            tt_move = d.mv;
            if !is_pv && d.depth >= depth {
                let s = score_from_tt(d.score, ply);
                match d.bound {
                    Bound::Exact => return s,
                    Bound::Lower if s >= beta => return s,
                    Bound::Upper if s <= alpha => return s,
                    _ => {}
                }
            }
        }

        let static_eval = self.eval.evaluate(board);

        // Null-move pruning. `null_ok` forbids two nulls in a row: a double null
        // restores the original Zobrist key while inflating the half-move clock,
        // which would make `is_repetition` report a spurious draw.
        if null_ok
            && !is_pv
            && !in_check
            && depth >= 3
            && static_eval >= beta
            && board.has_non_pawn_material(board.side_to_move())
        {
            let r = 2 + depth / 4;
            let undo = board.make_null_move();
            self.eval.on_make(board, Move::NONE);
            self.path.push(board.hash());
            let score = -self.negamax(board, depth - 1 - r, -beta, -beta + 1, ply + 1, false);
            self.path.pop();
            board.unmake_null_move(undo);
            self.eval.on_unmake(board, Move::NONE);
            if self.stopped {
                return 0;
            }
            if score >= beta {
                return if is_mate(score) { beta } else { score };
            }
        }

        let moves = board.legal_moves();
        if moves.is_empty() {
            return if in_check {
                -MATE + ply as i32 // checkmated
            } else {
                DRAW // stalemate
            };
        }

        let stm = board.side_to_move().index();
        let ordered = self.order(board, &moves, tt_move, ply);
        let n = moves.len();

        let mut best_score = -INFINITY;
        let mut best_move = Move::NONE;
        let mut bound = Bound::Upper;

        for (i, &mv) in ordered[..n].iter().enumerate() {
            let is_quiet = !mv.is_capture() && !mv.is_promotion();

            let undo = board.make_move(mv);
            self.eval.on_make(board, mv);
            self.path.push(board.hash());

            // Late-move reductions for late quiet moves.
            let mut reduction = 0;
            if depth >= 3 && i >= 4 && is_quiet && !in_check {
                reduction = lmr(depth, i);
                if is_pv {
                    reduction = (reduction - 1).max(0);
                }
            }

            let new_depth = depth - 1;
            let score = if i == 0 {
                -self.negamax(board, new_depth, -beta, -alpha, ply + 1, true)
            } else {
                // Principal-variation search with a null window (+ LMR).
                let mut s =
                    -self.negamax(board, new_depth - reduction, -alpha - 1, -alpha, ply + 1, true);
                if s > alpha && reduction > 0 {
                    s = -self.negamax(board, new_depth, -alpha - 1, -alpha, ply + 1, true);
                }
                if s > alpha && s < beta {
                    s = -self.negamax(board, new_depth, -beta, -alpha, ply + 1, true);
                }
                s
            };

            self.path.pop();
            board.unmake_move(mv, undo);
            self.eval.on_unmake(board, mv);

            if self.stopped {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = mv;
                if score > alpha {
                    alpha = score;
                    bound = Bound::Exact;
                    self.update_pv(ply, mv);
                    if score >= beta {
                        bound = Bound::Lower;
                        if is_quiet {
                            self.update_killers(ply, mv);
                            self.update_history(stm, mv, depth);
                        }
                        break;
                    }
                }
            }
        }

        self.tt
            .store(key, best_move, score_to_tt(best_score, ply), depth, bound);
        best_score
    }

    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        if self.nodes & 2047 == 0 && self.should_stop() {
            self.stopped = true;
        }
        if self.stopped {
            return 0;
        }
        self.nodes += 1;
        if ply > self.seldepth as usize {
            self.seldepth = ply as i32;
        }
        if ply >= MAX_PLY - 1 {
            return self.eval.evaluate(board);
        }

        let in_check = board.in_check();
        let mut scored: [(i32, Move); 256] = [(0, Move::NONE); 256];
        let mut count = 0;
        let mut best;

        if in_check {
            // In check there is no "stand pat" — all legal evasions must be
            // searched, and no legal move means checkmate.
            let moves = board.legal_moves();
            if moves.is_empty() {
                return -MATE + ply as i32;
            }
            let stm = board.side_to_move().index();
            for &mv in moves.iter() {
                scored[count] = (self.move_score(board, mv, Move::NONE, ply, stm), mv);
                count += 1;
            }
            best = -INFINITY;
        } else {
            let stand_pat = self.eval.evaluate(board);
            if stand_pat >= beta {
                return stand_pat;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
            best = stand_pat;
            // Tactical moves only.
            for &mv in board.legal_moves().iter() {
                if mv.is_capture() || mv.is_promotion() {
                    scored[count] = (self.capture_score(board, mv), mv);
                    count += 1;
                }
            }
        }
        scored[..count].sort_unstable_by_key(|x| core::cmp::Reverse(x.0));

        for &(_, mv) in &scored[..count] {
            let undo = board.make_move(mv);
            self.eval.on_make(board, mv);
            let score = -self.quiescence(board, -beta, -alpha, ply + 1);
            board.unmake_move(mv, undo);
            self.eval.on_unmake(board, mv);
            if self.stopped {
                return 0;
            }
            if score > best {
                best = score;
                if score > alpha {
                    alpha = score;
                    if score >= beta {
                        break;
                    }
                }
            }
        }
        best
    }

    // --- move ordering ---

    fn order(&self, board: &Board, moves: &MoveList, tt_move: Move, ply: usize) -> [Move; 256] {
        let stm = board.side_to_move().index();
        let mut scored: [(i32, Move); 256] = [(0, Move::NONE); 256];
        let n = moves.len();
        for i in 0..n {
            let mv = moves[i];
            scored[i] = (self.move_score(board, mv, tt_move, ply, stm), mv);
        }
        scored[..n].sort_unstable_by_key(|x| core::cmp::Reverse(x.0));
        let mut out = [Move::NONE; 256];
        for i in 0..n {
            out[i] = scored[i].1;
        }
        out
    }

    fn move_score(&self, board: &Board, mv: Move, tt_move: Move, ply: usize, stm: usize) -> i32 {
        if mv == tt_move {
            return 2_000_000;
        }
        if mv.is_capture() {
            return 1_000_000 + self.capture_score(board, mv);
        }
        if let Some(p) = mv.promotion_piece() {
            return 900_000 + PIECE_VALUE[p.index()];
        }
        if self.data.killers[ply][0] == mv {
            return 800_000;
        }
        if self.data.killers[ply][1] == mv {
            return 700_000;
        }
        self.data.history[stm][mv.from().index()][mv.to().index()]
    }

    /// MVV-LVA capture score (victim heavily weighted over attacker).
    fn capture_score(&self, board: &Board, mv: Move) -> i32 {
        let victim = if mv.is_en_passant() {
            PieceType::Pawn
        } else {
            board.piece_type_at(mv.to()).unwrap_or(PieceType::Pawn)
        };
        let attacker = board.piece_type_at(mv.from()).unwrap_or(PieceType::Pawn);
        let mut s = PIECE_VALUE[victim.index()] * 16 - PIECE_VALUE[attacker.index()];
        if let Some(p) = mv.promotion_piece() {
            s += PIECE_VALUE[p.index()];
        }
        s
    }

    // --- heuristics ---

    fn update_killers(&mut self, ply: usize, mv: Move) {
        if self.data.killers[ply][0] != mv {
            self.data.killers[ply][1] = self.data.killers[ply][0];
            self.data.killers[ply][0] = mv;
        }
    }

    fn update_history(&mut self, stm: usize, mv: Move, depth: i32) {
        let h = &mut self.data.history[stm][mv.from().index()][mv.to().index()];
        *h = (*h + depth * depth).min(600_000);
    }

    fn update_pv(&mut self, ply: usize, mv: Move) {
        self.data.pv[ply][0] = mv;
        let child_len = self.data.pv_len[ply + 1];
        for i in 0..child_len {
            self.data.pv[ply][i + 1] = self.data.pv[ply + 1][i];
        }
        self.data.pv_len[ply] = child_len + 1;
    }

    fn collect_pv(&self) -> Vec<Move> {
        let len = self.data.pv_len[0];
        self.data.pv[0][..len].to_vec()
    }

    // --- repetition / stop ---

    fn is_repetition(&self, board: &Board) -> bool {
        let n = self.path.len();
        if n < 2 {
            return false;
        }
        // The current position is `path[n-1]`; scan only its ancestors within
        // the half-move window (positions since the last irreversible move).
        let key = self.path[n - 1];
        let hm = board.halfmove_clock() as usize;
        let start = (n - 1).saturating_sub(hm);
        let mut i = n - 1;
        while i > start {
            i -= 1;
            if self.path[i] == key {
                return true;
            }
        }
        false
    }

    fn should_stop(&self) -> bool {
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        if let Some(nl) = self.node_limit
            && self.nodes >= nl
        {
            return true;
        }
        if let Some(hd) = self.hard_deadline
            && Instant::now() >= hd
        {
            return true;
        }
        false
    }

    fn make_info(&self, depth: i32, score: i32, pv: &[Move]) -> SearchInfo {
        let time_ms = self.start.elapsed().as_millis();
        let nps = (self.nodes as u128 * 1000)
            .checked_div(time_ms)
            .unwrap_or(0) as u64;
        SearchInfo {
            depth,
            seldepth: self.seldepth,
            score,
            nodes: self.nodes,
            time_ms,
            nps,
            hashfull: self.tt.hashfull(),
            pv: pv.to_vec(),
        }
    }
}

/// Late-move reduction amount (a small log-ish table).
#[inline]
fn lmr(depth: i32, move_index: usize) -> i32 {
    let d = depth as f32;
    let m = (move_index as f32) + 1.0;
    (0.75 + d.ln() * m.ln() / 2.25) as i32
}

#[inline]
fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_IN_MAX {
        score + ply as i32
    } else if score <= -MATE_IN_MAX {
        score - ply as i32
    } else {
        score
    }
}

#[inline]
fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_IN_MAX {
        score - ply as i32
    } else if score <= -MATE_IN_MAX {
        score + ply as i32
    } else {
        score
    }
}
