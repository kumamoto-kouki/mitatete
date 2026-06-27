#!/usr/bin/env bash
# Mitatete 並行開発の観測ダッシュボード（tmux）。
# コンダクター（Claude）が並行起動するワーカーの状況を人間が俯瞰するための画面。
#   pane0: worktree 一覧 + 各 worktree の git 変更状況
#   pane1: 進捗ログ（.orchestration/progress.log）の追尾
#   pane2: 全ブランチ横断のコミット流れ
#   pane3: 自由ペイン（cargo test / tauri dev など）
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SESSION="mitatete-dev"
LOG="$ROOT/.orchestration/progress.log"

mkdir -p "$ROOT/.orchestration"
[ -f "$LOG" ] || : >"$LOG"

if tmux has-session -t "$SESSION" 2>/dev/null; then
  exec tmux attach -t "$SESSION"
fi

tmux new-session -d -s "$SESSION" -c "$ROOT" -n dashboard

# pane0: worktree + git 状態
tmux send-keys -t "$SESSION":0.0 "watch -t -n2 'bash scripts/_wt-status.sh'" C-m

# pane1: 進捗ログ追尾
tmux split-window -h -t "$SESSION":0 -c "$ROOT"
tmux send-keys -t "$SESSION":0.1 "tail -n 100 -f '$LOG'" C-m

# pane2: ブランチ横断のコミット流れ
tmux split-window -v -t "$SESSION":0.1 -c "$ROOT"
tmux send-keys -t "$SESSION":0.2 "watch -t -n3 'git log --all --oneline --decorate -20'" C-m

# pane3: 自由ペイン
tmux split-window -v -t "$SESSION":0.0 -c "$ROOT"
tmux send-keys -t "$SESSION":0.3 "echo 'free pane: cargo test / pnpm tauri dev など'" C-m

tmux select-layout -t "$SESSION":0 tiled
tmux attach -t "$SESSION"
