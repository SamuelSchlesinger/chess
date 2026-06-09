//! PUCT Monte-Carlo Tree Search — the search half of the AlphaZero-style arm.
//!
//! Generic over a [`Guide`] that supplies, for a leaf, a **value** (win estimate
//! for the side to move) and **priors** over the legal moves. Two guides ship:
//!  * [`ValueGuide`] — any value-only [`Evaluator`] with *uniform* priors (the
//!    spike; weak, because uniform priors don't focus the tree), and
//!  * [`PolicyValueNet`] — a trained policy+value net, where the priors come
//!    from the learned policy. The policy head is what makes MCTS strong, and it
//!    is trained by self-play RL with no teacher ceiling — the only principled
//!    route to *exceed* a teacher like Stockfish.
//!
//! Each simulation selects by PUCT `Q + c·P·√N_parent/(1+N_child)`, expands one
//! leaf, and backs the value up the path with a per-ply sign flip.

use crate::board::Board;
use crate::eval::{Evaluator, PolicyValueNet};
use crate::moves::Move;

/// Supplies a leaf value and move priors to the search.
pub trait Guide {
    /// `(value in [-1,1] for the side to move, priors over `moves`)`.
    fn evaluate(&mut self, board: &Board, moves: &[Move]) -> (f32, Vec<f32>);

    /// Evaluate many leaves at once. The default loops over [`Guide::evaluate`];
    /// a remote guide overrides it to pipeline all leaves in one round trip, so
    /// a GPU server can batch across leaves and across games (FINDINGS §8).
    fn evaluate_batch(&mut self, items: &[(Board, Vec<Move>)]) -> Vec<(f32, Vec<f32>)> {
        items.iter().map(|(b, m)| self.evaluate(b, m)).collect()
    }
}

/// A value-only guide: any [`Evaluator`] for the value, uniform move priors.
pub struct ValueGuide<E: Evaluator>(pub E);

impl<E: Evaluator> Guide for ValueGuide<E> {
    fn evaluate(&mut self, board: &Board, moves: &[Move]) -> (f32, Vec<f32>) {
        let v = cp_to_value(self.0.evaluate(board));
        let p = 1.0 / moves.len().max(1) as f32;
        (v, vec![p; moves.len()])
    }
}

impl Guide for PolicyValueNet {
    fn evaluate(&mut self, board: &Board, moves: &[Move]) -> (f32, Vec<f32>) {
        PolicyValueNet::evaluate(self, board, moves)
    }
}

/// Share one net (read-only) across parallel self-play threads.
impl Guide for std::sync::Arc<PolicyValueNet> {
    fn evaluate(&mut self, board: &Board, moves: &[Move]) -> (f32, Vec<f32>) {
        (**self).evaluate(board, moves)
    }
}

struct Node {
    mv: Move,
    prior: f32,
    visits: u32,
    /// Sum of backed-up values, from the side-to-move's perspective here.
    value_sum: f64,
    first_child: u32,
    n_children: u32,
    expanded: bool,
    terminal: bool,
    /// Awaiting a batched evaluation this round (batched search only).
    pending: bool,
}

impl Node {
    fn new(mv: Move, prior: f32) -> Node {
        Node {
            mv,
            prior,
            visits: 0,
            value_sum: 0.0,
            first_child: 0,
            n_children: 0,
            expanded: false,
            terminal: false,
            pending: false,
        }
    }
    #[inline]
    fn q(&self) -> f32 {
        if self.visits == 0 {
            0.0
        } else {
            (self.value_sum / self.visits as f64) as f32
        }
    }
}

/// A PUCT MCTS searcher.
pub struct Mcts<G: Guide> {
    guide: G,
    c_puct: f32,
    nodes: Vec<Node>,
}

impl<E: Evaluator> Mcts<ValueGuide<E>> {
    /// Convenience: a value-only MCTS over an [`Evaluator`] (uniform priors).
    pub fn value(eval: E) -> Mcts<ValueGuide<E>> {
        Mcts::new(ValueGuide(eval))
    }
}

impl<G: Guide> Mcts<G> {
    pub fn new(guide: G) -> Mcts<G> {
        Mcts {
            guide,
            c_puct: 1.5,
            nodes: Vec::new(),
        }
    }

