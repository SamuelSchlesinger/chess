#!/usr/bin/env bash
# The multi-generation self-play RL program — the route to exceed Stockfish.
# RESUMABLE and SESSION-INDEPENDENT: launch via neural/launch_program.sh (tmux),
# survives the Claude session being restarted/upgraded, and if it is ever stopped
# (or the machine reboots) just relaunch — it skips finished generations via the
# markers in logs/program_state/ and resumes from the last good net.
#
# Encodes the recipe validated on the M4 (FINDINGS.md):
#   warm-start value (SF distill) -> per generation: decisive self-play (Dirichlet
#   noise + root-q) -> value-unfreeze train (q-target + anchor-lag-1) -> Elo gate.
# Trainer = train_az_torch.py (auto: CUDA on the 3080, MPS on the M4). On the 3080
# also swap in GPU-batched-eval self-play for throughput (neural/GPU_PLAN.md).
#
# Usage: neural/run_gpu_program.sh [NGEN] [GAMES] [SIMS] [THREADS]
# Optional GPU-batched leaf evaluation (the 3080 throughput unlock, FINDINGS §8):
#   BATCH_EVAL=1 [SP_THREADS=256] [BATCH_LEAVES=16] neural/run_gpu_program.sh ...
# (self-play leaves then evaluate on the GPU via neural/eval_server.py; THREADS
#  becomes games-in-flight. Keep BATCH_EVAL=0 on the M4 — its CPU path is faster.)
set -uo pipefail
cd "$(dirname "$0")/.."

NGEN=${1:-20}
GAMES=${2:-700}
SIMS=${3:-400}
THREADS=${4:-$( (command -v nproc >/dev/null && nproc) || sysctl -n hw.ncpu)}
PY=./neural/.venv/bin/python
VAL_DATA="data/sp_sf data/sp2_sf"
GATE_ELO=5
STATE=logs/program_state
BESTFILE=$STATE/best.txt
mkdir -p "$STATE" logs nets data

# Batch-eval config: env wins; else the persisted config from the original
# launch (so a bare resume after a reboot keeps the same mode); else defaults.
# Under BATCH_EVAL=1 a positional THREADS arg means games-in-flight.
CFG=$STATE/config
if [ -z "${BATCH_EVAL+x}" ] && [ -f "$CFG" ]; then . "$CFG"; fi
BATCH_EVAL=${BATCH_EVAL:-0}
SP_THREADS=${SP_THREADS:-${4:-256}}
BATCH_LEAVES=${BATCH_LEAVES:-16}
printf 'BATCH_EVAL=%s\nSP_THREADS=%s\nBATCH_LEAVES=%s\n' \
  "$BATCH_EVAL" "$SP_THREADS" "$BATCH_LEAVES" > "$CFG"
SOCK="/tmp/chess_eval_$$.sock"
SERVER_PID=""

start_server() { # $1 = net to serve
  $PY neural/eval_server.py --net "$1" --socket "$SOCK" --max-batch 2048 \
      >> logs/eval_server.log 2>&1 &
  SERVER_PID=$!
  for _ in $(seq 1 240); do
    [ -S "$SOCK" ] && break
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then break; fi
    sleep 0.5
  done
  if [ ! -S "$SOCK" ]; then
    say "FATAL: eval server did not come up; last log lines:"
    tail -5 logs/eval_server.log | while IFS= read -r l; do say "  $l"; done
    status "FATAL: eval server"; exit 1
  fi
}
stop_server() {
  if [ -n "$SERVER_PID" ]; then
    kill "$SERVER_PID" 2>/dev/null
    wait "$SERVER_PID" 2>/dev/null
  fi
  SERVER_PID=""; rm -f "$SOCK"
}
trap 'rc=$?; stop_server; say "EXITED rc=$rc"' EXIT

ts() { date +"%Y-%m-%d %H:%M:%S"; }
say() { echo "[$(ts)] $*"; }
status() { echo "$*" > logs/STATUS; }

if [ "$BATCH_EVAL" = "1" ]; then
  say "program start: $NGEN gens x $GAMES games @ $SIMS sims, GPU-batched eval ($SP_THREADS games in flight, K=$BATCH_LEAVES)"
else
  say "program start: $NGEN gens x $GAMES games @ $SIMS sims, $THREADS threads (local CPU eval)"
fi

