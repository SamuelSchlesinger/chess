# Beating Stockfish — a principled program

> Status: **bootstrapping.** This is the design + resource plan. A research
> workflow (mapping the `slm-optimization` corpus → chess, surveying SOTA nets,
> analyzing where SF leaves value) is enriching the architecture/training
> sections; the resource picture and phased plan below are firm.

## 0. Honest framing

Stockfish is ~3600 Elo: NNUE evaluation + the world's most SPRT-tuned alpha-beta,
decades of work, and net training on **billions** of positions. Beating it
outright at equal conditions is a moonshot, and I won't pretend otherwise.

The goal is therefore a **research program** with two co-equal outputs:
1. An engine that is **measurably, honestly stronger than Stockfish under a
   defined, fair condition** (the condition matters — see §5), and
2. A **principled account** of *why* — what SF's NNUE leaves on the table in
   inductive bias / trainability / gradient dynamics terms, and which of our
   bets actually pay off (with negative results reported, `slm-optimization`-style).

Near-term, the defensible wins are: (a) a neural eval that decisively beats our
own handcrafted engine; (b) evidence for the inductive-bias thesis (more eval
accuracy per FLOP than NNUE); (c) superiority at **fixed low node budgets**,
where evaluation quality dominates raw speed. Full SF superiority is the north
star we steer by.

## 1. The principled thesis

Framed in your terms (inductive bias / optimization stability / gradient dynamics
→ outcome quality):

**Stockfish's NNUE is a speed-first design.** To run ~10⁸ evals/sec on CPU SIMD
inside alpha-beta, it is forced into: a big **sparse linear** feature transformer
(HalfKAv2_hm, hand-designed king-bucketed features) feeding an **incremental
accumulator**, then a **tiny quantized int8 MLP** tail. That is, by construction:
- **Minimal inductive bias.** The features are clever but hand-engineered; the
  tail is a generic 2–3 layer MLP with *no* board geometry, *no* relational
  structure between pieces, *no* symmetry/equivariance. All structure is in the
  human-chosen feature set, not learned-relational.
- **Shallow + quantized**, so its representational ceiling and its
  trainability-at-depth are deliberately capped for speed.

**Our bet:** chess evaluation is fundamentally **relational and geometric** —
king safety, pins, pawn chains, piece coordination are *relations between pieces
on a 2-D board*. The right inductive biases are therefore (i) **attention /
relational** structure over pieces-and-squares (the bias Leela's transformer nets
already exploit), and (ii) **2-D geometric positional encoding** — exactly the
group-theoretic `exp(τB)` / damped-RoPE design space your `positional-attention`
corpus maps out, adapted from 1-D sequence to the 8×8 board. A net with these
biases should reach **higher eval accuracy per parameter and per FLOP** than
NNUE's structure-free MLP.

The catch is the **speed/accuracy frontier**: a richer net is slower, so it
searches fewer nodes. Superiority requires that *better-eval × fewer-nodes >
NNUE-eval × more-nodes*. Two ways to win that trade:
- **Distillation** (the high-probability path): train the strong, slow,
  inductive-bias **teacher**, then distill it into a fast **NNUE-shaped student**
  with *better training targets and a better-informed teacher than SF uses* — so
  the student keeps NNUE's speed but inherits a better function.
- **MCTS** (the Leela path): accept far fewer, better-informed nodes with a
  policy+value net. A separate engine; higher variance.

**Trainability is the enabling condition.** Whatever net we pick, the
`slm-optimization` findings are the levers that let us train it *deeper and more
accurately than SF's recipe can*: residual scaling (`1/√depth` vs `1/depth` vs
init-only), GPT-2 residual-projection init downscale, QK-norm, loss-spike guards,
optimizer choice (Muon/AdamW/8-bit), and watching residual-stream variance growth
(`p≈0.5` random-walk) as the diagnostic. These are *exactly* the things that
decide whether a deep relational eval net trains stably or wastes the compute.

## 2. Hardware & resources

### What we have
| Resource | Spec | Role |
|---|---|---|
| **This Mac (M4 Pro)** | 14 CPU / 20 GPU cores, **48 GB unified**, Metal 4 | Data-gen now (CPU); prototyping + small/medium net training (MLX / PyTorch-MPS). 48 GB unified is a real edge for memory. |
| **RTX 3080** (`samuel@desktop`, 192.168.4.25) | 12 GB GDDR6X, Ampere SM 8.6, CUDA | The serious-training box. **Currently occupied.** Reachable (47 ms), SSH available. 12 GB is the binding size constraint (per your `hardware` corpus). |
| **The Rust engine** | ~530–748 Mnps, swappable `Evaluator` w/ NNUE hooks | Generates labeled data fast; hosts the net for measurement. |

### Data-gen capacity (measured, today)
`gen-data` does **~5.9k quiet positions/s on 4 threads** → ~**20k/s on all 14
cores** → **~70M positions/hour**, **~1.7B/day** on the Mac CPU alone. Disk:
37 bytes/record → 100 M = 3.7 GB, 1 B = 37 GB.