    pub fn set_cpuct(&mut self, c: f32) {
        self.c_puct = c;
    }

    /// The root's mean backed-up value (side-to-move) from the last search. This
    /// `q_root` is a *search-improved* value estimate that is informative even on
    /// drawn games (unlike the binary outcome), so it is the key value-training
    /// target past the decisiveness wall (FINDINGS §5b).
    pub fn last_root_value(&self) -> f32 {
        self.nodes.first().map(|n| n.q()).unwrap_or(0.0)
    }

    /// Search from `root` for `sims` simulations; returns the most-visited move
    /// and the full visit distribution (the improved policy, for training).
    pub fn search(&mut self, root: &Board, sims: u32) -> (Move, Vec<(Move, u32)>) {
        self.search_inner(root, sims, 0.0, 0.0, 0)
    }

    /// Self-play search with **root Dirichlet exploration noise** (AlphaZero):
    /// the root priors become `(1-eps)·p + eps·Dir(alpha)`, which diversifies the
    /// opening and forces imbalanced — i.e. *decisive* — games. Without this,
    /// self-play between two copies of one net is ~93% draws and the game-outcome
    /// value target carries no signal (the decisiveness wall, FINDINGS §5b).
    pub fn search_noisy(
        &mut self,
        root: &Board,
        sims: u32,
        alpha: f32,
        eps: f32,
        seed: u64,
    ) -> (Move, Vec<(Move, u32)>) {
        self.search_inner(root, sims, alpha, eps, seed)
    }

    fn search_inner(
        &mut self,
        root: &Board,
        sims: u32,
        alpha: f32,
        eps: f32,
        seed: u64,
    ) -> (Move, Vec<(Move, u32)>) {
        self.nodes.clear();
        self.nodes.push(Node::new(Move::NONE, 1.0));
        // Expand the root up front (so the full sim budget lands on root children
        // and, with eps > 0, so we can perturb the child priors).
        let _ = self.expand_and_eval(0, root);
        if eps > 0.0 {
            let (cs, nc) = (self.nodes[0].first_child, self.nodes[0].n_children);
            if nc > 0 {
                let noise = dirichlet(nc as usize, alpha, seed);
                for (k, c) in (cs..cs + nc).enumerate() {
                    let p = self.nodes[c as usize].prior;
                    self.nodes[c as usize].prior = (1.0 - eps) * p + eps * noise[k];
                }
            }
        }
        for _ in 0..sims {
            self.simulate(root);
        }
        let r = &self.nodes[0];
        let mut best = Move::NONE;
        let mut best_n = 0u32;
        let mut dist = Vec::with_capacity(r.n_children as usize);
        for c in r.first_child..r.first_child + r.n_children {
            let ch = &self.nodes[c as usize];
            dist.push((ch.mv, ch.visits));
            if ch.visits > best_n {
                best_n = ch.visits;
                best = ch.mv;
            }
        }
        (best, dist)
    }

