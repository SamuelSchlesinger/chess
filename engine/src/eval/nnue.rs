//! NNUE-style neural evaluation (float inference).
//!
//! Architecture (the baseline "768" perspective net — the trainer must match
//! this exactly):
//! ```text
//!   feature set "768":  index(perspective, piece@sq) =
//!       (rel_color*6 + piece_type)*64 + rel_sq
//!     where rel_color = 0 if piece.color == perspective else 1,
//!           rel_sq    = sq if perspective == White else sq ^ 56  (vertical mirror)
//!     -> 2 (own/opp) × 6 (types) × 64 (squares) = 768 features per perspective.
//!
//!   accumulator[p][h] = ft_bias[h] + Σ_{active f} ft_weight[f][h]   (HIDDEN = 256)
//!   input             = clippedReLU( concat[ acc[stm], acc[!stm] ] )  (512)
//!   out               = out_bias + Σ_i input[i] · out_weight[i]
//!   eval_cp (stm)     = out · SCALE
//! ```
//! Inference is float for now (correct + already far faster than a transformer);
//! int8/int16 quantization and incremental accumulator updates are follow-ups.
//! The accumulator is currently recomputed from scratch per evaluation; the
//! `on_make`/`on_unmake` hooks remain available for the incremental upgrade.
//!
//! Weights file (little-endian): `[magic u32 = 0x4E4E5545 "NNUE"][version u32]
//! [inputs u32 = 768][hidden u32 = 256]` then f32 arrays in order:
//! `ft_weight[768*256]` (feature-major), `ft_bias[256]`, `out_weight[512]`,
//! `out_bias[1]`.

use super::Evaluator;
use crate::board::Board;
use crate::moves::Move;
use crate::types::{Color, PieceType};
use std::sync::Arc;

pub const INPUTS: usize = 768;
pub const HIDDEN: usize = 256;
const OUT_INPUTS: usize = HIDDEN * 2;
const MAGIC: u32 = 0x4E4E_5545; // "NNUE"
/// Output-to-centipawn scale (the trainer targets sigmoid(out) ≈ win prob).
const SCALE: f32 = 400.0;

/// Trained network parameters.
pub struct Nnue {
    /// Feature-major `[INPUTS][HIDDEN]`.
    ft_weight: Vec<f32>,
    ft_bias: Vec<f32>,
    /// `[OUT_INPUTS]` = `[2*HIDDEN]`.
    out_weight: Vec<f32>,
    out_bias: f32,
}

impl Nnue {
    /// Parse a network from its binary representation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Nnue, String> {
        let mut r = Reader { bytes, pos: 0 };
        let magic = r.u32()?;
        if magic != MAGIC {
            return Err(format!("bad magic {magic:#x}, expected {MAGIC:#x}"));
        }
        let _version = r.u32()?;
        let inputs = r.u32()? as usize;
        let hidden = r.u32()? as usize;
        if inputs != INPUTS || hidden != HIDDEN {
            return Err(format!(
                "net is {inputs}x{hidden}, this build expects {INPUTS}x{HIDDEN}"
            ));
        }
        let ft_weight = r.f32s(INPUTS * HIDDEN)?;
        let ft_bias = r.f32s(HIDDEN)?;
        let out_weight = r.f32s(OUT_INPUTS)?;
        let out_bias = r.f32s(1)?[0];
        Ok(Nnue {
            ft_weight,
            ft_bias,
            out_weight,
            out_bias,
        })
    }

    /// Load a network from a file.
    pub fn load(path: &str) -> Result<Nnue, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
        Nnue::from_bytes(&bytes)
    }

    /// Serialize to the binary format (used by tests / round-trips).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(INPUTS as u32).to_le_bytes());
        out.extend_from_slice(&(HIDDEN as u32).to_le_bytes());
        for v in self
            .ft_weight
            .iter()
            .chain(&self.ft_bias)
            .chain(&self.out_weight)
            .chain(std::iter::once(&self.out_bias))
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// Accumulators for the White and Black perspectives, from scratch.
    fn accumulate(&self, board: &Board) -> ([f32; HIDDEN], [f32; HIDDEN]) {
        let mut white = [0f32; HIDDEN];
        let mut black = [0f32; HIDDEN];
        white.copy_from_slice(&self.ft_bias);
        black.copy_from_slice(&self.ft_bias);

        for pt in PieceType::ALL {
            for color in Color::ALL {
                let mut bb = board.pieces_colored(pt, color);
                while let Some(sq) = bb.pop_lsb() {
                    let (wi, bi) = feature_indices(color, pt, sq.index());
                    add_column(&mut white, &self.ft_weight, wi);
                    add_column(&mut black, &self.ft_weight, bi);
                }
            }
        }
        (white, black)
    }

    /// Evaluate in centipawns from the side-to-move's perspective.
    fn forward(&self, board: &Board) -> i32 {
        let (white, black) = self.accumulate(board);
        let (us, them) = match board.side_to_move() {
            Color::White => (&white, &black),
            Color::Black => (&black, &white),
        };
        let mut out = self.out_bias;
        for h in 0..HIDDEN {
            out += clipped_relu(us[h]) * self.out_weight[h];
            out += clipped_relu(them[h]) * self.out_weight[HIDDEN + h];
        }
        (out * SCALE).round() as i32
    }
}

