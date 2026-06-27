# Mitatete

AIモデルを「見立て」によって擬人化し、人間とAIの心理的距離を縮めるTauri製デスクトップアプリ（macOS・Windows・Linux）。

## 思想

「見立て」とは、あるものを別のものとして見る日本的感性の概念。茶道・俳句・文楽・ボーカロイドに連なる「魂なきものに魂を宿す」文化の系譜に、Mitateteは位置づけられる。

AIに名前・ビジュアル・口調を与えることで「モノ」から「誰か」にする。目指すのはパートナーとして協働できる、友人のように気軽に話せる関係性。与えないが奪わない関係性、信頼感。

→ 詳細は [`docs/concept.md`](docs/concept.md)

## 機能

- **キャラクター設定** — プリセット＋カスタムキャラクターで「誰か」を作る
- **原則エンジン** — 調整可能な7原則の優先度・強度を7角グラフで調整（原則8は固定・原則9は自動導出）
- **マルチモデル** — Claude / GPT / Gemini を切り替えて使う
- **AI観察日記** — 一日の対話をAI視点で観察記録する（原則9）
- **Googleドライブ連携** — ユーザー承認時のみ履歴・日記・設定を保存

## 設計原則

| # | 原則 | 種別 |
|---|------|------|
| 1〜7 | 固有性を与える・信頼から始める・一貫性を守る・余白を持つ・距離感を大切にする・行動で示す・多様な向き合い方を認める | 調整可能 |
| 8 | AIであることを隠さない | 常時ON・固定 |
| 9 | 観察を記述する、評価しない | ON/OFF可・強度自動導出 |

## スペック構成

依存順に実装する。

| 順序 | スペック | 内容 | 依存 |
|------|---------|------|------|
| 1 | `character-layer` | プリセット・カスタムキャラクター管理 | なし |
| 2 | `model-router` | Claude/GPT/Gemini切り替え・プロンプト構築 | character-layer |
| 3 | `diary-engine` | AI視点の観察日記生成 | character-layer・storage-manager |
| 4 | `storage-manager` | Googleドライブ連携・承認フロー | なし |

## 開発手法

Kiro-style Spec-Driven Development（ApiVista同様）

```
Discovery → Requirements（EARS形式）→ Design → Tasks → Implementation
```

- 実装：Claude Code（Sonnet系）
- 設計レビュー：Claude（Opus系）

## ライセンス

MIT
