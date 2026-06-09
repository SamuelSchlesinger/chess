#!/usr/bin/env bash
# The multi-generation self-play RL program — the route to exceed Stockfish.
# One command on the RTX 3080. Encodes the recipe validated on the M4 (FINDINGS.md):
#   warm-start value (SF distill) -> per generation: decisive self-play (Dirichlet
#   noise + root-q) -> value-unfreeze train (q-target + anchor-lag-1) -> Elo gate.
#
# On the 3080 the trainer (train_az_torch.py) runs on CUDA automatically. Self-play
# is Rust/CPU here; for the full throughput win, swap in GPU-batched eval self-play
# (see neural/GPU_PLAN.md). Training and gating are unchanged.
#
# Usage: neural/run_gpu_program.sh [NGEN] [GAMES] [SIMS] [THREADS]
set -euo pipefail
cd "$(dirname "$0")/.."

NGEN=${1:-30}
GAMES=${2:-20000}     # per generation; raise on the 3080
SIMS=${3:-800}        # AlphaZero-chess uses 800; deeper => more decisive
THREADS=${4:-$(nproc 2>/dev/null || sysctl -n hw.ncpu)}
PY=./neural/.venv/bin/python
VAL_DATA="data/sp_sf data/sp2_sf"     # Stockfish-labeled positions for warm-start
GATE_ELO=5                            # accept a generation only if >= +5 Elo vs current

mkdir -p nets data logs
echo "program: $NGEN gens x $GAMES games @ $SIMS sims, $THREADS threads"

# --- gen 0: warm-start the value from Stockfish evals -------------------------
BEST=nets/gpu_gen0.azn
if [ ! -f "$BEST" ]; then
  echo "=== gen0: warm-start value from SF ==="
  $PY neural/train_az_torch.py --value-data $VAL_DATA --epochs 8 --batch 8192 \
      --lr 1.5e-3 --blend 0.15 --out "$BEST"
fi

elo_of() { # parse "Elo +NN" from a play-match score line
  grep -oE 'Elo [+-][0-9]+' | grep -oE '[+-][0-9]+' | head -1
}

# --- generations -------------------------------------------------------------
for g in $(seq 1 "$NGEN"); do
  prev=$((g-1)); data="data/gpu_g$prev"; cand="nets/gpu_gen$g.azn"
  anchor="nets/gpu_gen$((g>=2 ? g-1 : 0)).azn"   # anchor-lag-1 (gen1 anchors to gen0)
  echo "=== gen$g: self-play from gpu_gen$prev ==="
  ./target/release/selfplay --games "$GAMES" --out "$data" --net "$BEST" \
      --sims "$SIMS" --threads "$THREADS" --seed "$g" 2>&1 | tail -1
  dec=$($PY - "$data" <<'PY'
import sys,glob,struct
from collections import Counter
c=Counter()
for p in sorted(glob.glob(sys.argv[1]+".part*")):
    d=open(p,"rb").read();o=0
    while o+38<=len(d):
        r=struct.unpack("b",d[o+34:o+35])[0];n=d[o+37];c[r]+=1;o+=38+n*4
t=max(1,sum(c.values()));print(f"{100*(c[-1]+c[1])/t:.1f}")
PY
)
  echo "   decisive ${dec}%  (gate >=15% for the outcome term to matter)"
  echo "=== gen$g: train (value-unfreeze q-target, anchor=$anchor) ==="
  $PY neural/train_az_torch.py --records "$data" --warm "$BEST" --anchor "$anchor" \
      --value-blend 0.6 --beta 0.3 --epochs 8 --batch 4096 --lr 5e-4 --out "$cand"
  echo "=== gen$g: Elo gate vs current best ==="
  line=$(./target/release/play-match --games 200 --az-a "$cand" --az-b "$BEST" \
      --mcts-a "$SIMS" --mcts-b "$SIMS" --random-plies 6 --seed $((100+g)) 2>&1 | tail -1)
  echo "   $line"
  elo=$(echo "$line" | elo_of)
  if [ "${elo:-0}" -ge "$GATE_ELO" ]; then
    echo "   ACCEPT gen$g (Elo $elo) -> new best"; BEST="$cand"
  else
    echo "   REJECT gen$g (Elo ${elo:-?}); keep $BEST, drop value-blend next time"
  fi
done
echo "=== PROGRAM DONE. strongest net: $BEST ==="