### What I need from you (the explicit ask)
1. **RTX 3080 access** — (a) when it frees up / a scheduling window, and (b)
   confirm I can `ssh samuel@192.168.4.25` (key/agent) and that there's a CUDA +
   PyTorch (or JAX) env I can use or create with `uv`. Until then I'll do the
   full bootstrap (data → train a baseline NNUE → validate → measure) on the M4 Pro.
2. **Permission to install tooling** locally via Homebrew: **Stockfish** (the
   benchmark target, and a candidate stronger data-labeler) and **cutechess-cli**
   (the match runner). Plus a `uv` venv with the trainer's deps.
3. **Disk budget** — ~50–100 GB for datasets/checkpoints.
4. **Training framework call** — reuse your JAX `home-cooked-slms` (great for the
   *transformer* teacher net; its RoPE/RMSNorm/attention blocks transfer), vs
   PyTorch/MLX (the NNUE student is a tiny custom net; PyTorch's `nnue-pytorch`
   lineage + quantization-aware training is the trodden path). My lean:
   **PyTorch (or MLX) for the NNUE student, your JAX stack for the teacher** — but
   I'll confirm once the research workflow reports the framework tradeoffs.
5. **Time horizon** — the bootstrap is hours on the Mac; a competitive net is
   **weeks of 3080 wall-clock**. This is a multi-session program.

## 3. Architecture direction

- **Baseline (validates the loop):** a standard NNUE — perspective feature
  transformer → accumulator → clipped-ReLU → small int8 tail. Quantized inference
  in Rust behind the `Evaluator` trait (incremental via the existing
  `on_make`/`on_unmake` hooks). Target: beat the handcrafted PeSTO engine.
- **Teacher (the thesis):** an attention/relational net over the 64 squares /
  32 pieces with 2-D geometric positional encoding (the `exp(τB)` family), trained
  with the corpus's trainability controls. Higher accuracy, slower.
- **Student:** distill the teacher into a fast NNUE-shaped net → keep speed,
  inherit a better function than SF's training gives.
- Each architectural bet is an **ablation arm with a falsifier**, decided by
  measurement, not asserted.

## 4. Training recipe (initial, to be refined by the research)
- Targets: blend **search-eval cp** (distillation) + **game WDL** (ground truth
  beyond the eval) — `loss = λ·mse(sigmoid(eval)) + (1-λ)·ce(wdl)`, standard NNUE.
- Optimizer/schedule/init/norm: start from the `slm-optimization` defaults
  (AdamW or Muon, warmup-stable-decay, GPT-2 init downscale, QK-norm, grad-clip
  1.0 + spike-skip) and ablate.
- Quantization-aware training for the NNUE student (int16 accumulator, int8 tail).

## 5. Measurement vs Stockfish (the part that keeps us honest)
- **cutechess-cli** matches, our `chess-uci` vs `stockfish`, **SPRT** for
  accept/reject, Elo with error bars.
- **Fixed nodes-per-move** is the primary condition: it isolates *evaluation +
  search quality* from raw nodes/sec (which is an implementation/SIMD race we
  won't win in Rust vs SF's hand-tuned AVX). Also report fixed-time on equal
  hardware for the "real" number.
- "Beating SF" is stated **with its condition**: e.g. "+Elo vs SF at N
  nodes/move." A win at fixed nodes is the scientifically meaningful result about
  eval quality; a win at fixed time on equal hardware is the headline.
- Tactical suites (WAC/Arasan) and an eval-accuracy metric (vs a deep-search
  oracle) track eval quality directly, decoupled from search.

