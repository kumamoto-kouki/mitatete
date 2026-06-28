---
paths:
  - src/**/*.css
  - src/styles.css
  - src/tokens.css
  - src/fonts.css
  - design/**
  - docs/design-brief.md
---

## デザインの受理条件（Checker が機械的に確認）

テストのように一意でないデザインでも、受理は主観でなく**観測可能な証拠**で行う（委譲規律 B）。正本の体制は `.kiro/steering/orchestration.md`。

- **レンダリング結果**：各コンポーネントの HTML プレビューが実際に描画される（スクリーンショット相当）。
- **アクセシビリティ**：コントラスト比 WCAG AA（本文 4.5:1・大文字/UI 3:1）。フォーカス可視。
- **デザインシステム整合**：色/余白/タイポはトークン経由。定義外の値を直書きしない。
- **状態網羅**：hover / focus / disabled / error / empty / loading が揃う。
- **プロダクト不変条件**：**原則8（AI開示）を視覚的に隠さない**（AI を人間に偽装する表現を作らない）。最優先の受理条件。
- **実装可能性**：採用技術（Tauri v2 ＋ vanilla TS ＋素の CSS）で無理なく実装できる構造。

## /design-sync 運用（手順）

1. **ブリーフ提示**：`docs/design-brief.md` を人間が claude.ai/design に貼り、デザイン実施者（Claude Design）がデザインシステムを生成。
2. **レビュー**：デザイン・レビュアー（独立 subagent）が上記受理条件で点検し受理/差し戻し。
3. **同期**：受理後 `/design-sync`＋`DesignSync` で **1 コンポーネントずつ**取り込む（一括置換しない＝既存資産の破壊を避ける）。差分はエンジニアリング・レビューも通す（二重ゲート）。
4. **公開判断は人間**：finalize_plan / write / push は人間承認。claude.ai/design は新規課金不要だが共通の利用枠を消費する点に留意。