/// An [`Evaluator`] backed by a trained [`Nnue`].
#[derive(Clone)]
pub struct NnueEval {
    net: Arc<Nnue>,
}

impl NnueEval {
    pub fn new(net: Nnue) -> NnueEval {
        NnueEval { net: Arc::new(net) }
    }
    pub fn load(path: &str) -> Result<NnueEval, String> {
        Ok(NnueEval::new(Nnue::load(path)?))
    }
}

impl Evaluator for NnueEval {
    fn evaluate(&mut self, board: &Board) -> i32 {
        self.net.forward(board)
    }
}

// --- Quantized, incremental inference (the production path) -----------------

/// Feature-transformer quantization scale (weights ≈ `round(w · QA)` as i16).
pub const QA: i32 = 1024;
/// Output-layer quantization scale (the dot accumulates in i64, so this can
/// be generous; i16 caps `|out_w|` at 32767/QB = 8.0 — ample for trained nets).
pub const QB: i32 = 4096;

/// [`Nnue`] weights quantized to i16 for fast integer inference.
pub struct QNnue {
    ft_w: Vec<i16>, // [INPUTS*HIDDEN] feature-major, scale QA
    ft_b: Vec<i16>, // [HIDDEN], scale QA
    out_w: Vec<i16>, // [OUT_INPUTS], scale QB
    out_b: i64,     // scale QA*QB
}

impl QNnue {
    pub fn from_float(n: &Nnue) -> QNnue {
        let q16 = |x: f32, s: i32| ((x * s as f32).round() as i32).clamp(-32768, 32767) as i16;
        QNnue {
            ft_w: n.ft_weight.iter().map(|&w| q16(w, QA)).collect(),
            ft_b: n.ft_bias.iter().map(|&b| q16(b, QA)).collect(),
            out_w: n.out_weight.iter().map(|&w| q16(w, QB)).collect(),
            out_b: (n.out_bias * (QA * QB) as f32).round() as i64,
        }
    }

    pub fn load(path: &str) -> Result<QNnue, String> {
        Ok(QNnue::from_float(&Nnue::load(path)?))
    }
}

/// One accumulator pair (White perspective, Black perspective), scale QA.
#[derive(Clone)]
struct Acc {
    w: [i32; HIDDEN],
    b: [i32; HIDDEN],
}

/// Quantized **incremental** NNUE evaluator: the accumulator is updated on
/// [`Evaluator::on_make`]/[`Evaluator::on_unmake`] (a stack, one entry per
/// search ply) instead of being rebuilt per evaluation — the classic NNUE
/// speedup. `on_make` runs against the **pre-move** board, which fully
/// determines the feature diff (mover, captures incl. en passant, promotion,
/// castling rook); after `unmake` the board is back in that same state, so
/// popping the stack is exact. Integer arithmetic makes incremental and
/// from-scratch accumulators bit-identical (tested below).
pub struct QNnueEval {
    net: Arc<QNnue>,
    stack: Vec<Acc>,
}

impl QNnueEval {
    pub fn new(net: QNnue) -> QNnueEval {
        QNnueEval {
            net: Arc::new(net),
            stack: Vec::with_capacity(160),
        }
    }

    pub fn load(path: &str) -> Result<QNnueEval, String> {
        Ok(QNnueEval::new(QNnue::load(path)?))
    }

    fn rebuild(&self, board: &Board) -> Acc {
        let mut acc = Acc {
            w: [0; HIDDEN],
            b: [0; HIDDEN],
        };
        for h in 0..HIDDEN {
            acc.w[h] = self.net.ft_b[h] as i32;
            acc.b[h] = self.net.ft_b[h] as i32;
        }
        for pt in PieceType::ALL {
            for color in Color::ALL {
                let mut bb = board.pieces_colored(pt, color);
                while let Some(sq) = bb.pop_lsb() {
                    add_feature::<1>(&mut acc, &self.net, color, pt, sq.index());
                }
            }
        }
        acc
    }

