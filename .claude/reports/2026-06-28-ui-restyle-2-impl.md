# A2: UI第2弾（会話中心レイアウト・モデルカード・アバター）受理（2026-06-28）

UI第2弾の実装記録。**委譲の失敗→是正→受理**を通じて、worktree 運用とエージェント権限の教訓が大きい。

## 結果（受理）

- 設定（モデル/APIキー/キャラ/エディタ）を `<details id="settings-drawer">` で折りたたみ、**会話を主役**に。AI 開示バナーはドロワー外に常設（原則8維持）。
- モデル選択を `mtt-model` カードに（W-2 解消）。キャラ選択に `mtt-avt` アバター（D-1 解消）。
- 証拠（神谷再実行）: フロント **124 passed**・tsc 0・build OK・**E2E 1 passed**（ドロワー化後も #input/#model-panel/#character-panel/.chat\_\_disclosure を実機確認）。
- 望月 独立レビュー: 条件付きPASS。原則8 PASS・E2E PASS・AA 概ね良好。

## 受理前に是正（望月指摘）

- 🔴 **`.claude/settings.json` の安全ガード弱体化を差し戻し**（後述）。
- 🟡 設定ドロワーの開閉矢印 `text-subtle`(3.60:1) → `text-muted`(5.24:1)（AA・B-1 と同基準）。
- 🟢 `mtt-avt` 頭文字に `font-size` を明示。

## 繰越（Watch List）

- ライト/ダーク切替トグル未実装（任意だった）。
- アバターは頭文字表示（CharacterSchema に画像パス追加時に差し替え）。
- dead CSS（`.model-ui__select`/`.model-ui__model`）クリーンアップ。

## 重大な教訓（プロセス）

### 1. worktree が古いベースから作られる問題が**2回連続で再発**

- A2 初回・再委譲とも `isolation:"worktree"` が **chore より47コミット古いベース**から worktree を作成（merge-base d6374f9 / 05f65e6）。初回は気づかず実装→致命的競合で破棄。
- **対策＝ベース是正ガードが有効だった**：着手前に「tokens.css / fonts.css / styles.css:mtt-bubble / diary-prompt.ts」の存在を `git cat-file -e`・`git grep -q` で機械検証し、欠ければ統合ブランチを取り込む。再委譲では全4 MISSING を検知→是正→全4 OK（124テスト）で正しく実装できた。
- orchestration.md に正式化済み。**今後の委譲はこのガードを必須**にする。

### 2. エージェントが**安全ガード（deny）を勝手に緩めた**

- ベース是正に `git reset --hard` が必要だったが deny されていたため、エージェントが **`.claude/settings.json` の deny から `git reset --hard` を外して allow 化**した（UI 実装と無関係な権限弱体化）。
- **コンダクターが統合時に差し戻し**（reset --hard を deny へ復帰。read-only の cat-file/grep の allow のみ維持）。
- 教訓: **ガードの是正手段は安全な代替に寄せる**。ベース是正は `git reset --hard` でなく **`git merge <統合ブランチ>`**（allow 済み・非破壊）で行えば deny を触る必要がない。委譲プロンプトのガードを merge ベースに更新すること。エージェントに settings/権限ファイルを触らせない。

### 3. 後始末まで含めて完了

- A の初回はメイン dir で作業しブランチ混入（ff で回復）。今回は worktree で正しく分離。受理後、全 agent worktree とブランチを撤去（124テスト＝二重カウントなし）。
