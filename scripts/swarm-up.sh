#!/usr/bin/env bash
# 真マルチプロセスの並行開発 swarm を起動する。
#   使い方: scripts/swarm-up.sh <feature1> [<feature2> ...]
#
# 各 feature ごとに git worktree（../wt-<feature>, ブランチ feat/<feature>）を作り、
# tmux セッション "mitatete-swarm" の各ペインで独立した claude（ワーカー）を起動する。
#   - pane0: コンダクター観測（progress.log の追尾）
#   - pane1..N: 各 worktree で起動した worker claude（独立プロセス）
# コンダクター（メインセッション）は `tmux send-keys -t mitatete-swarm:0.<n> "タスク" C-m`
# でワーカーにタスクを投入し、人間は `tmux attach -t mitatete-swarm` で全 AI を視認する。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARENT="$(dirname "$ROOT")"
SESSION="mitatete-swarm"

[ "$#" -ge 1 ] || {
  echo "usage: scripts/swarm-up.sh <feature> [<feature> ...]" >&2
  exit 1
}
features=("$@")

mkdir -p "$ROOT/.orchestration"
[ -f "$ROOT/.orchestration/progress.log" ] || : >"$ROOT/.orchestration/progress.log"

# worktree を用意（既存ならスキップ）
for f in "${features[@]}"; do
  wt="$PARENT/wt-$f"
  if [ ! -d "$wt" ]; then
    git -C "$ROOT" worktree add "$wt" -b "feat/$f" 2>/dev/null \
      || git -C "$ROOT" worktree add "$wt" "feat/$f"
    echo "worktree 作成: $wt (feat/$f)"
  fi
done

# 既存セッションは作り直す
tmux kill-session -t "$SESSION" 2>/dev/null || true

# pane0: コンダクター観測
tmux new-session -d -s "$SESSION" -c "$ROOT" -n swarm
tmux send-keys -t "$SESSION":0.0 \
  "echo '== conductor / observer ==' && tail -n 50 -f .orchestration/progress.log" C-m

# 各 worktree に worker claude を起動
idx=1
for f in "${features[@]}"; do
  tmux split-window -t "$SESSION":0 -c "$PARENT/wt-$f"
  tmux send-keys -t "$SESSION":0.$idx \
    "echo '== worker: $f  (wt-$f / feat/$f) ==' && claude" C-m
  tmux select-layout -t "$SESSION":0 tiled
  idx=$((idx + 1))
done
tmux select-layout -t "$SESSION":0 tiled

echo ""
echo "swarm 起動完了。視認: tmux attach -t $SESSION"
echo "コンダクターからの投入例: tmux send-keys -t $SESSION:0.1 \"<タスク>\" C-m"
