#!/usr/bin/env bash
# Launch / inspect / stop the self-play RL program in a tmux session that is
# INDEPENDENT of the Claude Code session — so you can restart and upgrade the
# model without interrupting training. The tmux server is owned by your login
# (launchd), not by Claude.
#
#   neural/launch_program.sh start [NGEN GAMES SIMS THREADS]   # begin (resumes if checkpoints exist)
#   neural/launch_program.sh status                            # one-line status + recent log
#   neural/launch_program.sh watch                             # attach live (Ctrl-b d to detach)
#   neural/launch_program.sh stop                              # stop (progress is checkpointed; restartable)
#
# Re-pointing to the RTX 3080: copy the repo there, `git pull`, install torch
# (`uv pip install --python neural/.venv/bin/python torch`), then
#   neural/launch_program.sh start 30 20000 800
set -uo pipefail
cd "$(dirname "$0")/.."
SESSION=chesstrain
LOG=logs/program.log
mkdir -p logs

case "${1:-status}" in
  start)
    shift
    if tmux has-session -t "$SESSION" 2>/dev/null; then
      echo "already running (tmux: $SESSION). use 'status' or 'watch'."; exit 0
    fi
    echo "[$(date +%H:%M:%S)] launching program in tmux '$SESSION' (survives Claude restarts)" | tee -a "$LOG"
    tmux new-session -d -s "$SESSION" \
      "exec neural/run_gpu_program.sh $* >> $LOG 2>&1; echo EXITED >> $LOG"
    sleep 1
    echo "started. tail: neural/launch_program.sh status   | live: neural/launch_program.sh watch"
    ;;
  status)
    if tmux has-session -t "$SESSION" 2>/dev/null; then
      echo "RUNNING (tmux: $SESSION)"
    else
      echo "NOT running (no tmux session '$SESSION')"
    fi
    echo "STATUS: $(cat logs/STATUS 2>/dev/null || echo '-')"
    echo "BEST:   $(cat logs/program_state/best.txt 2>/dev/null || echo '-')"
    echo "--- recent log ---"; tail -8 "$LOG" 2>/dev/null || echo "(no log yet)"
    ;;
  watch)
    exec tmux attach -t "$SESSION"
    ;;
  stop)
    tmux kill-session -t "$SESSION" 2>/dev/null && echo "stopped (checkpointed; relaunch to resume)" \
      || echo "was not running"
    ;;
  *) echo "usage: $0 {start|status|watch|stop}"; exit 1 ;;
esac