# --- gen 0: warm-start the value from Stockfish evals (idempotent) ------------
if [ ! -f nets/gpu_gen0.azn ]; then
  say "gen0: warm-start value from SF"; status "gen0: warm-starting value"
  $PY neural/train_az_torch.py --value-data $VAL_DATA --epochs 8 --batch 8192 \
      --lr 1.5e-3 --blend 0.15 --out nets/gpu_gen0.azn
fi
if [ ! -f nets/gpu_gen0.azn ]; then
  say "FATAL: gen0 warm-start did not produce nets/gpu_gen0.azn — aborting"
  status "FATAL: gen0 warm-start failed"; exit 1
fi
[ -f "$BESTFILE" ] || echo nets/gpu_gen0.azn > "$BESTFILE"
BEST=$(cat "$BESTFILE")
say "current best: $BEST"

elo_of() { grep -oE 'Elo [+-][0-9]+' | grep -oE '[+-][0-9]+' | head -1; }
decisive() {
  $PY - "$1" <<'PY'
import sys,glob,struct
from collections import Counter
c=Counter()
for p in sorted(glob.glob(sys.argv[1]+".part*")):
    d=open(p,"rb").read();o=0
    while o+38<=len(d):
        r=struct.unpack("b",d[o+34:o+35])[0];n=d[o+37];c[r]+=1;o+=38+n*4
t=max(1,sum(c.values()));print(f"{100*(c[-1]+c[1])/t:.1f}")
PY
}

for g in $(seq 1 "$NGEN"); do
  prev=$((g-1)); data="data/gpu_g$prev"; cand="nets/gpu_gen$g.azn"
  done_marker="$STATE/gen$g.done"
  if [ -f "$done_marker" ]; then BEST=$(cat "$BESTFILE"); continue; fi   # resume past it
  alag=$([ "$g" -ge 2 ] && echo "nets/gpu_gen$((g-1)).azn" || echo "nets/gpu_gen0.azn")

  say "gen$g: self-play from $(basename "$BEST")"; status "gen$g/$NGEN: self-play"
  rm -f "$data".part*   # stale shards from an aborted attempt must not mix in
  if [ "$BATCH_EVAL" = "1" ]; then
    start_server "$BEST"
    ./target/release/selfplay --games "$GAMES" --out "$data" --net "$BEST" \
        --sims "$SIMS" --threads "$SP_THREADS" --batch-leaves "$BATCH_LEAVES" \
        --eval-server "$SOCK" --seed "$g" 2>&1 | tail -1
    sp_rc=$?   # pipefail: reflects selfplay, not tail
    stop_server
  else
    ./target/release/selfplay --games "$GAMES" --out "$data" --net "$BEST" \
        --sims "$SIMS" --threads "$THREADS" --seed "$g" 2>&1 | tail -1
    sp_rc=$?
  fi
  if [ "$sp_rc" -ne 0 ] || ! ls "$data".part* >/dev/null 2>&1; then
    say "FATAL: gen$g self-play failed (rc=$sp_rc) — aborting"
    status "FATAL: gen$g self-play"; exit 1
  fi
  dec=$(decisive "$data"); say "gen$g: decisive ${dec}% (want >=15%)"

  say "gen$g: train value-unfreeze (q-target, anchor=$(basename "$alag"))"; status "gen$g/$NGEN: train"
  $PY neural/train_az_torch.py --records "$data" --warm "$BEST" --anchor "$alag" \
      --value-blend 0.6 --beta 0.3 --epochs 8 --batch 4096 --lr 5e-4 --out "$cand"
  if [ ! -f "$cand" ]; then
    say "FATAL: gen$g training produced no net — aborting"; status "FATAL: gen$g train"; exit 1
  fi

  say "gen$g: Elo gate vs best"; status "gen$g/$NGEN: gating"
  line=$(./target/release/play-match --games 200 --az-a "$cand" --az-b "$BEST" \
      --mcts-a "$SIMS" --mcts-b "$SIMS" --random-plies 6 --seed $((100+g)) 2>&1 | tail -1)
  elo=$(echo "$line" | elo_of); say "gen$g: $line"
  if [ "${elo:-0}" -ge "$GATE_ELO" ]; then
    echo "$cand" > "$BESTFILE"; BEST="$cand"; say "gen$g: ACCEPT (Elo $elo) -> new best"
  else
    say "gen$g: REJECT (Elo ${elo:-?}); keep $(basename "$BEST")"
  fi
  echo "${elo:-NA}" > "$done_marker"
done
say "PROGRAM DONE. strongest net: $(cat "$BESTFILE")"; status "done: best=$(cat "$BESTFILE")"
