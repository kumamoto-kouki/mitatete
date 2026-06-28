# review-checklists.md — レビュー観点チェックリスト

レビュアー（🕵🏼‍♀️ 望月／🛡️ 守屋）と統括（👨🏼‍💼 神谷）が受理前に当てる観点。**汎用のレビュー手順は `kiro-review`（Mechanical/Judgment 12 点・Severity・Verdict）、証拠ゲートは `kiro-verify-completion` が正本**。本ファイルはそれを**補完する本プロジェクト固有の所見のみ**（汎用手順は再掲しない＝重複でドリフトさせない）。分類は agent-skills（[[agent-skills-and-role-catalog]]）の `references/` を借用、中身は実レビューで繰り返し出た実績から。

## 🔐 セキュリティ

- シークレット（API キー・トークン）を**エラー型・ステータス型・ログ・Display に載せない**。照会は有無のみ（`ApiKeyStatus{has_key}`）。
- CSP は最小（`default-src 'self'`）。**外部依存はローカルバンドル**（Zen Maru Gothic を woff2 サブセット・Lucide を npm）。インライン script は CSP で不可 → `public/` の 'self' 外部 JS（FOUC 対策の theme-init はこの方式）。
- 認証・資格情報は keyring。トークン型は store 外へ出さない。
- **`.claude/**`の権限・設定ファイルを緩めない**（deny の`git reset --hard` 等）。エージェントに触らせない（A2 で settings.json を勝手に allow 化した事故）。

## ⚡ パフォーマンス

- **起動 FOUC を防ぐ**：テーマ等の初期状態は head 同期スクリプトで最初のペイント前に確定（A3 F-1）。
- バンドルサイズ：日本語フォントはサブセット同梱。不要な依存を足さない（`@vitest/browser` 等）。
- リスナー/購読は**解除する**：`listen` の unlisten を捕捉し window close で解除（R1）。`subscribe` は unsubscribe 返却。

## ♿ アクセシビリティ（a11y）

- **WCAG AA**：本文 4.5:1・大文字/UI 3:1。**小文字に `--text-subtle`(3.6:1) を使わない → `--text-muted`(5.24:1)**（B-1・繰り返し指摘）。
- フォーカス可視：`:focus-visible { box-shadow: var(--focus-ring) }`。
- ラベル/関連付け：`aria-label`・`aria-describedby`（ヒントを入力へ）。
- 状態網羅：hover / focus / disabled / error / empty / loading。

## 📜 プロダクト原則（受理の核）

- **原則8（AI開示）**：会話画面に AI 開示バナーを**常設**（markup に焼く・JS 不在でも表示）。AI 発話に「AI」ラベルを全経路で付与。`character-validator.ts` の `aiDisclosure` 固定付与ロジックは**不変**（差分ゼロを確認）。
- **原則9（観察日記）**：`diaryEnabled` の ON/OFF をユーザーが選べる。日記は AI 生成である旨を明示。

## 🧩 フロント状態（frontend-state ルール準拠）

- Tauri invoke は JS camelCase ↔ Rust snake_case。
- **「成功時のみ保存」はフロントが orchestrate**（生成と保存を分離・保存失敗で応答を捨てない）。
- 別ウィンドウ連携は Tauri イベント（`character:changed` / `theme:changed`）。store は module singleton＋subscribe。

## 🔁 プロセス（汎用は skills が正本・固有の注意のみ）

- 証拠の再生成・独立レビュー・boundary/受理判定は `kiro-review` ＋ `kiro-verify-completion` に従う（ここで再掲しない）。
- 本プロジェクト固有：**テスト合計が現行を下回ったら古ベースの疑い**（base-guard の信号）。ベース是正（`git merge`）・worktree 撤去の手順は `orchestration.md`（worktree 戦略）が正本。
