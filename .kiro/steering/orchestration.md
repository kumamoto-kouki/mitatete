# orchestration.md — 並行開発オーケストレーション運用

AI コンダクター（メイン Claude セッション）が配下のワーカー（subagent）を指揮し、worktree を駆使して並行開発する体制の運用ルール。人間は tmux ダッシュボードで状況を俯瞰する。

## 役割

- **コンダクター**：メインの Claude セッション。依存 wave への分解・subagent のディスパッチ・reviewer ゲート・main への統合を担う唯一の責任主体。
- **ワーカー**：`Agent` ツールで起動する subagent。実装フェーズは **worktree 隔離**、spec フェーズは **spec ディレクトリ分離**で並行作業する。
- **観測**：人間は `scripts/dev-dashboard.sh`（tmux）で worktree / git / テスト / 進捗ログを並行確認する。

## 依存 wave（並行単位）

```
Wave 1（並行可・依存なし）:  character-layer      storage-manager
Wave 2（並行可）:            model-router         diary-engine
                            (←character-layer)  (←character-layer + storage-manager)
```

- 同 wave 内のワーカーは互いに独立。Wave 2 は Wave 1 の完了（main 統合）後に着手する。

## worktree 戦略（実装フェーズ）

- feature ごとに worktree を分ける。`Agent` の `isolation:"worktree"`（自動）または手動 `git worktree add ../wt-<feature> -b feat/<feature>`。
- 各ワーカーは自 worktree のブランチ `feat/<feature>` で作業し、`src-tauri/` `src/` 等の共有ファイル衝突を物理的に回避する。
- 完了 → コンダクターが `kiro-review` でレビュー → pass のみ main へ merge → worktree 撤去。
- **同一ファイルを編集する feature は同 wave に入れない**。spec フェーズは `.kiro/specs/<feature>/` がディレクトリ分離されるため worktree 不要。

## 進捗ログ（人間の可視化用）

- コンダクター／ワーカーは `.orchestration/progress.log` に1行ずつ状態を追記する：
  `[HH:MM] <feature> | <phase> | <status>`（phase 例: requirements/design/tasks/impl/review）
- dashboard がこのログを `tail -f` する。ログは gitignore 済み（ローカル観測専用）。

## レビューゲート

- Kiro の3フェーズ承認（requirements → design → tasks → impl）を維持する。各フェーズ完了時にコンダクターがレビューし、重要判断は人間レビューを挟む。
- 並行で一気通貫に進めず、wave 境界・フェーズ境界でゲートを通す。

## 権限境界（コンダクターへの委譲）

- **プロジェクト配下（`/var/syslabo/mitatete` 配下）の read / write / exec はコンダクターの自律判断で実行してよい**（ビルド・テスト・spec生成・コード編集・ローカルツール導入など）。
- 次は必ず人間の承認を求める：
  - 外部公開（`git push` / PR / リリース / 外部送信）
  - 破壊的・不可逆操作（履歴改変・大量削除・`git reset --hard` 等）
  - 課金を伴う操作、GitHub 等の外部サービスへの書き込み・変更
  - 認証・シークレット操作（`gh auth refresh` 等の対話的認証は人間が実行する）
- マイルストーン（M1〜M5：視認できる垂直スライス）の完成時は、人間がアプリを起動して視認確認できる状態にしてから報告する。

## コミュニケーション・ゲート（連携リズム）

- 報告・相談は **wave 境界・マイルストーン境界**で行う。個々のタスク（Issue）はその範囲内で自走する。
- 例外として、次は途中でも即相談する：重要な設計分岐、前提の崩れ、想定外のエラーが収束しないとき、外部/破壊的操作が必要になったとき。
- 長時間の自走で人間の介在機会が減りすぎないよう、上記ゲートを必ず通す。

## 可視化：真マルチプロセス swarm

人間が「複数ウィンドウで各 AI が稼働している様子」を視認できるようにする。

- `scripts/swarm-up.sh <feature>...` — 各 feature の worktree（`../wt-<feature>` / `feat/<feature>`）を作り、tmux セッション `mitatete-swarm` の各ペインで**独立した worker claude プロセス**を起動する（pane0=コンダクター観測、pane1..N=各ワーカー）。
- コンダクター（メインセッション）は `tmux send-keys -t mitatete-swarm:0.<n> "<タスク>" C-m` でワーカーへタスクを投入する。ワーカーは自 worktree で実装・コミットし、結果は git / `progress.log` で確認・統合する。
- 人間は `tmux attach -t mitatete-swarm` で全ワーカーの稼働をリアルタイム視認する。
- `scripts/swarm-down.sh [<feature>...]` — swarm 終了・worktree 撤去（マージ確認後）。
- 観測専用サマリ（git/progress/build）は `scripts/dev-dashboard.sh`。swarm と併用してよい。
- 各 worker claude は独立認証・独立コンテキスト・独立課金である点に留意（コスト効率重視の作業はコンダクター直下の subagent 方式を選ぶ）。

## git 運用（commit / push の分担）

- **コミット／ローカルマージはコンダクターが行う**。worker は自 `feat/<feature>` ブランチにコミットし、wave/マイルストーン境界でコンダクターがレビューして `main` へローカルマージする。コミットは意味のある単位で、trailer に `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>` を付ける。デフォルトブランチ上では先にブランチを切る。
- **push（外部公開）は人間が実行する**。コンダクターは push せず、「コミット済み・push 可能」状態を報告する。最終公開判断は人間が保持する。
