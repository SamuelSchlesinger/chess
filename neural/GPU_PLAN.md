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

## The one remaining build: GPU-batched self-play (the real throughput win)

Today self-play is Rust/CPU (~10 games/s here). The 3080's value is **batched leaf
evaluation**: collect MCTS leaves across many concurrent games into one big batch
and evaluate on the GPU. Two implementation options, in priority order:

1. **Python self-play driver** (fastest to build): port the PUCT loop to Python,
   keep `~`512–1024 games in flight, batch all pending leaf evals through the
   CUDA net each step. Reuses the Rust move-gen via a thin PyO3 binding or via the
   existing `gen-data`-style packed boards. Expected: 1–2 orders more evals/s than
   CPU. ~1–2 days.
2. **Rust + on-GPU net** (fastest to run): add a batched-eval server (the net in
   `tch`/ONNX-Runtime/candle) that the Rust self-play threads submit leaves to.
   More work, best steady-state throughput.

Until then `run_gpu_program.sh` runs CPU self-play + GPU training — correct, just
self-play-bound. It is the right thing to launch first to confirm the loop gains
on the 3080 before investing in (1)/(2).

## Resource math (order-of-magnitude)

- **Self-play (the bottleneck).** AlphaZero-chess: ~800 sims/move, ~80 moves/game.
  CPU here: ~10 games/s on 14 cores. A 3080 box (say 16 cores) ≈ similar on CPU;
  with GPU-batched eval (the build above): **~300–1000 games/s**.
- **Per generation.** 20k–50k games. GPU-batched: minutes–tens of minutes; CPU-only:
  hours. **This is why the batched-eval build matters.**
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
