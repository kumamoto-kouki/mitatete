# A3: ライト/ダーク切替＋dead CSS 整理 受理（2026-06-28）

振り返り後に開発を再開し、**直した運用が実際に効いた（効力確認・成功）**ことを示すケース。

## 結果（受理）

- ヘッダーにライト/ダーク切替トグル（既定ライト＝「ぬくもり」基調・localStorage 永続化・`data-theme`）。A2 でカード化して不要になった旧 model-ui の dead CSS を削除。
- 証拠（神谷＝統括 再実行）: フロント **133**・tsc 0・build OK・**E2E 1 passed**。
- 望月（デザインレビュー）独立レビュー: 条件付きPASS。原則8/E2E/AA/dead CSS 全 PASS。

## 受理前に是正（望月指摘）

- 🔴 **F-1（FOUC）**：ダーク常用者の起動時にライトが一瞬ちらつく。望月の提案「head にインライン script」は**我々の CSP（`script-src 'self'`）がインラインを禁止**するため不可。**正規の道**＝`public/theme-init.js`（'self' 外部 JS・head 同期実行）で最初のペイント前にテーマ確定。**CSP も tauri.conf も触らず**＝安全レール維持。index.html / character.html に追加。
- 繰越：character.html のライブ同期（W-1・Tauri イベント要）／テストの main.ts ロジック二重化（I-1）。

## 「直した運用が効いた」確認（学習ループ step3＝成功）

振り返りで足した対策が、今回すべて機能した：

- **ベース是正は `git merge` で**（禁止の `reset --hard` 不要）→ 着手前検証で全 MISSING を検知し merge で是正。**settings.json を一切触らず**（前回の安全ガード自己改変が再発しなかった）。
- **詰まりは越えず共有**（P2）→ 桜井（デザイン実装）が character.html のクロス窓同期で詰まった点を、回避せず「相談事項」として正直に開示。
- **禁止ファイル不可侵**（P3）→ `.claude/**`・`vite.config.ts`・`tauri.conf.json`・`character-validator.ts` すべて無変更を差分で確認。
- **F-1 是正も信用の道で**→ CSP を緩める安易な手でなく、'self' 準拠の正規策を選んだ。

## 小さなヒヤリ（系の学び・blameless）

- 受理統合時に `git add -A` が**委譲 worktree ディレクトリを誤って取り込み**（埋め込みリポジトリ警告）。→ untrack＋`.gitignore` に `.claude/worktrees/` を追加して恒久対処。**教訓**: 委譲 worktree は最初から gitignore する。統合時は対象ファイルを明示 add する（`git add -A` を避ける）。
