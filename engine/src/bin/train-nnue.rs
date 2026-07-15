//! A minimal, dependency-free NNUE trainer (CPU, Adam).
//!
//! Trains the 768→256×2→1 perspective net of [`chess::eval::nnue`] on the
//! 37-byte self-play records from `gen-data`, with a blended target:
//! `λ·sigmoid(eval/400) + (1−λ)·WDL`, and exports a `.nnue` weights file the
//! Rust engine loads. This closes the W1 loop entirely in-repo; the strong net
//! comes later from PyTorch + the Lichess dataset on the 3080.
//!
//! Usage:
//!   cargo run --release --bin train-nnue -- --data data/selfplay --epochs 8 \
//!       --lr 0.001 --batch 1000 --lambda 0.7 --out nets/v1.nnue

// The per-hidden index loops read parallel slices (acc[h] vs ft_w[base+h]);
// rewriting them as iterators is less clear, so allow the range-loop lint.
#![allow(clippy::needless_range_loop)]

use chess::Packed;
use chess::eval::nnue::{HIDDEN, INPUTS, feature_indices};
use chess::types::Square;

const OUT_INPUTS: usize = HIDDEN * 2;
const SCALE: f32 = 400.0;
const RECORD_LEN: usize = 37;

struct Sample {
    white_feats: Vec<u16>,
    black_feats: Vec<u16>,
    stm_white: bool,
    target: f32, // win prob from side-to-move's perspective
}

fn main() {
    let cfg = parse_args();
    let samples = load_samples(&cfg);
    if samples.is_empty() {
        eprintln!("no samples loaded from {}*", cfg.data);
        std::process::exit(1);
    }
    let n_val = (samples.len() / 20).clamp(1, 200_000);
    let (val, train) = samples.split_at(n_val);
    eprintln!(
        "loaded {} samples ({} train, {} val); {} epochs, lr {}, batch {}, lambda {}",
        samples.len(),
        train.len(),
        val.len(),
        cfg.epochs,
        cfg.lr,
        cfg.batch,
        cfg.lambda
    );

    let mut net = Net::init(cfg.seed);
    let mut rng = cfg.seed | 1;
    let mut order: Vec<usize> = (0..train.len()).collect();
    // Keep the best-validating net, not the last (it overfits).
    let mut best_val = f64::INFINITY;
    let mut best_bytes = net.to_nnue_bytes();

    for epoch in 0..cfg.epochs {
        // Shuffle.
        for i in (1..order.len()).rev() {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            order.swap(i, (rng % (i as u64 + 1)) as usize);
        }
        let mut running = 0.0f64;
        let mut seen = 0usize;
        let start = std::time::Instant::now();
        for chunk in order.chunks(cfg.batch) {
            let mut grad = Grad::zero();
            let mut batch_loss = 0.0f64;
            for &idx in chunk {
                batch_loss += net.accumulate_grad(&train[idx], &mut grad);
            }
            net.adam_step(&grad, chunk.len(), cfg.lr);
            running += batch_loss;
            seen += chunk.len();
        }
        let val_loss = val.iter().map(|s| net.loss(s)).sum::<f64>() / val.len() as f64;
        let best_marker = if val_loss < best_val {
            best_val = val_loss;
            best_bytes = net.to_nnue_bytes();
            " *"
        } else {
            ""
        };
        eprintln!(
            "epoch {:2}/{}: train mse {:.5}  val mse {:.5}{}  ({:.1}s, {:.0}k samples/s)",
            epoch + 1,
            cfg.epochs,
            running / seen as f64,
            val_loss,
            best_marker,
            start.elapsed().as_secs_f64(),
            seen as f64 / start.elapsed().as_secs_f64() / 1000.0,
        );
    }

    if let Some(dir) = std::path::Path::new(&cfg.out).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&cfg.out, &best_bytes).expect("write net");
    eprintln!(
        "wrote {} (best val mse {:.5}, {} params)",
        cfg.out,
        best_val,
        INPUTS * HIDDEN + HIDDEN + OUT_INPUTS + 1
    );
}

// --- network + Adam ---

struct Net {
    ft_w: Vec<f32>,
    ft_b: Vec<f32>,
    out_w: Vec<f32>,
    out_b: f32,
    // Adam moments.
    m: Moments,
    v: Moments,
    t: i32,
}

struct Moments {
    ft_w: Vec<f32>,
    ft_b: Vec<f32>,
    out_w: Vec<f32>,
    out_b: f32,
}
impl Moments {
    fn zero() -> Moments {
        Moments {
            ft_w: vec![0.0; INPUTS * HIDDEN],
            ft_b: vec![0.0; HIDDEN],
            out_w: vec![0.0; OUT_INPUTS],
            out_b: 0.0,
        }
    }
}

