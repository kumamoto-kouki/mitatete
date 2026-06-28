# Mitatete Design System

AI モデルを「見立て（mitate）」によって擬人化し、人と AI の心理的距離を縮めるデスクトップアプリ **Mitatete**（Tauri v2 製）のためのデザインシステム。

採用方向：**A「ぬくもり / Hearth」** — 角丸・やわらかい影・親しみ前面。喫茶店の常連席のような安心感。見出しに丸ゴシック（Zen Maru Gothic）で体温を持たせる。

---

## 最重要原則 — 原則8（AI開示）

- どの画面でも「これは AI である」ことを **隠さない／偽装しない**。擬人化はするが人間のフリはさせない。
- 会話画面には常設の **AI 開示バナー**（`components/ai-banner.html`）を必ず置く。装飾で消してよい要素ではなく、**機能的に必須**。
- AI 発話のバブルには小さな「AI」ラベルを **必ず** 添える（`components/chat-bubble.html`）。

---

## 技術前提

- Tauri v2 ＋ **素の TypeScript ＋素の CSS**（React 等の重い UI フレームワークは使わない）。
- 全コンポーネントは **プレーンな HTML/CSS で実装可能**。色・余白・タイポは直書きせず、**CSS 変数（トークン）を参照**。
- アクセシビリティ：**WCAG AA**（本文 4.5:1・大きな文字/UI 3:1）。フォーカスリングは可視（`--focus-ring`）。
- ライト／ダーク両対応（トークンは2層：primitives → semantic）。`[data-theme="dark"]` で明示切替、未指定時は OS（`prefers-color-scheme`）に追従。

---

## CONTENT FUNDAMENTALS（コピーの書き方）

- トーン：温かい・親しみやすい・やわらかい。けれど誠実。敬体（です・ます）中心、押しつけない。
- 一人称はキャラクター名（例：「クロ」）、相手は「あなた」を多用しすぎず自然に。
- 例：「今日はどうされました？」「よければ、いちばん不安な場面を一緒に整理してみませんか。」
- AI 開示は事実をまっすぐ：「これは AI による応答です。人の発言ではありません。」
- 絵文字は使わない（アイコンは Lucide 線画で表現）。誇張・煽りの語彙は避ける。

---

## VISUAL FOUNDATIONS

- **カラー**：温かいベージュ/ブラウン基調。背景 `--bg #faf8f4` / カード `--surface #fff` / 本文 `--text #2b2622` / 補助 `--text-muted`（AA 確保のため提示 `#8a817a` を `#6f675f` に微調整）/ 罫線 `--border #e6e0d8`。差し色 `--accent #b08968`。文字を載せる塗りは深いコーヒー `--accent-strong #6f4e37`（白文字で AA 通過）。状態色：成功 `#2e7d52` / 注意 `#b9842a` / 危険 `#b00020` / 情報 `#3a6ea5`。
- **タイポ**：見出し `--font-display`（Zen Maru Gothic）、本文 `--font-base`（`system-ui, "Hiragino Kaku Gothic ProN", "Noto Sans JP"`）、等幅 `--font-mono`。標準本文 14px。
- **角丸**：大きめでやわらか。`--radius-sm 10 / md 16 / lg 22 / bubble 18`。
- **影**：広めでやわらかい（暖色のにじみ）。`--shadow-sm / card / md / lg`。ダークは黒ベースに切替。
- **カード**：白面＋1pxの淡い罫線＋やわらかい影＋大きめ角丸。選択時は `--accent` の罫線＋ `--accent-soft` 面。
- **アバター**：やわらかいフラット（円＋淡いトーン＋線画アイコン）。実イラスト差し替え前提のプレースホルダ。
- **ホバー/フォーカス/プレス**：hover=面を淡く（`--accent-soft`）/罫線濃く、focus=`--focus-ring` の外側ハロー、press=`translateY(1px)`。
- **背景**：単色（`--bg`）。グラデーションや過剰な装飾は使わない。

---

## ICONOGRAPHY

- **Lucide**（線画・統一ストローク）を CDN から使用：`https://unpkg.com/lucide@latest`。各プレビューで `data-lucide="..."` ＋ `lucide.createIcons()`。
- アプリ実装時は lucide を npm/ローカルバンドルして同等の名前で使用可。
- 絵文字は不使用。AI 開示やステータスも線画アイコン＋テキストで表現。

---

## ファイル索引（マニフェスト）

- `styles.css` — 消費側が link する唯一のエントリ（中身は `@import` のみ）。
- `tokens/` — `colors.css` / `typography.css` / `spacing.css` / `radius.css` / `shadow.css`。
- `foundations/` — トークン見本：`colors.html` / `typography.html` / `spacing.html`。
- `components/` — `ai-banner.html` / `chat-bubble.html` / `buttons.html` / `model-select.html` / `character-card.html` / `input.html`。
- `states/` — `states.html`（empty / error / loading）。
- `explorations/` — `core-directions.html`（方向A/Bの比較アーカイブ・参考）。
- `index.html` — 全プレビューへの索引ページ。

各コンポーネントのプレビューは **1ファイル自己完結**（その component の CSS をファイル内に持ち、トークンは `styles.css` 経由で参照）。`/design-sync` で 1 個ずつ取り込みやすい構成。先頭に `<!-- @dsCard group="..." -->` 見出しコメント付き。

---

## 共通クラス命名

`mtt-` プレフィックス。例：`.mtt-btn` / `.mtt-bubble` / `.mtt-ai-banner` / `.mtt-ai-label` / `.mtt-model` / `.mtt-char` / `.mtt-input`。状態は `.is-selected` / `.is-disabled` / `.is-focus` / `.is-hover`。
