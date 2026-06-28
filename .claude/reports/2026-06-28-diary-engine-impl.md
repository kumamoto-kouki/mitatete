# diary-engine 実装・受理（2026-06-28）

観察日記エンジン（diary-engine）を委譲実装→独立レビュー→統合した記録。コンダクター・オーケストレーション（Maker-Checker）の実運用ケース。

## 体制と流れ

- **実施者（拓・実装エージェント）**: worktree `feat/diary-engine` で TDD 実装（`60fab10`）。
- **独立レビュアー（健）**: 別エージェントで敵対的レビュー＝**条件付きPASS**。証拠を自分で再実行。
- **コンダクター（司）**: 統合 → 必須是正 → 証拠再生成 → 受理。

## 実装（要件カバレッジ）

| タスク                         | 実装                                                        | テスト      |
| ------------------------------ | ----------------------------------------------------------- | ----------- |
| 1.1 詳細度判定・日記プロンプト | `src/diary-prompt.ts`                                       | 38          |
| 2.1 当日日記の生成中核         | `src/diary.ts`                                              | 7           |
| 3.1 日記パネル UI 配線         | `src/diary.ts initDiaryPanel()` + `index.html #diary-panel` | モック      |
| 4.1 生成・保存・縮退の検証     | —                                                           | 全 105 pass |

設計判断（rules 準拠）: 生成と保存を分離（保存失敗で生成を捨てない `saved:false`）／原則8 はプロンプト末尾の AI 明示で担保／`generate_text` は当日履歴を messages として渡す汎用入口／Tauri camelCase↔snake_case。

## 証拠（コンダクター再実行・統合後 main）

- `pnpm test`: **105 passed（10 files）**
- `pnpm check`（tsc）: **0 エラー**
- `cargo test`（src-tauri）: **81 passed**
- `pnpm vite build`: 成功

## 独立レビューで捕捉された是正（受理ゲートが機能した証拠）

自己申告では漏れていた項目を健が検出 → 司が是正:

- **必須（マージ前・是正済）**: `spec.json` を `tasks-approved`／`ready_for_implementation:true` に、`tasks.md` 全チェックを `[x]` に（プロセスとコードの乖離を解消）。
- **品質デット（Watch List・次スプリント）**:
  - 🟡 `diary-prompt.test.ts` の要件4.1 アサーションが弱い（「感情の模倣」禁止を未検証）→ 強化する。
  - 🟡 `diary.ts` の `read_history` throw 経路（I/O 失敗→no_history 縮退）のテスト未整備 → 追加する。
- **未確認（環境制約）**: tasks 3.1 の実機 `pnpm dev` 目視（ボタン→日記表示）。日記パネルの CSS（`.diary-panel` 等）はデザインレーン A の担当で未定義（機能は動くが素のまま）。

## 学び

- **独立レビューは「証拠の再実行」を必須にすると効く**: 健はテストを自分で回し、かつ spec.json/tasks.md の追跡漏れという“テストに出ない”プロセス乖離を捕捉した。コンダクターの一次受理だけでは見落としやすい。
- **worktree は撤去まで含めて完了**: 残置すると vitest が二重カウント（210）した。撤去で 105 に是正。将来は vitest 側で `.claude/worktrees` を除外しておくと安全。
- **原則9（日記 ON/OFF=diaryEnabled）の編集 UI は character-layer の責務**で本 spec 外。試用には `diaryEnabled:true` のキャラ保存が要る（繰越メモ）。
