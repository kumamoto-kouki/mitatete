# デザインシステム取り込み・トークン適用（2026-06-28）

claude.ai/design 生成のデザインシステムを repo へ取り込み、トークンをアプリ本体へ適用した記録。

## やったこと（上から順に実施）

1. **デザインレビュー**（デザイン・レビュアー役。作者=Claude Design なので自己レビューではない）
   - 判定: **受理（GO）/実装時注意あり**。
   - 原則8（AI開示）✅・状態網羅✅・実装可能性✅・コントラスト概ねAA。
   - 注意: 外部依存2件（Google Fonts `Zen Maru Gothic` / Lucide CDN）は Tauri の CSP・オフラインで不可 → **アプリ適用時はローカルバンドル必須**。
2. **取り込み**（`DesignSync` read → `design/`）: tokens 5・components 6・foundations 3・states・readme。
3. **トークン適用**（`src/`）:
   - `src/tokens.css` を新設（colors/typography/spacing/radius/shadow）。**外部フォント @import は除外**（CSP/オフライン対策。未バンドル時は system フォールバック）。
   - `src/styles.css` を全面トークン化（クラス名・構造は不変）。ボタンの塗りを `--accent` → `--accent-strong`＋`--on-accent` に変更し**文字コントラストを AA 改善**。角丸・影・状態色をトークン参照に。
4. **記録**: ダッシュボード更新履歴・`design/SOURCE.md`・本レポート・memory。

## 証拠（再実行）

- `pnpm check`（tsc）: 0 エラー
- `pnpm test`（vitest）: **61 passed**
- `pnpm vite build`: 成功

## 学び / 次へ

- **写しであり自動同期ではない**: claude.ai/design 側を編集したら再取得が必要（元 ID は `design/SOURCE.md`）。
- **適用は段階的**: 今回は「トークン採用」まで。UI を `mtt-` コンポーネント体裁へ寄せる本格リスタイルは別タスク（デザイン→エンジニアリングの二重ゲート）。
- **外部依存のローカルバンドル**（Zen Maru Gothic / Lucide）が、見出しフォントとアイコンを完全反映する前提。CSP 追加 or 同梱で対応する。
