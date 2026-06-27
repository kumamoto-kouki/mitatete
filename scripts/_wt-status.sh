#!/usr/bin/env bash
# worktree 一覧と各 worktree の git 変更状況を出力する（dashboard の watch 用）。
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

echo "### git worktrees"
git worktree list
echo
git worktree list --porcelain | awk '/^worktree /{print $2}' | while read -r w; do
  branch=$(git -C "$w" rev-parse --abbrev-ref HEAD 2>/dev/null)
  echo "== ${w}  [${branch}] =="
  git -C "$w" status -s
done
