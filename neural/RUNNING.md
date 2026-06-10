# What is running, and how to resume (read this first after a restart)

**PAUSED 2026-06-09 22:17** at the user's request. Nothing is running anywhere.
State of the world at pause:

- **3080 (`ssh samuel@desktop`, repo at `~/projects/games/chess`)**: program
  stopped after **gen1 ACCEPT +74 Elo [+53,+96]** (decisive 37.6%); gen2
  self-play was ~90% done and is NOT checkpointed (gens checkpoint atomically —
  resume redoes gen2). Checkpoints intact: `nets/gpu_gen{0,1}.azn`,
  `logs/program_state/{best.txt,gen1.done,config}`. Code+binary there are
  current (incl. multi-server). **Resume:**
  `BATCH_EVAL=1 SP_THREADS=512 NUM_SERVERS=4 BATCH_LEAVES=24 neural/launch_program.sh start 30 20000 800`
  (NUM_SERVERS=4 is the untested-at-scale GIL fix — watch the first
  `logs/eval_server.log` rates; single-server measured ~130–170k evals/s.)
- **Mac**: M4 control run finished (gens 2–20 all rejected — the volume null
  result). SF gauntlet watcher (`sfwatch` tmux) stopped; restart with
  `tmux new-session -d -s sfwatch 'neural/sf_gauntlet.sh >> logs/sf_gauntlet.log 2>&1'`.
- **Artifacts pulled to the Mac**: the 3080 gen1 corpus (20k games, 2.99M
  positions, 37.6% decisive) at `data/gpu3080_g0/` (347 MB); nets at
  `nets/remote/gpu3080_gen0.azn` + `nets/remote/gpu_gen1.azn` (the +74 net).
- **Ladder baseline (3080 gen1 @800 sims)**: −363 vs SF@100, −449 @200,
  −604 @400, saturated ~−800+ @equal 800.
- **Next experiment queued (FINDINGS §9)**: distill `data/gpu3080_g0` (root-q +
  outcome, cp-scale) into the NNUE architecture → A/B vs `nets/sf_v1.nnue`
  inside αβ — RL data vs teacher data in our strongest engine, no SF labels.

A long-running self-play RL program runs in a **tmux session named `chesstrain`**
that is independent of the Claude Code session. You can **restart/upgrade the
model freely** — training keeps going. A freshly-started (or upgraded) assistant
should read this file and run `neural/launch_program.sh status` to pick up.

## Check / control (no Claude needed)

```bash
neural/launch_program.sh status   # RUNNING? + one-line status + last log lines + current best net
neural/launch_program.sh watch    # attach live; detach with Ctrl-b then d
neural/launch_program.sh stop      # checkpointed stop
neural/launch_program.sh start     # (re)start — RESUMES from the last finished generation
```

## How it survives restarts and stops

- **Session-independent:** runs under the tmux server (owned by your login via
  launchd), not under Claude. Restarting/upgrading the model does not touch it.
- **Resumable:** each generation writes `nets/gpu_gen<N>.azn` and a marker in
  `logs/program_state/gen<N>.done`; the current best is in
  `logs/program_state/best.txt`. Relaunching skips finished generations and
  continues. Safe across machine reboots too (tmux dies on reboot → just
  `start` again; it resumes from checkpoints).
- **Logs:** everything appends to `logs/program.log`; `logs/STATUS` holds the
  current one-line phase.

## What it is doing (recipe — see FINDINGS.md for the why)

Per generation: decisive self-play (Dirichlet noise + root-q) → value-unfreeze
train (q-target, anchor-lag-1) → 200-game Elo gate (keep only ≥ +5 Elo vs best).

## Honest expectation by hardware

- **On the M4 (now):** the loop runs and is correct, but the value is pinned at
  the Stockfish-distillation ceiling, so most generations will be *rejected* by
  the gate (no real gain) — this is expected (FINDINGS §5c). It demonstrates the
  harness and yields the strongest M4-reachable net; it will **not** beat SF.
- **On the RTX 3080:** the throughput to run decisive self-play at volume over
  20–50 generations is the actual route past SF. Same command, bigger params,
  plus the (built, verified) GPU-batched leaf evaluation:
  ```bash
  git clone <this repo>   # runtime state (logs/, nets/, data/) is untracked — clean slate
  rsync -a m4:projects/games/chess/data/sp_sf* m4:projects/games/chess/data/sp2_sf* data/
       # ^ 94 MB of SF-labeled positions; required by the gen0 warm-start (FATAL if absent)
  cargo build --release
  uv venv neural/.venv && uv pip install --python neural/.venv/bin/python torch numpy  # CUDA build
  BATCH_EVAL=1 SP_THREADS=256 neural/launch_program.sh start 30 20000 800
  ```
  BATCH_EVAL=1 runs self-play leaf evals through neural/eval_server.py on the
  GPU (virtual-loss K-leaf batching x games-in-flight; see GPU_PLAN.md for the
  measured/projected throughput). Keep BATCH_EVAL=0 on the M4 — its CPU path
  is faster than MPS at these batch sizes.

## The Stockfish gauntlet (acceptance metric)

A watcher on the Mac (`tmux` session `sfwatch`, log `logs/sf_gauntlet.log`)
polls the 3080 program's accepted nets and plays each against Stockfish at a
fixed equal node budget (100 games @ 800 nodes). The log is the gap-to-SF
trajectory; the goal line is a positive score at a meaningful budget.

## The strongest standalone net right now

`nets/azq_pol1.azn` (sharp tanh-correct value + one generation of policy RL):
~+35 Elo over the blank-policy warm net at 256 sims; the gain compounds with
search depth (+~64 Elo per sims-doubling, FINDINGS §5a). Play stronger by raising
`--mcts-a`/sims.
