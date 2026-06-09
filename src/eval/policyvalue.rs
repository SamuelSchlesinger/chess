//! Policy + value network for the AlphaZero-style self-play arm.
//!
//! Unlike the NNUE (a value-only eval for alpha-beta), this net outputs both a
//! **value** (win estimate, side-to-move) and a **policy** (prior over moves) so
//! MCTS can focus its search — the spike proved uniform priors are the
//! bottleneck. It is trained by self-play reinforcement learning, so it has no
//! teacher ceiling: the only principled route to *exceed* Stockfish.
//!
//! Architecture (small, for fast Rust inference inside MCTS):
//! ```text
//!   input   768 side-to-move-relative features (own pieces = "us"; board
//!           mirrored when Black to move — this bakes in the exact color-swap
//!           antisymmetry as a hard inductive bias).
//!   trunk   sparse 768 -> FT (clipped-ReLU accumulator), then dense FT -> HID (ReLU)
//!   value   HID -> 1, tanh -> [-1, 1]
//!   policy  HID -> 4096 logits, indexed by from*64 + to (promotions share an
//!           index), masked + softmaxed over legal moves at use time.
//! ```
//!
//! Weights file (little-endian f32 after an `[magic u32 = "AZNET"][version]
//! [FT][HID]` header): ft_w[768*FT], ft_b[FT], h_w[FT*HID], h_b[HID],
//! v_w[HID], v_b[1], p_w[HID*POLICY], p_b[POLICY].

// The dense-layer loops index parallel arrays (acc[i] vs weight[base+i]);
// the iterator rewrite is less clear for matmuls, so allow the range-loop lint.
#![allow(clippy::needless_range_loop)]

use crate::board::Board;
use crate::moves::Move;
use crate::types::{Color, PieceType};

pub const INPUTS: usize = 768;
pub const FT: usize = 256;
pub const HID: usize = 256;
pub const POLICY: usize = 4096; // from*64 + to
const MAGIC: u32 = 0x415A_4E54; // "AZNT"

/// Trained policy+value parameters.
pub struct PolicyValueNet {
    ft_w: Vec<f32>, // [INPUTS*FT], feature-major
    ft_b: Vec<f32>, // [FT]
    h_w: Vec<f32>,  // [FT*HID], out-major (h[o] uses h_w[o*FT + i])
    h_b: Vec<f32>,  // [HID]
    v_w: Vec<f32>,  // [HID]
    v_b: f32,
    p_w: Vec<f32>, // [HID*POLICY], move-major (logit[m] uses p_w[m*HID + i])
    p_b: Vec<f32>, // [POLICY]
}

/// Policy index for a move (promotions of the same from/to share an index).
#[inline]
pub fn move_index(mv: Move) -> usize {
    mv.from().index() * 64 + mv.to().index()
}

/// Side-to-move-relative 768 feature index for a piece at `sq`.
#[inline]
fn stm_feature(stm: Color, color: Color, pt: PieceType, sq: usize) -> usize {
    let rel_color = if color == stm { 0 } else { 1 };
    let rel_sq = if stm == Color::White { sq } else { sq ^ 56 };
    (rel_color * 6 + pt.index()) * 64 + rel_sq
}

/// All active side-to-move-relative feature indices for `board` (≤ 32).
/// Used by remote/batched evaluation so the server does no chess logic.
pub fn stm_feature_indices(board: &Board) -> Vec<u16> {
    let stm = board.side_to_move();
    let mut feats = Vec::with_capacity(32);
    for pt in PieceType::ALL {
        for color in Color::ALL {
            let mut bb = board.pieces_colored(pt, color);
            while let Some(sq) = bb.pop_lsb() {
                feats.push(stm_feature(stm, color, pt, sq.index()) as u16);
            }
        }
    }
    feats
}

impl PolicyValueNet {
    pub fn from_bytes(bytes: &[u8]) -> Result<PolicyValueNet, String> {
        let mut r = Reader { bytes, pos: 0 };
        if r.u32()? != MAGIC {
            return Err("bad magic".into());
        }
        let _v = r.u32()?;
        let ft = r.u32()? as usize;
        let hid = r.u32()? as usize;
        if ft != FT || hid != HID {
            return Err(format!("net {ft}x{hid}, expected {FT}x{HID}"));
        }
        Ok(PolicyValueNet {
            ft_w: r.f32s(INPUTS * FT)?,
            ft_b: r.f32s(FT)?,
            h_w: r.f32s(FT * HID)?,
            h_b: r.f32s(HID)?,
            v_w: r.f32s(HID)?,
            v_b: r.f32s(1)?[0],
            p_w: r.f32s(HID * POLICY)?,
            p_b: r.f32s(POLICY)?,
        })
    }

