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
        if eps > 0.0 {
            // Expand the root up front so we can perturb its child priors.
            let _ = self.expand_and_eval(0, root);
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
    fn policy_value_net_drives_mcts() {
        let net = PolicyValueNet::random(3);
        let board = Board::startpos();
        let mut mcts = Mcts::new(net);
        let (best, dist) = mcts.search(&board, 400);
        assert!(board.legal_moves().contains(best));
        assert!(!dist.is_empty());
    }
}
