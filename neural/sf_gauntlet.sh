#!/usr/bin/env bash
# Stockfish gauntlet watcher — the acceptance metric for the beat-SF goal.
#
# Watches the 3080 program's accepted nets (logs/program_state/best.txt on the
# remote box) and measures each against Stockfish on a NODE-ODDS LADDER: our
# net at a fixed sims budget vs SF at {100, 200, 400, 800} nodes, 100 games
# per rung. Rationale: an equal-node match saturates (score ~1%, one draw =
# 120 "Elo") while the net is still hundreds of Elo below SF — the rung where
# we score ~50% is the high-resolution progress needle. The EQUAL-budget rung
# (800) is the headline goal-line number (GPU_PLAN "what done looks like");
# low-node rungs are progress instrumentation, NOT the goal.
#
#   neural/sf_gauntlet.sh [REMOTE] [GAMES] [SIMS]
#   tmux new-session -d -s sfwatch 'neural/sf_gauntlet.sh >> logs/sf_gauntlet.log 2>&1'
set -uo pipefail
cd "$(dirname "$0")/.."

REMOTE=${1:-samuel@desktop}
GAMES=${2:-100}
SIMS=${3:-800}           # our side's per-move search budget on every rung
LADDER=${LADDER:-"100 200 400 800"}
RPATH=projects/games/chess
SF=${SF:-$(command -v stockfish)}
mkdir -p nets/remote logs

ts() { date +"%Y-%m-%d %H:%M:%S"; }
echo "[$(ts)] gauntlet watcher up: $REMOTE, ours@$SIMS sims vs SF@{$LADDER} nodes, $GAMES games/rung, sf=$SF"

last=""
while true; do
  best=$(ssh -o BatchMode=yes -o ConnectTimeout=15 "$REMOTE" \
           "cat $RPATH/logs/program_state/best.txt 2>/dev/null" </dev/null || true)
  if [ -n "$best" ] && [ "$best" != "$last" ]; then
    name=$(basename "$best")
    if rsync -a "$REMOTE:$RPATH/$best" "nets/remote/$name" </dev/null 2>/dev/null; then
      echo "[$(ts)] new accepted net: $name — ladder vs SF, $GAMES games/rung"
      for nodes in $LADDER; do
        ./target/release/play-match --games "$GAMES" --az-a "nets/remote/$name" \
            --mcts-a "$SIMS" --engine-b "$SF" --nodes-b "$nodes" \
            --random-plies 6 --seed 11 2>/dev/null </dev/null | tail -1 \
          | while IFS= read -r l; do echo "[$(ts)] $name vs SF@$nodes: $l"; done
      done
      last="$best"
    else
      echo "[$(ts)] rsync of $best failed; will retry"
    fi
  fi
  sleep 300
done