    pub fn load(path: &str) -> Result<PolicyValueNet, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
        PolicyValueNet::from_bytes(&bytes)
    }

    /// Serialize to the weights format (e.g. to save generation 0).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut o = Vec::new();
        o.extend_from_slice(&MAGIC.to_le_bytes());
        o.extend_from_slice(&1u32.to_le_bytes());
        o.extend_from_slice(&(FT as u32).to_le_bytes());
        o.extend_from_slice(&(HID as u32).to_le_bytes());
        for v in self
            .ft_w
            .iter()
            .chain(&self.ft_b)
            .chain(&self.h_w)
            .chain(&self.h_b)
            .chain(&self.v_w)
            .chain(std::iter::once(&self.v_b))
            .chain(&self.p_w)
            .chain(&self.p_b)
        {
            o.extend_from_slice(&v.to_le_bytes());
        }
        o
    }

    pub fn save(&self, path: &str) -> std::io::Result<()> {
        std::fs::write(path, self.to_bytes())
    }

    /// Random net (for testing the MCTS/self-play loop before any training).
    pub fn random(seed: u64) -> PolicyValueNet {
        let mut s = seed | 1;
        let mut g = move |scale: f32| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 2.0 * scale
        };
        let he = |n: usize| (2.0f32 / n as f32).sqrt();
        PolicyValueNet {
            ft_w: (0..INPUTS * FT).map(|_| g(0.05)).collect(),
            ft_b: vec![0.0; FT],
            h_w: (0..FT * HID).map(|_| g(he(FT))).collect(),
            h_b: vec![0.0; HID],
            v_w: (0..HID).map(|_| g(he(HID))).collect(),
            v_b: 0.0,
            p_w: (0..HID * POLICY).map(|_| g(0.01)).collect(),
            p_b: vec![0.0; POLICY],
        }
    }

    /// The shared trunk activation for a board (side-to-move relative).
    fn trunk(&self, board: &Board) -> [f32; HID] {
        let stm = board.side_to_move();
        let mut acc = [0f32; FT];
        acc.copy_from_slice(&self.ft_b);
        for pt in PieceType::ALL {
            for color in Color::ALL {
                let mut bb = board.pieces_colored(pt, color);
                while let Some(sq) = bb.pop_lsb() {
                    let f = stm_feature(stm, color, pt, sq.index());
                    let base = f * FT;
                    for i in 0..FT {
                        acc[i] += self.ft_w[base + i];
                    }
                }
            }
        }
        for a in &mut acc {
            *a = a.clamp(0.0, 1.0); // clipped-ReLU
        }
        let mut h = [0f32; HID];
        for o in 0..HID {
            let base = o * FT;
            let mut s = self.h_b[o];
            for i in 0..FT {
                s += self.h_w[base + i] * acc[i];
            }
            h[o] = s.max(0.0); // ReLU
        }
        h
    }

    /// Value in `[-1, 1]` from the side-to-move's perspective.
    pub fn value(&self, board: &Board) -> f32 {
        let h = self.trunk(board);
        let mut s = self.v_b;
        for i in 0..HID {
            s += self.v_w[i] * h[i];
        }
        s.tanh()
    }

    /// `(value, priors)` where priors are a softmax over the given legal moves.
    pub fn evaluate(&self, board: &Board, moves: &[Move]) -> (f32, Vec<f32>) {
        let h = self.trunk(board);
        let mut value = self.v_b;
        for i in 0..HID {
            value += self.v_w[i] * h[i];
        }
        let value = value.tanh();

        // Policy logits for just the legal moves, then softmax.
        let mut logits = Vec::with_capacity(moves.len());
        let mut max = f32::MIN;
        for &mv in moves {
            let m = move_index(mv);
            let base = m * HID;
            let mut s = self.p_b[m];
            for i in 0..HID {
                s += self.p_w[base + i] * h[i];
            }
            logits.push(s);
            if s > max {
                max = s;
            }
        }
        let mut sum = 0.0;
        for l in &mut logits {
            *l = (*l - max).exp();
            sum += *l;
        }
        let inv = 1.0 / sum.max(1e-9);
        for l in &mut logits {
            *l *= inv;
        }
        (value, logits)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl Reader<'_> {
    fn u32(&mut self) -> Result<u32, String> {
        let e = self.pos + 4;
        if e > self.bytes.len() {
            return Err("EOF".into());
        }
        let v = u32::from_le_bytes(self.bytes[self.pos..e].try_into().unwrap());
        self.pos = e;
        Ok(v)
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, String> {
        let e = self.pos + n * 4;
        if e > self.bytes.len() {
            return Err("EOF floats".into());
        }
        let v = self.bytes[self.pos..e]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        self.pos = e;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_runs_and_is_valid() {
        let net = PolicyValueNet::random(7);
        let board = Board::startpos();
        let moves: Vec<Move> = board.legal_moves().iter().copied().collect();
        let (v, p) = net.evaluate(&board, &moves);
        assert!((-1.0..=1.0).contains(&v));
        assert_eq!(p.len(), moves.len());
        let sum: f32 = p.iter().sum();
        assert!((sum - 1.0).abs() < 1e-3, "priors must sum to 1, got {sum}");
        assert!(p.iter().all(|&x| x >= 0.0));
    }
}