    /// Apply `mv`'s feature diff to the top accumulator (board = pre-move).
    fn apply(&mut self, acc: &mut Acc, board: &Board, mv: Move) {
        let us = board.side_to_move();
        let from = mv.from();
        let to = mv.to();
        let pc = board.piece_at(from).expect("on_make: no piece on from-square");
        add_feature::<-1>(acc, &self.net, pc.color, pc.piece_type, from.index());
        if mv.is_en_passant() {
            let cap_sq = to.index() ^ 8; // the pawn one rank behind the target
            add_feature::<-1>(acc, &self.net, us.flip(), PieceType::Pawn, cap_sq);
        } else if let Some(victim) = board.piece_at(to) {
            add_feature::<-1>(acc, &self.net, victim.color, victim.piece_type, to.index());
        }
        let placed = mv.promotion_piece().unwrap_or(pc.piece_type);
        add_feature::<1>(acc, &self.net, us, placed, to.index());
        if mv.is_castle() {
            let (rf, rt) = if mv.is_king_castle() {
                (to.index() + 1, to.index() - 1)
            } else {
                (to.index() - 2, to.index() + 1)
            };
            add_feature::<-1>(acc, &self.net, us, PieceType::Rook, rf);
            add_feature::<1>(acc, &self.net, us, PieceType::Rook, rt);
        }
    }
}

impl Evaluator for QNnueEval {
    fn evaluate(&mut self, board: &Board) -> i32 {
        if self.stack.is_empty() {
            self.stack.push(self.rebuild(board));
        }
        let acc = self.stack.last().unwrap();
        let (us, them) = match board.side_to_move() {
            Color::White => (&acc.w, &acc.b),
            Color::Black => (&acc.b, &acc.w),
        };
        let mut out = self.net.out_b;
        for h in 0..HIDDEN {
            out += (us[h].clamp(0, QA) as i64) * (self.net.out_w[h] as i64);
            out += (them[h].clamp(0, QA) as i64) * (self.net.out_w[HIDDEN + h] as i64);
        }
        ((out as f32) * SCALE / (QA * QB) as f32).round() as i32
    }

    fn on_make(&mut self, board: &Board, mv: Move) {
        if self.stack.is_empty() {
            self.stack.push(self.rebuild(board));
        }
        let mut acc = self.stack.last().unwrap().clone();
        if !mv.is_none() {
            self.apply(&mut acc, board, mv);
        }
        self.stack.push(acc);
    }

    fn on_unmake(&mut self, _board: &Board, _mv: Move) {
        self.stack.pop();
    }

    fn refresh(&mut self, board: &Board) {
        self.stack.clear();
        self.stack.push(self.rebuild(board));
    }
}

/// Add (`SIGN = 1`) or remove (`SIGN = -1`) one piece-square feature from both
/// perspective accumulators.
#[inline]
fn add_feature<const SIGN: i32>(acc: &mut Acc, net: &QNnue, color: Color, pt: PieceType, sq: usize) {
    let (wi, bi) = feature_indices(color, pt, sq);
    let wcol = &net.ft_w[wi * HIDDEN..wi * HIDDEN + HIDDEN];
    let bcol = &net.ft_w[bi * HIDDEN..bi * HIDDEN + HIDDEN];
    for h in 0..HIDDEN {
        acc.w[h] += SIGN * wcol[h] as i32;
    }
    for h in 0..HIDDEN {
        acc.b[h] += SIGN * bcol[h] as i32;
    }
}

/// `(white_perspective_index, black_perspective_index)` for a piece.
#[inline]
pub fn feature_indices(color: Color, pt: PieceType, sq: usize) -> (usize, usize) {
    let t = pt.index();
    // White perspective: own pieces are White.
    let w_rel_color = if color == Color::White { 0 } else { 1 };
    let wi = (w_rel_color * 6 + t) * 64 + sq;
    // Black perspective: own pieces are Black; board mirrored vertically.
    let b_rel_color = if color == Color::Black { 0 } else { 1 };
    let bi = (b_rel_color * 6 + t) * 64 + (sq ^ 56);
    (wi, bi)
}

#[inline]
fn add_column(acc: &mut [f32; HIDDEN], ft_weight: &[f32], feature: usize) {
    let base = feature * HIDDEN;
    let col = &ft_weight[base..base + HIDDEN];
    for h in 0..HIDDEN {
        acc[h] += col[h];
    }
}

