//! PUCT Monte-Carlo Tree Search — the search half of the AlphaZero-style arm.
//!
//! This is the engine that, with a policy+value network and self-play
//! reinforcement learning, is the only principled route to *exceed* (not merely
//! approach) a teacher like Stockfish. As a scoping spike it runs on any
//! [`Evaluator`] as the leaf **value** with **uniform move priors**; a trained
//! policy head replaces the uniform priors later, which is where MCTS's strength
//! comes from.
//!
//! Tree is a flat arena (`Vec<Node>`); each simulation selects by the PUCT rule
//! `Q + c·P·√N_parent/(1+N_child)`, expands one leaf (evaluating the position
//! with the value function), and backs the value up the path with a per-ply sign
//! flip (negamax convention). Values are in `[-1, 1]` (loss..win, side-to-move).

use crate::board::Board;
use crate::eval::Evaluator;
use crate::moves::Move;

struct Node {
    mv: Move,
    prior: f32,
    visits: u32,
    /// Sum of backed-up values, from the perspective of the side to move here.
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

/// A PUCT MCTS searcher over a value (and, later, policy) [`Evaluator`].
pub struct Mcts<E: Evaluator> {
    eval: E,
    c_puct: f32,
    nodes: Vec<Node>,
}

impl<E: Evaluator> Mcts<E> {
    pub fn new(eval: E) -> Mcts<E> {
        Mcts {
            eval,
            c_puct: 1.5,
            nodes: Vec::new(),
        }
    }

    pub fn set_cpuct(&mut self, c: f32) {
        self.c_puct = c;
    }

    /// Search from `root` for `sims` simulations; returns the most-visited move
    /// and the full visit distribution (the improved policy, for training).
    pub fn search(&mut self, root: &Board, sims: u32) -> (Move, Vec<(Move, u32)>) {
        self.nodes.clear();
        self.nodes.push(Node::new(Move::NONE, 1.0));
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

        // Selection: descend by PUCT until a leaf (unexpanded or terminal).
        loop {
            let node = &self.nodes[idx as usize];
            if !node.expanded || node.terminal {
                break;
            }
            let parent_n = node.visits as f32;
            let (cs, nc) = (node.first_child, node.n_children);
            let mut best_score = f32::MIN;
            let mut best = cs;
            let sqrt_parent = parent_n.max(1.0).sqrt();
            for c in cs..cs + nc {
                let ch = &self.nodes[c as usize];
                // Child Q is from the child's perspective; from ours it's negated.
                let q = -ch.q();
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

        // Expansion + leaf evaluation (value from the leaf's side-to-move view).
        let value = self.expand_and_eval(idx, &board);

        // Backup with a per-ply sign flip.
        let mut v = value;
        for &i in path.iter().rev() {
            let n = &mut self.nodes[i as usize];
            n.visits += 1;
            n.value_sum += v as f64;
            v = -v;
        }
    }

    /// Expand `idx` (unless terminal) and return its position value in `[-1,1]`.
    fn expand_and_eval(&mut self, idx: u32, board: &Board) -> f32 {
        let legal = board.legal_moves();
        if legal.is_empty() {
            self.nodes[idx as usize].terminal = true;
            self.nodes[idx as usize].expanded = true;
            return if board.in_check() { -1.0 } else { 0.0 };
        }
        if board.halfmove_clock() >= 100 || board.is_insufficient_material() {
            self.nodes[idx as usize].terminal = true;
            self.nodes[idx as usize].expanded = true;
            return 0.0;
        }

        // Uniform priors for the spike (a policy head replaces these later).
        let prior = 1.0 / legal.len() as f32;
        let first = self.nodes.len() as u32;
        for &mv in legal.iter() {
            self.nodes.push(Node::new(mv, prior));
        }
        let node = &mut self.nodes[idx as usize];
        node.first_child = first;
        node.n_children = legal.len() as u32;
        node.expanded = true;

        let cp = self.eval.evaluate(board);
        cp_to_value(cp)
    }
}

/// Map a centipawn eval (side-to-move) to a value in `(-1, 1)`.
#[inline]
fn cp_to_value(cp: i32) -> f32 {
    2.0 / (1.0 + (-(cp as f32) / 400.0).exp()) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::HandcraftedEval;

    #[test]
    fn finds_mate_in_one() {
        // Back-rank mate: Ra8#.
        let board = Board::from_fen("6k1/5ppp/8/8/8/8/8/R6K w - - 0 1").unwrap();
        let mut mcts = Mcts::new(HandcraftedEval::new());
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
            let mut mcts = Mcts::new(HandcraftedEval::new());
            let (best, dist) = mcts.search(&board, 800);
            assert!(board.legal_moves().contains(best));
            let total: u32 = dist.iter().map(|&(_, n)| n).sum();
            assert!(total > 0);
        }
    }
}