## 6. Phased plan (with accept/reject gates)
| Phase | Goal | Gate to pass |
|---|---|---|
| **P0 Foundation** | data-gen ✓; NNUE inference in Rust; SF+cutechess harness | NNUE inference matches a Python reference within rounding; harness produces an Elo number |
| **P1 Baseline NNUE** | train a standard NNUE on self-play data; plug in | **beats handcrafted PeSTO engine by ≥ +100 Elo** at fixed nodes |
| **P2 Frontier map** | measure eval-accuracy-vs-nodes/sec; place vs SF | a clear, quantified frontier curve; identify the node budget where eval quality dominates |
| **P3 Inductive-bias teacher** | attention/geometry net; trainability ablations | teacher eval-accuracy beats the baseline NNUE per the oracle metric; document which biases/controls helped (and which didn't) |
| **P4 Distill + scale (3080)** | distill teacher→fast student; scale data/size | **+Elo vs Stockfish at a stated, fair condition** |

## 7. Status
- **Done:** `gen-data` self-play pipeline (P0), measured ~20k pos/s. Engine +
  swappable `Evaluator` with NNUE hooks already in place.
- **Next:** NNUE inference in Rust (P0), then the SF/cutechess harness (P0), then
  a first dataset + baseline NNUE train on the Mac (P1).
- **Blocked-on-you:** 3080 access window; install permission for SF/cutechess;
  framework call.

---

## 8. Sharpened by the research synthesis

**The binding number: ~135 ns/node.** The engine runs ~7.5 M nps single-threaded
(measured, M4 Pro), so any *in-search* evaluator gets ~135 ns/node. A board-
attention eval (d=256, L=6, 64 tokens) is ~1.3 GFLOP/eval → **100–1000× too slow**
to call per node in alpha-beta. This is decisive and forces a **two-arm** program:

- **ARM 1 — the engine workhorse:** a classic **HalfKA-style NNUE** (int16
  accumulator, int8 clipped-ReLU tail, ~10–40 M params dominated by the sparse
  feature transformer), trained in **PyTorch** (the `nnue-pytorch` lineage),
  exported **bit-exact** into our Rust `Evaluator` incremental hooks. This is the
  path to engine strength. (Our `eval::nnue` float inference + loader is the v0
  of the Rust side; the 768 feature set is the warm-up before HalfKA.)
- **ARM 2 — the science:** a deeper **attention/relational board net** where the
  `slm-optimization` depth-trainability machinery (`1/√D` residual scaling re-keyed
  to *actual* depth, QK-norm, WSD schedule, loss-spike guards) genuinely applies.
  Run as a **teacher** (distilled into ARM 1) and as a clean, fully-observable
  **learning-dynamics testbed** — the corpus's program, with a ground-truth oracle.

**The falsifiable superiority claim (narrow):** chess has **exact symmetries**
SF's HalfKA MLP doesn't structurally enforce — the color-swap antisymmetry
`eval(pos) = −eval(colorflip(pos))`, horizontal mirror, and relational
pieces-on-squares structure (attention captures it natively; SF only sees
hand-injected "Threat" features). A **symmetry-correct, distance-aware** net should
hit a given eval accuracy with **fewer params / less data** → a better *teacher* →
better-than-SF-self-referential targets a fast *student* distills into at the same
135 ns/node budget. In gradient terms: **WDL-sigmoid targets bound gradient
magnitude** vs raw-cp regression; for ARM 1 the corpus's residual-variance story is
moot (no depth) and is replaced by the real frontier the corpus never faced —
**quantization-aware training** of the int8/16 accumulator.

**Data, unblocked.** Start on the **free Lichess evals DB (~340 M SF-labeled
positions, HuggingFace `Lichess/chess-position-evaluations`)** — enough for a first
credible NNUE with zero generation. `gen-data` self-play is the *asset* for the
later anneal (curriculum: shallow labels → deep-search / self-play / tablebase-exact
in the WSD decay tail). Ladder: 1–10 M (debug the loop) → 50–200 M (first credible
net) → 0.5–2 B (competitive).

**Measurement (import wholesale, first).** Primary cheap proxy each iteration:
held-out position MSE/MAE + top-1 move-agreement vs a strong reference. Gated
ground truth: **Elo vs Stockfish at FIXED NODES** (`go nodes N`, already supported)
via cutechess-cli — fixed-*nodes* isolates eval-quality-per-node (fixed-*time*
rewards SF's hand-tuned SIMD; fixed-*depth* is incomparable). Pin matches: 1
thread, equal 64 MB hash, UHO opening book, swapped colors, SPRT(α=β=0.05). **σ
calibration first** (≥5 identical seeds for MSE σ; separately the much larger Elo σ)
— the corpus's σ=0.003 placeholder does NOT transfer. Report fixed-nodes as
"nodes-normalized Elo" + a fixed-time confirmation; never claim "beat SF" on
fixed-nodes alone.

**Honest odds (the bottom line you asked for).**
| Milestone | Claim | Odds |
|---|---|---|
| **W1** | our trained net beats our PeSTO by **>100 Elo** at fixed nodes | near-certain (first milestone) |
| **W2** | positive Elo vs **strength-reduced** SF | likely |
| **W3** | within **−200 Elo** of full SF at equal fixed nodes | serious, plausible with effort |
| **W4** | **beat full SF** at equal nodes | low-odds moonshot (single digits) |

SF's shallow/quantized/hand-featured eval is a *direct consequence* of the 135 ns
budget, trained on ~16 B SF-self-play + billions of Leela positions near a hard CPU
local optimum — not a missed opportunity. **The durable deliverable is the science:**
a fully-observable eval-quality-per-node frontier as a function of architecture and
training, valuable even landing 100–300 Elo short. Timeline: P0–P1 ~2–4 weeks
(harness + v1 loop on free data); P2–P3 ~1–3 months (own data + inductive-bias
ablations); P4 ongoing. **Biggest risk: uncritical transfer** of corpus defaults
that are keyed to a deep VRAM-bound autoregressive LM and are moot for a shallow
quantized chess regressor — apply selectively, measure everything.
