---
paths:
  - src/**
---

## フロントエンド（TypeScript / vanilla）の設計判断（character-layer / model-router で確立）

- **Tauri コマンド呼び出しは JS camelCase ↔ Rust snake_case**。`invoke("send_message", { schemaJson, historyJson })` が Rust `send_message(schema_json, history_json)` に対応。理由: Tauri v2 が自動変換するため、JS 側は camelCase で渡す。
- **「成功時のみ永続化」はフロントが orchestrate する**。バックエンドのモデル生成（`generate`）は汎用に保ち履歴を保存しない。`chat.ts` が応答成功時のみ `save_history` を呼ぶ。理由: 生成エントリを diary 等で再利用でき、日付/storage 結合を生成側に持ち込まない。
- **「応答の成功」と「保存の成功」を分離する**。`send_message` 成功後に `save_history` が失敗しても応答は破棄せず、`{text, saved:false}` で返し会話を継続する（保存失敗は警告にとどめる）。理由: 保存失敗で実際に得たモデル応答を失う／「送信失敗」と誤表示するのを防ぐ（QA-R1 の実バグ）。
- **`aiDisclosure`（原則8）はユーザー入力で上書きできない**。`CharacterValidator.validate` が候補値を一切参照せず固定文言を強制付与し、store の save/init でも再 validate する。理由: 原則8は全 spec の核心。二重に担保する。
- **別ウィンドウ間の状態は Tauri イベントで連携する**。main と character は別 webview＝別 JS コンテキストでモジュール store を共有できない。同一ウィンドウ内は store を直接 `subscribe`、別ウィンドウへは `emit`/`listen`（`character:changed`）。理由: store 共有を仮定すると別窓がサイレントに更新されないバグになる。
- **store はモジュール singleton ＋ `subscribe`（unsubscribe 返却）**。`Map<id, T>` + `activeId` + `listeners: Set`。setActive/save/delete/init 完了時に購読者へ通知。`setActive` はユーザー操作起点限定（設計コメントで明示、実行時強制はしない）。
