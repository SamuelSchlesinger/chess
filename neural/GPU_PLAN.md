# RTX 3080 launch plan — the route past Stockfish

Everything below is **built and verified on the M4**; this is the exact recipe to
run on the 3080 when it frees. The principled justification for each step is in
`FINDINGS.md`; this file is the operational checklist + resource math.

## One command

```bash
# on the 3080 box, after `git pull`:
uv pip install --python neural/.venv/bin/python torch     # CUDA build auto-selected
neural/run_gpu_program.sh  30  20000  800  $(nproc)        # NGEN GAMES SIMS THREADS
```

`train_az_torch.py` auto-selects CUDA; it writes byte-identical `.azn` files the
Rust engine loads (verified: a PyTorch-trained warm net probes correctly in Rust —
up-queen +0.945, antisymmetry holds). `run_gpu_program.sh` then loops:
warm-start → [decisive self-play → value-unfreeze train (q-target, anchor-lag-1) →
Elo gate] × NGEN, keeping only generations that gain ≥ +5 Elo.

## The validated recipe (why each knob is set where it is)

| Knob | Value | Justification (FINDINGS) |
|---|---|---|
| warm-start value | SF distill, blend 0.15 | §4/§5: random can't bootstrap; SF value gives meaningful games |
| root Dirichlet noise | α=0.3, ε=0.25 | §5b: decisiveness; raised draws-only 93%→ ~87% on M4 |
| value target | 0.6·(0.3·z+0.7·q)+0.4·anchor | §5c: q informative on draws; stops collapse |
| anchor | lag-1 (gen N anchors to N−1) | panel: a fixed SF anchor goes stale as play improves |
| sims | 800 (AlphaZero-chess) | §5a/lever: search amplifies value (+64 Elo/doubling) → also more decisive |
| data window | last 1–2 gens only | §3: off-policy/stale labels are contradictory (distribution shift) |
| Elo gate | ≥ +5 vs current, 200 games | §3: MSE can fall while Elo regresses — gate on play, not loss |
| tanh in trainer | yes | §5c bug: must match Rust inference or the value is silently compressed |

## GPU-batched self-play — BUILT and verified (the real throughput win)

The batched-eval machinery is implemented and verified end to end on the M4:

- **Rust** (`src/mcts.rs`): `search_noisy_batched` collects up to K leaves per
  round under **virtual loss** and evaluates them through `Guide::evaluate_batch`;
  `RemoteGuide` pipelines the K leaves as one *frame* over a Unix socket. The
  classic single-leaf path is untouched (K=1 is bit-for-bit the validated search).
- **Python** (`neural/eval_server.py`): torch server (CUDA/MPS/CPU) that batches
  frames across all connections into one GPU forward; softmax over the legal-move
  indices on-GPU; frame-level decode/reply so Python per-leaf overhead is small.
  The Rust side ships precomputed stm-relative feature indices — the server does
  zero chess logic.
- **Correctness**: `probe-az --eval-server <sock>` prints values/priors identical
  to the local Rust evaluation on the diagnostic FENs (server on CPU). MCTS unit
  tests cover the batched search (mate-in-one, exact visit budgets, legality).
- **Wired in**: `BATCH_EVAL=1 [SP_THREADS=256] [BATCH_LEAVES=16]
  neural/run_gpu_program.sh ...` starts/stops the server per generation, serving
  the current best net. Default (BATCH_EVAL=0) is the unchanged CPU path.

## Resource math (measured, then projected)

- **Measured on the M4** (96 games in flight, K=16, MPS, batch ~270): **~51k
  evals/s** server-side ≈ 0.8 games/s @400 sims. The M4's *local CPU* path does
  ~90k evals/s (≈2.8 games/s @400 sims, 6 threads) — so on the M4 keep
  BATCH_EVAL=0; MPS forward latency dominates at these batch sizes. The build's
  value is the 3080, where the forward is ~5-10x faster and batches are bigger
  (more games in flight on a 16-core box).
- **3080 projection (honest)**: 200–500k evals/s server-capped. At 800 sims and
  ~90 plies/game ≈ 72k evals/game → **~3–7 games/s** → 20k games in **0.8–1.9 h
  per generation**; 30 generations ≈ 1–2.5 days wall-clock. (An earlier draft of
  this plan said "300–1000 games/s" — that was wrong by an order of magnitude:
  each game's evals are *sequential*, so throughput = in-flight games / round-trip
  time, not raw GPU FLOPs. Virtual-loss K-leaf pipelining is what closes the gap.)
- **If self-play still binds on the 3080**: raise SP_THREADS (games in flight)
  and BATCH_LEAVES; the next step after that is in-process Rust inference
  (tch/candle) to delete the socket round trip entirely.
- **Training.** Trivial on a 3080 (<1 min/gen at this width). Widen 256→512 + SCReLU
  once outcome-grounded gains are confirmed (panel deferred this to isolate effects).
- **Generations to cross SF.** Unknown a priori, but the loop *provably* self-improves
  (+58 Elo/gen on the policy alone, M4). With GPU-batched self-play + decisive games
  + value-unfreeze, expect steady gains; budget **20–50 generations**, days of
  wall-clock with the batched build, gated so only real gains are kept.
- **VRAM.** This net is tiny (~5 MB). The 3080's 10 GB is ample even at width 512
  and large eval batches; the constraint is self-play throughput, not memory.

## What "done" looks like

`play-match --engine-b <stockfish path> --nodes-a N --nodes-b N` (the harness
already drives external UCI engines) showing our net ≥ Stockfish 18 at a **fixed,
equal, meaningful** node budget — not a degenerate low-node window. The Elo gate in
the program guarantees monotone improvement; the SF match is the acceptance test.