struct Grad {
    ft_w: Vec<f32>,
    ft_b: Vec<f32>,
    out_w: Vec<f32>,
    out_b: f32,
}
impl Grad {
    fn zero() -> Grad {
        Grad {
            ft_w: vec![0.0; INPUTS * HIDDEN],
            ft_b: vec![0.0; HIDDEN],
            out_w: vec![0.0; OUT_INPUTS],
            out_b: 0.0,
        }
    }
}

impl Net {
    fn init(seed: u64) -> Net {
        let mut s = seed | 1;
        let mut nrm = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            ((s >> 40) as f32 / (1u64 << 24) as f32 - 0.5) * 2.0 // [-1,1)
        };
        // Small init; the feature transformer is large+sparse so keep it tiny.
        let ft_w = (0..INPUTS * HIDDEN).map(|_| nrm() * 0.02).collect();
        let out_w = (0..OUT_INPUTS).map(|_| nrm() * 0.1).collect();
        Net {
            ft_w,
            ft_b: vec![0.0; HIDDEN],
            out_w,
            out_b: 0.0,
            m: Moments::zero(),
            v: Moments::zero(),
            t: 0,
        }
    }

    /// Forward pass; returns (pred winprob, out, stm_acc, opp_acc).
    fn forward(&self, s: &Sample) -> (f32, [f32; HIDDEN], [f32; HIDDEN]) {
        let mut acc_w = [0f32; HIDDEN];
        let mut acc_b = [0f32; HIDDEN];
        acc_w.copy_from_slice(&self.ft_b);
        acc_b.copy_from_slice(&self.ft_b);
        for &f in &s.white_feats {
            let base = f as usize * HIDDEN;
            for h in 0..HIDDEN {
                acc_w[h] += self.ft_w[base + h];
            }
        }
        for &f in &s.black_feats {
            let base = f as usize * HIDDEN;
            for h in 0..HIDDEN {
                acc_b[h] += self.ft_w[base + h];
            }
        }
        let (stm, opp) = if s.stm_white { (acc_w, acc_b) } else { (acc_b, acc_w) };
        let mut out = self.out_b;
        for h in 0..HIDDEN {
            out += stm[h].clamp(0.0, 1.0) * self.out_w[h];
            out += opp[h].clamp(0.0, 1.0) * self.out_w[HIDDEN + h];
        }
        (sigmoid(out), stm, opp)
    }

    fn loss(&self, s: &Sample) -> f64 {
        let (pred, _, _) = self.forward(s);
        let d = (pred - s.target) as f64;
        d * d
    }

    /// Accumulate this sample's gradient into `g`; returns its loss.
    fn accumulate_grad(&self, s: &Sample, g: &mut Grad) -> f64 {
        let (pred, stm, opp) = self.forward(s);
        let err = pred - s.target;
        // dL/dout for MSE on sigmoid(out): 2*err*pred*(1-pred).
        let dout = 2.0 * err * pred * (1.0 - pred);
        g.out_b += dout;
        let mut d_stm = [0f32; HIDDEN];
        let mut d_opp = [0f32; HIDDEN];
        for h in 0..HIDDEN {
            let c0 = stm[h].clamp(0.0, 1.0);
            let c1 = opp[h].clamp(0.0, 1.0);
            g.out_w[h] += dout * c0;
            g.out_w[HIDDEN + h] += dout * c1;
            // clipped-ReLU derivative is 1 strictly inside (0,1), else 0.
            if stm[h] > 0.0 && stm[h] < 1.0 {
                d_stm[h] = dout * self.out_w[h];
            }
            if opp[h] > 0.0 && opp[h] < 1.0 {
                d_opp[h] = dout * self.out_w[HIDDEN + h];
            }
        }
        let (d_acc_w, d_acc_b) = if s.stm_white {
            (&d_stm, &d_opp)
        } else {
            (&d_opp, &d_stm)
        };
        for h in 0..HIDDEN {
            g.ft_b[h] += d_acc_w[h] + d_acc_b[h];
        }
        for &f in &s.white_feats {
            let base = f as usize * HIDDEN;
            for h in 0..HIDDEN {
                g.ft_w[base + h] += d_acc_w[h];
            }
        }
        for &f in &s.black_feats {
            let base = f as usize * HIDDEN;
            for h in 0..HIDDEN {
                g.ft_w[base + h] += d_acc_b[h];
            }
        }
        (err * err) as f64
    }

    fn adam_step(&mut self, g: &Grad, batch: usize, lr: f32) {
        self.t += 1;
        let (b1, b2, eps) = (0.9f32, 0.999f32, 1e-8f32);
        let bc1 = 1.0 - b1.powi(self.t);
        let bc2 = 1.0 - b2.powi(self.t);
        let inv = 1.0 / batch as f32;
        let upd = |p: &mut f32, gv: f32, m: &mut f32, v: &mut f32| {
            let grad = gv * inv;
            *m = b1 * *m + (1.0 - b1) * grad;
            *v = b2 * *v + (1.0 - b2) * grad * grad;
            let mhat = *m / bc1;
            let vhat = *v / bc2;
            *p -= lr * mhat / (vhat.sqrt() + eps);
        };
        for i in 0..self.ft_w.len() {
            if g.ft_w[i] != 0.0 || self.m.ft_w[i] != 0.0 {
                upd(&mut self.ft_w[i], g.ft_w[i], &mut self.m.ft_w[i], &mut self.v.ft_w[i]);
            }
        }
        for i in 0..HIDDEN {
            upd(&mut self.ft_b[i], g.ft_b[i], &mut self.m.ft_b[i], &mut self.v.ft_b[i]);
        }
        for i in 0..OUT_INPUTS {
            upd(&mut self.out_w[i], g.out_w[i], &mut self.m.out_w[i], &mut self.v.out_w[i]);
        }
        upd(&mut self.out_b, g.out_b, &mut self.m.out_b, &mut self.v.out_b);
    }

    fn to_nnue_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&0x4E4E_5545u32.to_le_bytes()); // "NNUE"
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(INPUTS as u32).to_le_bytes());
        out.extend_from_slice(&(HIDDEN as u32).to_le_bytes());
        for v in self
            .ft_w
            .iter()
            .chain(&self.ft_b)
            .chain(&self.out_w)
            .chain(std::iter::once(&self.out_b))
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

