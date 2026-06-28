# この design/ の出所

claude.ai/design で生成した「Mitatete Design System」を、`DesignSync`（read）で repo に取り込んだもの（2026-06-28）。

- 元プロジェクト: `https://claude.ai/design/p/48e4b59e-4a4e-40eb-a8bd-36e60816277d`（種別: 通常プロジェクト）
- ブリーフ: [docs/design-brief.md](../docs/design-brief.md)
- 体制上の位置づけ: デザイン・レーンの成果（[.kiro/steering/orchestration.md](../.kiro/steering/orchestration.md)）

## 取り込んだもの（テキスト＝統合の核）

- `styles.css`（@import エントリ）
- `tokens/`（colors / typography / spacing / radius / shadow）
- `components/`（ai-banner / chat-bubble / buttons / character-card / input / model-select）
- `readme.md`（デザインシステム規約・原則8・命名 `mtt-` 等）

## まだ取り込んでいないもの（必要なら追加取得可）

- `foundations/`（colors / typography / spacing のトークン見本）
- `states/states.html`（empty / error / loading）
- `explorations/core-directions.html`（方向比較アーカイブ）
- `index.html`（全プレビュー索引）
- `screenshots/*.png`（参考画像・バイナリ）

> 取り込みは「写し」。claude.ai/design 側を編集したら再取得が必要（自動同期ではない）。

## 次のステップ（実装は承認待ちで保留中）

これは**デザインの素材**であり、まだアプリ本体（`src/`）には未適用。適用時はデザイン・レビュー → エンジニアリング・レビューの二重ゲートを通す（orchestration.md）。トークン（`design/tokens/`）を `src/` の CSS 変数へ採用するのが第一歩。
