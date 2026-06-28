# この design/ の出所

claude.ai/design で生成した「Mitatete Design System」を、`DesignSync`（read）で repo に取り込んだもの（2026-06-28）。

- 元プロジェクト: `https://claude.ai/design/p/48e4b59e-4a4e-40eb-a8bd-36e60816277d`（種別: 通常プロジェクト）
- ブリーフ: [docs/design-brief.md](../docs/design-brief.md)
- 体制上の位置づけ: デザイン・レーンの成果（[.kiro/steering/orchestration.md](../.kiro/steering/orchestration.md)）

## 取り込み済み

- `styles.css`（@import エントリ）
- `tokens/`（colors / typography / spacing / radius / shadow）
- `components/`（ai-banner / chat-bubble / buttons / character-card / input / model-select）
- `foundations/`（colors / typography / spacing のトークン見本）
- `states/states.html`（empty / error / loading）
- `readme.md`（デザインシステム規約・原則8・命名 `mtt-` 等）

## 意図的に未取得（必要なら追加取得可）

- `explorations/core-directions.html`（方向A/B比較アーカイブ・参考のみ）
- `index.html`（全プレビュー索引。各 component を直接開けば足りる）
- `screenshots/*.png`（参考画像・バイナリ。context コスト回避で見送り）

> 取り込みは「写し」。claude.ai/design 側を編集したら再取得が必要（自動同期ではない）。

## 次のステップ（実装は承認待ちで保留中）

これは**デザインの素材**であり、まだアプリ本体（`src/`）には未適用。適用時はデザイン・レビュー → エンジニアリング・レビューの二重ゲートを通す（orchestration.md）。トークン（`design/tokens/`）を `src/` の CSS 変数へ採用するのが第一歩。
