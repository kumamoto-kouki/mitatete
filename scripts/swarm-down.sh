#!/usr/bin/env bash
# swarm を終了する。tmux セッションを閉じ、指定 feature の worktree を撤去する。
#   使い方: scripts/swarm-down.sh [<feature> ...]
#   feature 省略時は tmux セッションのみ閉じる（worktree は残す）。
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARENT="$(dirname "$ROOT")"
SESSION="mitatete-swarm"

tmux kill-session -t "$SESSION" 2>/dev/null && echo "tmux セッション $SESSION を終了" || echo "$SESSION は起動していません"

for f in "$@"; do
  wt="$PARENT/wt-$f"
  if [ -d "$wt" ]; then
    git -C "$ROOT" worktree remove "$wt" --force && echo "worktree 撤去: $wt"
  fi
done
echo "完了（マージ済みかは事前に確認すること）。"
