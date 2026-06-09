#!/usr/bin/env bash
# Stockfish gauntlet watcher — the acceptance metric for the beat-SF goal.
#
# Watches the 3080 program's accepted nets (logs/program_state/best.txt on the
# remote box) and measures each one against Stockfish at a FIXED, EQUAL,
# meaningful node budget (GPU_PLAN "what done looks like"). Runs on the Mac,
# where Stockfish is installed, fully independent of the training program.
#
#   neural/sf_gauntlet.sh [REMOTE] [GAMES] [NODES]
#   tmux new-session -d -s sfwatch 'neural/sf_gauntlet.sh >> logs/sf_gauntlet.log 2>&1'
#
# Each line of logs/sf_gauntlet.log is one gauntlet result — the gap-to-SF
# trajectory across generations.
set -uo pipefail
cd "$(dirname "$0")/.."

REMOTE=${1:-samuel@desktop}
GAMES=${2:-100}
NODES=${3:-800}          # equal budget both sides; SIMS for our MCTS = NODES
RPATH=projects/games/chess
SF=${SF:-$(command -v stockfish)}
mkdir -p nets/remote logs

ts() { date +"%Y-%m-%d %H:%M:%S"; }
echo "[$(ts)] gauntlet watcher up: $REMOTE, $GAMES games @ equal $NODES nodes, sf=$SF"

last=""
while true; do
  best=$(ssh -o BatchMode=yes -o ConnectTimeout=15 "$REMOTE" \
           "cat $RPATH/logs/program_state/best.txt 2>/dev/null" </dev/null || true)
  if [ -n "$best" ] && [ "$best" != "$last" ]; then
    name=$(basename "$best")
    if rsync -a "$REMOTE:$RPATH/$best" "nets/remote/$name" </dev/null 2>/dev/null; then
      echo "[$(ts)] new accepted net: $name — gauntlet vs SF @ equal $NODES nodes, $GAMES games"
      ./target/release/play-match --games "$GAMES" --az-a "nets/remote/$name" \
          --mcts-a "$NODES" --engine-b "$SF" --nodes-b "$NODES" \
          --random-plies 6 --seed 11 2>/dev/null </dev/null | tail -2 \
        | while IFS= read -r l; do echo "[$(ts)] $name: $l"; done
      last="$best"
    else
      echo "[$(ts)] rsync of $best failed; will retry"
    fi
  fi
  sleep 300
done