    /// Like [`Mcts::search_noisy`], but collects up to `max_leaves` leaves per
    /// **virtual-loss** round and evaluates them through one
    /// [`Guide::evaluate_batch`] call. With a [`RemoteGuide`] this turns K
    /// sequential net evals into one pipelined round trip, which is what lets a
    /// GPU eval server batch across leaves × games. `max_leaves <= 1` is exactly
    /// the classic (validated) search path.
    ///
    /// Virtual loss: each descent increments `visits` along its path and adds a
    /// provisional win (+1 from the child's own perspective = a loss for the
    /// parent choosing it) so concurrent descents spread across the tree; both
    /// are reconciled at backup. A descent that reaches a leaf already pending
    /// evaluation is reverted and ends the round (the tree needs results first).
    pub fn search_noisy_batched(
        &mut self,
        root: &Board,
        sims: u32,
        alpha: f32,
        eps: f32,
        seed: u64,
        max_leaves: usize,
    ) -> (Move, Vec<(Move, u32)>) {
        if max_leaves <= 1 {
            return self.search_inner(root, sims, alpha, eps, seed);
        }
        self.nodes.clear();
        self.nodes.push(Node::new(Move::NONE, 1.0));
        let _ = self.expand_and_eval(0, root);
        if eps > 0.0 {
            let (cs, nc) = (self.nodes[0].first_child, self.nodes[0].n_children);
            if nc > 0 {
                let noise = dirichlet(nc as usize, alpha, seed);
                for (k, c) in (cs..cs + nc).enumerate() {
                    let p = self.nodes[c as usize].prior;
                    self.nodes[c as usize].prior = (1.0 - eps) * p + eps * noise[k];
                }
            }
        }

        let mut done = 0u32;
        while done < sims {
            let want = max_leaves.min((sims - done) as usize);
            let mut leaves: Vec<(Board, Vec<Move>)> = Vec::with_capacity(want);
            let mut meta: Vec<(u32, Vec<u32>)> = Vec::with_capacity(want); // (node, path)

            for _ in 0..want {
                // Descend under virtual loss to an unexpanded or terminal node.
                let mut board = root.clone();
                let mut path: Vec<u32> = vec![0];
                let mut idx = 0u32;
                self.nodes[0].visits += 1;
                loop {
                    let node = &self.nodes[idx as usize];
                    if !node.expanded || node.terminal {
                        break;
                    }
                    let parent_n = node.visits as f32;
                    let (cs, nc) = (node.first_child, node.n_children);
                    let sqrt_parent = parent_n.max(1.0).sqrt();
                    let mut best_score = f32::MIN;
                    let mut best = cs;
                    for c in cs..cs + nc {
                        let ch = &self.nodes[c as usize];
                        let q = -ch.q();
                        let u = self.c_puct * ch.prior * sqrt_parent / (1.0 + ch.visits as f32);
                        if q + u > best_score {
                            best_score = q + u;
                            best = c;
                        }
                    }
                    idx = best;
                    board.make_move(self.nodes[idx as usize].mv);
                    let ch = &mut self.nodes[idx as usize];
                    ch.visits += 1;
                    ch.value_sum += 1.0; // virtual loss (win for child = loss for parent)
                    path.push(idx);
                }

                let node = &self.nodes[idx as usize];
                if node.pending {
                    // Collision with an in-flight leaf: undo and end the round.
                    self.revert_descent(&path);
                    break;
                }
                if node.terminal {
                    let v = terminal_value(&board);
                    self.backup_batched(&path, v);
                    done += 1;
                    continue;
                }
                // Fresh leaf: terminal-check it here (expansion waits for the batch).
                let moves = board.legal_moves();
                let n = &mut self.nodes[idx as usize];
                if moves.is_empty() {
                    n.terminal = true;
                    n.expanded = true;
                    let v = if board.in_check() { -1.0 } else { 0.0 };
                    self.backup_batched(&path, v);
                    done += 1;
                } else if board.halfmove_clock() >= 100 || board.is_insufficient_material() {
                    n.terminal = true;
                    n.expanded = true;
                    self.backup_batched(&path, 0.0);
                    done += 1;
                } else {
                    n.pending = true;
                    let legal: Vec<Move> = moves.iter().copied().collect();
                    leaves.push((board, legal));
                    meta.push((idx, path));
                }
            }

            if !leaves.is_empty() {
                let results = self.guide.evaluate_batch(&leaves);
                assert_eq!(
                    results.len(),
                    leaves.len(),
                    "Guide::evaluate_batch must return one result per item"
                );
                for (((_, legal), (idx, path)), (value, priors)) in
                    leaves.iter().zip(&meta).zip(results)
                {
                    let first = self.nodes.len() as u32;
                    for (i, &mv) in legal.iter().enumerate() {
                        self.nodes.push(Node::new(mv, priors.get(i).copied().unwrap_or(0.0)));
                    }
                    let node = &mut self.nodes[*idx as usize];
                    node.first_child = first;
                    node.n_children = legal.len() as u32;
                    node.expanded = true;
                    node.pending = false;
                    self.backup_batched(path, value);
                    done += 1;
                }
            }
        }

        let r = &self.nodes[0];
        let mut best = Move::NONE;
        let mut best_n = 0u32;
        let mut dist = Vec::with_capacity(r.n_children as usize);
        for c in r.first_child..r.first_child + r.n_children {
            let ch = &self.nodes[c as usize];
            dist.push((ch.mv, ch.visits));
            if ch.visits > best_n {
                best_n = ch.visits;
                best = ch.mv;
            }
        }
        (best, dist)
    }