// --- data loading ---

fn load_samples(cfg: &Config) -> Vec<Sample> {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    // Accept either an exact file or a `prefix` whose `.partN` shards we glob.
    if std::path::Path::new(&cfg.data).is_file() {
        paths.push(cfg.data.clone().into());
    } else if let Some(dir) = std::path::Path::new(&cfg.data).parent() {
        let stem = std::path::Path::new(&cfg.data)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(&stem) {
                    paths.push(e.path());
                }
            }
        }
    }
    paths.sort();

    let mut samples = Vec::new();
    for p in paths {
        let Ok(bytes) = std::fs::read(&p) else { continue };
        for rec in bytes.chunks_exact(RECORD_LEN) {
            if samples.len() as u64 >= cfg.max_samples {
                return samples;
            }
            if let Some(s) = decode(rec, cfg.lambda) {
                samples.push(s);
            }
        }
    }
    samples
}

fn decode(rec: &[u8], lambda: f32) -> Option<Sample> {
    let mut bytes = [0u8; 34];
    bytes.copy_from_slice(&rec[0..34]);
    let packed = Packed { bytes };
    let score = i16::from_le_bytes([rec[34], rec[35]]) as f32; // white cp
    let wdl = rec[36] as i8 as f32; // white {-1,0,1}

    let stm_white = packed.side_to_move() == chess::Color::White;
    let (score_stm, wdl_stm) = if stm_white { (score, wdl) } else { (-score, -wdl) };
    let target = lambda * sigmoid(score_stm / SCALE) + (1.0 - lambda) * ((wdl_stm + 1.0) / 2.0);

    let mut white_feats = Vec::with_capacity(32);
    let mut black_feats = Vec::with_capacity(32);
    for i in 0..64u8 {
        if let Some(p) = packed.piece_at(Square(i)) {
            let (wi, bi) = feature_indices(p.color, p.piece_type, i as usize);
            white_feats.push(wi as u16);
            black_feats.push(bi as u16);
        }
    }
    if white_feats.is_empty() {
        return None;
    }
    Some(Sample {
        white_feats,
        black_feats,
        stm_white,
        target: target.clamp(0.0, 1.0),
    })
}

struct Config {
    data: String,
    out: String,
    epochs: usize,
    lr: f32,
    batch: usize,
    lambda: f32,
    seed: u64,
    max_samples: u64,
}

fn parse_args() -> Config {
    let mut cfg = Config {
        data: "data/selfplay".into(),
        out: "nets/v1.nnue".into(),
        epochs: 8,
        lr: 0.001,
        batch: 1000,
        lambda: 0.7,
        seed: 1,
        max_samples: u64::MAX,
    };
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i + 1 < args.len() {
        let v = &args[i + 1];
        match args[i].as_str() {
            "--data" => cfg.data = v.clone(),
            "--out" => cfg.out = v.clone(),
            "--epochs" => cfg.epochs = v.parse().unwrap_or(cfg.epochs),
            "--lr" => cfg.lr = v.parse().unwrap_or(cfg.lr),
            "--batch" => cfg.batch = v.parse().unwrap_or(cfg.batch),
            "--lambda" => cfg.lambda = v.parse().unwrap_or(cfg.lambda),
            "--seed" => cfg.seed = v.parse().unwrap_or(cfg.seed),
            "--max-samples" => cfg.max_samples = v.parse().unwrap_or(cfg.max_samples),
            _ => {
                i += 1;
                continue;
            }
        }
        i += 2;
    }
    cfg
}