#[inline]
fn clipped_relu(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn u32(&mut self) -> Result<u32, String> {
        let end = self.pos + 4;
        if end > self.bytes.len() {
            return Err("unexpected EOF".into());
        }
        let v = u32::from_le_bytes(self.bytes[self.pos..end].try_into().unwrap());
        self.pos = end;
        Ok(v)
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>, String> {
        let end = self.pos + n * 4;
        if end > self.bytes.len() {
            return Err("unexpected EOF reading floats".into());
        }
        let v = self.bytes[self.pos..end]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        self.pos = end;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic pseudo-random net for testing the forward pass.
    fn random_net(seed: u64) -> Nnue {
        let mut s = seed;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            // small values in [-0.1, 0.1)
            ((s >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 0.2
        };
        Nnue {
            ft_weight: (0..INPUTS * HIDDEN).map(|_| next()).collect(),
            ft_bias: (0..HIDDEN).map(|_| next()).collect(),
            out_weight: (0..OUT_INPUTS).map(|_| next()).collect(),
            out_bias: next(),
        }
    }

    /// Independent reference forward pass, to validate `forward`.
    fn reference(net: &Nnue, board: &Board) -> i32 {
        let mut white = net.ft_bias.clone();
        let mut black = net.ft_bias.clone();
        for sq in 0..64u8 {
            if let Some(p) = board.piece_at(crate::types::Square(sq)) {
                let (wi, bi) = feature_indices(p.color, p.piece_type, sq as usize);
                for h in 0..HIDDEN {
                    white[h] += net.ft_weight[wi * HIDDEN + h];
                    black[h] += net.ft_weight[bi * HIDDEN + h];
                }
            }
        }
        let (us, them) = match board.side_to_move() {
            Color::White => (&white, &black),
            Color::Black => (&black, &white),
        };
        let mut out = net.out_bias;
        for h in 0..HIDDEN {
            out += us[h].clamp(0.0, 1.0) * net.out_weight[h];
            out += them[h].clamp(0.0, 1.0) * net.out_weight[HIDDEN + h];
        }
        (out * SCALE).round() as i32
    }

    #[test]
    fn forward_matches_reference() {
        let net = random_net(0xC0FFEE);
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 b - - 0 1",
        ] {
            let board = Board::from_fen(fen).unwrap();
            assert_eq!(net.forward(&board), reference(&net, &board), "{fen}");
        }
    }

    #[test]
    fn serialize_round_trip() {
        let net = random_net(42);
        let bytes = net.to_bytes();
        let net2 = Nnue::from_bytes(&bytes).unwrap();
        let board = Board::startpos();
        assert_eq!(net.forward(&board), net2.forward(&board));
    }

    #[test]
    fn quantized_matches_float_within_tolerance() {
        let net = random_net(0xC0FFEE);
        let q = QNnue::from_float(&net);
        let mut qe = QNnueEval::new(q);
        for fen in [
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 b - - 0 1",
        ] {
            let board = Board::from_fen(fen).unwrap();
            qe.refresh(&board);
            let f = net.forward(&board);
            let qv = qe.evaluate(&board);
            assert!((f - qv).abs() <= 3, "{fen}: float {f} vs quant {qv}");
        }
    }

    /// Incremental accumulators must be BIT-IDENTICAL to from-scratch rebuilds
    /// across random playouts (integer arithmetic is exact). Exercises every
    /// move kind the diff handles: captures, en passant, promotions, castling.
    #[test]
    fn incremental_matches_rebuild_over_random_games() {
        let net = random_net(7);
        let mut qe = QNnueEval::new(QNnue::from_float(&net));
        let mut rng = 0x12345u64;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        for _ in 0..30 {
            let mut board = Board::startpos();
            qe.refresh(&board);
            let mut undos = Vec::new();
            for _ in 0..80 {
                let moves = board.legal_moves();
                if moves.is_empty() {
                    break;
                }
                let mv = moves.as_slice()[(next() % moves.len() as u64) as usize];
                qe.on_make(&board, mv);
                undos.push((mv, board.make_move(mv)));
                let top = qe.stack.last().unwrap();
                let fresh = qe.rebuild(&board);
                assert_eq!(top.w, fresh.w, "white acc diverged after {mv:?}");
                assert_eq!(top.b, fresh.b, "black acc diverged after {mv:?}");
                // Occasionally unwind a few plies to exercise the pop path.
                if next() % 11 == 0 && undos.len() >= 2 {
                    for _ in 0..2 {
                        let (m, u) = undos.pop().unwrap();
                        board.unmake_move(m, u);
                        qe.on_unmake(&board, m);
                    }
                    let top = qe.stack.last().unwrap();
                    let fresh = qe.rebuild(&board);
                    assert_eq!(top.w, fresh.w, "acc diverged after unmake");
                    assert_eq!(top.b, fresh.b, "acc diverged after unmake");
                }
            }
        }
    }

    #[test]
    fn feature_index_ranges() {
        for sq in 0..64usize {
            for pt in PieceType::ALL {
                for c in Color::ALL {
                    let (wi, bi) = feature_indices(c, pt, sq);
                    assert!(wi < INPUTS && bi < INPUTS);
                }
            }
        }
    }
}