    /// Back up `leaf_value` along a path whose visit counts and (non-root)
    /// virtual losses were applied during descent: revert the loss, keep the
    /// visit, add the real value with the per-ply sign flip.
    fn backup_batched(&mut self, path: &[u32], leaf_value: f32) {
        let mut v = leaf_value;
        for &i in path.iter().rev() {
            let n = &mut self.nodes[i as usize];
            if i != 0 {
                n.value_sum -= 1.0;
            }
            n.value_sum += v as f64;
            v = -v;
        }
    }

    /// Fully undo an aborted descent (visits and non-root virtual losses).
    fn revert_descent(&mut self, path: &[u32]) {
        for &i in path {
            let n = &mut self.nodes[i as usize];
            n.visits -= 1;
            if i != 0 {
                n.value_sum -= 1.0;
            }
        }
    }

    fn simulate(&mut self, root: &Board) {
        let mut board = root.clone();
        let mut path: Vec<u32> = vec![0];
        let mut idx = 0u32;

        loop {
            let node = &self.nodes[idx as usize];
            if !node.expanded || node.terminal {
                break;
            }
            let parent_n = node.visits as f32;
            let (cs, nc) = (node.first_child, node.n_children);
            let sqrt_parent = parent_n.max(1.0).sqrt();
            let mut best_score = f32::MIN;
            let mut best = cs;
            for c in cs..cs + nc {
                let ch = &self.nodes[c as usize];
                let q = -ch.q(); // child Q is from child's perspective
                let u = self.c_puct * ch.prior * sqrt_parent / (1.0 + ch.visits as f32);
                let score = q + u;
                if score > best_score {
                    best_score = score;
                    best = c;
                }
            }
            idx = best;
            board.make_move(self.nodes[idx as usize].mv);
            path.push(idx);
        }

        let value = self.expand_and_eval(idx, &board);

        let mut v = value;
        for &i in path.iter().rev() {
            let n = &mut self.nodes[i as usize];
            n.visits += 1;
            n.value_sum += v as f64;
            v = -v;
        }
    }

    fn expand_and_eval(&mut self, idx: u32, board: &Board) -> f32 {
        let moves = board.legal_moves();
        if moves.is_empty() {
            self.nodes[idx as usize].terminal = true;
            self.nodes[idx as usize].expanded = true;
            return if board.in_check() { -1.0 } else { 0.0 };
        }
        if board.halfmove_clock() >= 100 || board.is_insufficient_material() {
            self.nodes[idx as usize].terminal = true;
            self.nodes[idx as usize].expanded = true;
            return 0.0;
        }

        let legal: Vec<Move> = moves.iter().copied().collect();
        let (value, priors) = self.guide.evaluate(board, &legal);

        let first = self.nodes.len() as u32;
        for (i, &mv) in legal.iter().enumerate() {
            self.nodes.push(Node::new(mv, priors.get(i).copied().unwrap_or(0.0)));
        }
        let node = &mut self.nodes[idx as usize];
        node.first_child = first;
        node.n_children = legal.len() as u32;
        node.expanded = true;
        value
    }
}

/// Map a centipawn eval (side-to-move) to a value in `(-1, 1)`.
#[inline]
fn cp_to_value(cp: i32) -> f32 {
    2.0 / (1.0 + (-(cp as f32) / 400.0).exp()) - 1.0
}

/// Value of a known-terminal position (mirrors [`Mcts::expand_and_eval`]):
/// mate is a loss for the side to move; everything else terminal is a draw.
#[inline]
fn terminal_value(board: &Board) -> f32 {
    if board.legal_moves().is_empty() {
        if board.in_check() { -1.0 } else { 0.0 }
    } else {
        0.0 // 50-move / insufficient material
    }
}

