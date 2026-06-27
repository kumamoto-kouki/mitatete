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
