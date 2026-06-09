# What is running, and how to resume (read this first after a restart)

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
  20–50 generations is the actual route past SF. Same command, bigger params:
  ```bash
  uv pip install --python neural/.venv/bin/python torch   # CUDA build
  neural/launch_program.sh start 30 20000 800
  ```
  For the full speedup, also build GPU-batched-eval self-play (neural/GPU_PLAN.md).

## The strongest standalone net right now

`nets/azq_pol1.azn` (sharp tanh-correct value + one generation of policy RL):
~+35 Elo over the blank-policy warm net at 256 sims; the gain compounds with
search depth (+~64 Elo per sims-doubling, FINDINGS §5a). Play stronger by raising
`--mcts-a`/sims.