/// A [`Guide`] that evaluates leaves on a batched-eval server (e.g. the GPU
/// torch server `neural/eval_server.py`) over a Unix-domain socket.
///
/// `evaluate_batch` *pipelines*: all leaves are written in one flush and the
/// responses are read back in order, so K in-tree leaves cost one round trip,
/// and the server batches across leaves × connections (games) per GPU forward.
/// The Rust side sends precomputed stm-relative feature indices and legal move
/// indices, so the server does zero chess logic on the hot path.
///
/// Wire format (little-endian), one **frame** per `evaluate_batch` call (so the
/// server does one decode and one reply write per K leaves, not per leaf):
///   request : `[body_len u32][n u16]` then n × (`[nf u8][nmoves u16]`,
///             `nf × u16` features, `nmoves × u16` move indices)
///   response: n × (`[value f32]`, `nmoves × f32` priors), concatenated
///             (tanh value, softmax over the moves)
pub struct RemoteGuide {
    w: std::io::BufWriter<std::os::unix::net::UnixStream>,
    r: std::io::BufReader<std::os::unix::net::UnixStream>,
    body: Vec<u8>,
    resp: Vec<u8>,
}

impl RemoteGuide {
    pub fn connect(path: &str) -> std::io::Result<RemoteGuide> {
        let s = std::os::unix::net::UnixStream::connect(path)?;
        let r = std::io::BufReader::new(s.try_clone()?);
        Ok(RemoteGuide {
            w: std::io::BufWriter::new(s),
            r,
            body: Vec::new(),
            resp: Vec::new(),
        })
    }
}

impl Guide for RemoteGuide {
    fn evaluate(&mut self, board: &Board, moves: &[Move]) -> (f32, Vec<f32>) {
        self.evaluate_batch(&[(board.clone(), moves.to_vec())])
            .pop()
            .unwrap()
    }

    fn evaluate_batch(&mut self, items: &[(Board, Vec<Move>)]) -> Vec<(f32, Vec<f32>)> {
        use std::io::{Read, Write};
        self.body.clear();
        for (board, moves) in items {
            let feats = crate::eval::policyvalue::stm_feature_indices(board);
            self.body.push(feats.len() as u8);
            self.body.extend_from_slice(&(moves.len() as u16).to_le_bytes());
            for f in &feats {
                self.body.extend_from_slice(&f.to_le_bytes());
            }
            for &mv in moves {
                let mi = crate::eval::policyvalue::move_index(mv) as u16;
                self.body.extend_from_slice(&mi.to_le_bytes());
            }
        }
        self.w
            .write_all(&(self.body.len() as u32).to_le_bytes())
            .and_then(|_| self.w.write_all(&(items.len() as u16).to_le_bytes()))
            .and_then(|_| self.w.write_all(&self.body))
            .and_then(|_| self.w.flush())
            .expect("eval server write");

        let total: usize = items.iter().map(|(_, m)| 4 + 4 * m.len()).sum();
        self.resp.resize(total, 0);
        self.r.read_exact(&mut self.resp).expect("eval server closed");
        let mut out = Vec::with_capacity(items.len());
        let mut o = 0usize;
        for (_, moves) in items {
            let value = f32::from_le_bytes(self.resp[o..o + 4].try_into().unwrap());
            o += 4;
            let priors = (0..moves.len())
                .map(|i| {
                    f32::from_le_bytes(self.resp[o + 4 * i..o + 4 * i + 4].try_into().unwrap())
                })
                .collect();
            o += 4 * moves.len();
            out.push((value, priors));
        }
        out
    }
}

// --- Dirichlet noise (zero-dep): Dir(alpha) = normalized i.i.d. Gamma(alpha). ---

#[inline]
fn next_u64(s: &mut u64) -> u64 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    *s
}
#[inline]
fn uniform(s: &mut u64) -> f32 {
    // (0,1): keep it strictly positive so ln() is finite.
    ((next_u64(s) >> 40) as f32 + 1.0) / (((1u64 << 24) + 1) as f32)
}
#[inline]
fn normal(s: &mut u64) -> f32 {
    // Box–Muller.
    let u1 = uniform(s);
    let u2 = uniform(s);
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}
/// Marsaglia–Tsang Gamma(alpha, 1) sampler (with the alpha<1 boost).
fn gamma(alpha: f32, s: &mut u64) -> f32 {
    if alpha < 1.0 {
        let g = gamma(alpha + 1.0, s);
        return g * uniform(s).powf(1.0 / alpha);
    }
    let d = alpha - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        let x = normal(s);
        let v = (1.0 + c * x).powi(3);
        if v <= 0.0 {
            continue;
        }
        let u = uniform(s);
        if u < 1.0 - 0.0331 * x * x * x * x {
            return d * v;
        }
        if u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
            return d * v;
        }
    }
}
fn dirichlet(n: usize, alpha: f32, seed: u64) -> Vec<f32> {
    let mut s = seed | 1;
    let g: Vec<f32> = (0..n).map(|_| gamma(alpha, &mut s)).collect();
    let sum: f32 = g.iter().sum();
    if sum <= 0.0 {
        return vec![1.0 / n as f32; n];
    }
    g.into_iter().map(|x| x / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::HandcraftedEval;

    #[test]
    fn finds_mate_in_one() {
        let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R6K w - - 0 1").unwrap();
        let mut mcts = Mcts::value(HandcraftedEval::new());
        let (best, _) = mcts.search(&board, 2000);
        assert_eq!(board.san(best), "Ra8#");
    }

    #[test]
    fn returns_legal_moves() {
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        ] {
            let board = Board::from_fen(fen).unwrap();
            let mut mcts = Mcts::value(HandcraftedEval::new());
            let (best, dist) = mcts.search(&board, 800);
            assert!(board.legal_moves().contains(best));
            assert!(dist.iter().map(|&(_, n)| n).sum::<u32>() > 0);
        }
    }

    #[test]
    fn dirichlet_is_a_distribution() {
        for (n, a) in [(20usize, 0.3f32), (1, 0.3), (40, 0.15)] {
            let d = dirichlet(n, a, 12345);
            assert_eq!(d.len(), n);
            assert!(d.iter().all(|&x| x >= 0.0 && x.is_finite()));
            assert!((d.iter().sum::<f32>() - 1.0).abs() < 1e-3);
        }
    }

    #[test]
    fn noisy_search_returns_legal_full_visits() {
        let board = Board::startpos();
        let net = PolicyValueNet::random(5);
        let mut mcts = Mcts::new(net);
        let (best, dist) = mcts.search_noisy(&board, 400, 0.3, 0.25, 999);
        assert!(board.legal_moves().contains(best));
        assert_eq!(dist.iter().map(|&(_, n)| n).sum::<u32>(), 400);
    }

    #[test]
    fn batched_search_full_visits_and_legal() {
        let board = Board::startpos();
        let net = PolicyValueNet::random(7);
        let mut mcts = Mcts::new(net);
        let (best, dist) = mcts.search_noisy_batched(&board, 400, 0.3, 0.25, 42, 8);
        assert!(board.legal_moves().contains(best));
        assert_eq!(dist.iter().map(|&(_, n)| n).sum::<u32>(), 400);
    }

    #[test]
    fn batched_search_finds_mate_in_one() {
        let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R6K w - - 0 1").unwrap();
        let mut mcts = Mcts::value(HandcraftedEval::new());
        let (best, _) = mcts.search_noisy_batched(&board, 2000, 0.0, 0.0, 1, 8);
        assert_eq!(board.san(best), "Ra8#");
    }

    #[test]
    fn batched_and_classic_agree_on_visit_budget_per_root_child() {
        // Not bit-identical (virtual loss reorders exploration), but both must
        // produce a full, legal visit distribution from the same net.
        let board = Board::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 0 1").unwrap();
        let net = PolicyValueNet::random(11);
        let mut a = Mcts::new(net);
        let (_, da) = a.search_noisy_batched(&board, 256, 0.0, 0.0, 1, 16);
        assert_eq!(da.iter().map(|&(_, n)| n).sum::<u32>(), 256);
        assert!(da.iter().all(|&(m, _)| board.legal_moves().contains(m)));
    }

    #[test]
    fn policy_value_net_drives_mcts() {
        let net = PolicyValueNet::random(3);
        let board = Board::startpos();
        let mut mcts = Mcts::new(net);
        let (best, dist) = mcts.search(&board, 400);
        assert!(board.legal_moves().contains(best));
        assert!(!dist.is_empty());
    }
}
