# 2026-06-27 プロダクト現状・検証・引き継ぎ（次世代向け統合記録）

Wave2 実装の前に、全 spec を**証拠ベースで精査・検証**し、知見を次世代へ残すための統合記録。
コンダクターの一次精査＋**独立QAペルソナの二次検証**の両方を反映（コンダクター・オーケストレーションの「独立レビュー＋証拠で受理」の実践）。

## 1. 検証証拠（本日再実行）

| 検証                                                                | 結果                                                         |
| ------------------------------------------------------------------- | ------------------------------------------------------------ |
| `cargo test`（src-tauri）                                           | **78 passed / 0 failed**、warning 0                          |
| `pnpm test`（vitest）                                               | **60 passed / 0 failed**（7ファイル。QA-R1 テスト追加で +1） |
| `pnpm check`（tsc）                                                 | EXIT 0                                                       |
| `cargo build` / `pnpm vite build`                                   | 成功・warning 0                                              |
| semgrep（model_router/key_manager/storage/chat/model-ui/validator） | findings 0 / errors 0（QA実行）                              |
| CSP smoke（`pnpm tauri dev` 実機・WSLg）                            | 起動・storage初期化・vite200・**CSP違反0**                   |

## 2. spec 別ステータス

| spec            | phase           | 承認(req/design/tasks) | 実装                            | テスト             |
| --------------- | --------------- | ---------------------- | ------------------------------- | ------------------ |
| storage-manager | tasks-approved  | ✓/✓/✓                  | **完了**（全23）                | Rust 56            |
| character-layer | tasks-approved  | ✓/✓/✓                  | **完了**（全18, フェーズ2含む） | vitest 54          |
| model-router    | tasks-generated | ✓/✓/✓                  | **完了**（全18）                | Rust 22 + vitest 6 |
| diary-engine    | tasks-generated | ✓/✓/✗                  | **未着手**（spec のみ）         | —                  |

> 記録是正（本日）: character-layer / storage-manager の tasks.md で**大タスク見出しが未チェック**だったのを `[x]` に修正（実装は完了済み・サブタスクは既に `[x]`）。

## 3. 独立QA再検証の結果と是正

判定 **条件付きGO → 是正済み**。

| #   | 重大度          | 事象                                                                         | 対応                                                                                                                    |
| --- | --------------- | ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| R1  | 🟡 実バグ       | `chat.ts`: send 成功後の `save_history` 失敗が「送信失敗」と誤表示し応答欠落 | **修正済**: 応答取得と保存を分離（`{text, saved}`）。保存失敗は会話を壊さず警告。テスト追加（frontend 60）              |
| R2  | 🟡 設計乖離     | design.md は「backend が save_history」だが実装は frontend                   | **修正済**: design.md を frontend 保存に整合（実装整合メモ・図・トレーサビリティ更新）                                  |
| R3  | 🟡 セキュリティ | `tauri.conf.json` の `csp: null`                                             | **修正済**: 最小 CSP 設定（`default-src 'self'`＋各モデルAPI/GDrive/dev origin の connect-src）。実機 smoke で違反0確認 |
| R4  | 🟢              | OpenAI `max_tokens`（新モデルは `max_completion_tokens` 要求の場合あり）     | **繰越**（将来 provider 別パラメータ分岐）                                                                              |

**QA が確認した不変条件（問題なし）**: aiDisclosure 常時挿入（validator + build_system_prompt 両系・文言一致）／API キー秘匿（`get_api_key_status` は有無のみ・`ApiKeyStatus`/`ModelError`/`StorageError` に平文なし）／成功時のみ履歴保存（reject 経路は save 未到達）／別ウィンドウ連携（Tauri イベント）／diary-engine 未実装記録の正確性。

## 4. 要件↔実装カバレッジ（要点）

- storage-manager: 要件1〜4を `LocalFileSystem`/`OAuthManager`/`GDriveClient`/`StorageManager` がカバー。trait シーム＋モックで全経路テスト。
- character-layer: 要件1〜6を validator/store/ui/editor/visual-editor/principles/character がカバー。原則8不変・最後に使用したキャラ復元・破損縮退を検証。
- model-router: 要件1〜6（4.3 除く）を型/プロンプト/key_manager/provider×3/ModelRouter/コマンド/フロントがカバー。4.3 ストリーミングは **MVP範囲外（拡張点予約）**。
- diary-engine: 要件1〜6を design/tasks 化済み。実装は未（下記前提あり）。

## 5. 繰越・既知ギャップ（一元化）

- **diary-engine 実装の前提（cross-spec）**: 本文生成は model-router の汎用 `generate` を使う。現状 `generate` は Rust メソッド止まりで、**任意 system＋履歴→text の汎用生成 Tauri コマンドを model-router に1つ追加**してから diary 実装に着手（`send_message` はキャラ用 system 構築のため流用不可）。実装順はユーザー決定で「model-router 完成後」。
- **M3 サインオフ未**: 実 API キー＋`pnpm tauri dev` で「モデルと実対話」を目視確認（GUI 必須）。M4/M5 未着手。
- **OpenAI/Gemini 実 wire 未検証**: クライアントはモックのみ。実エンドポイント応答パース・実モデルID（model-ui の default `gpt-4o`/`gemini-1.5-pro` は暫定）は実機要検証。R4（max_tokens）も併せて。
- **storage**: GDrive サブフォルダ（`mitatete/history/...`）未対応・OAuth 実資格情報疎通未（env 空）。
- **未自動化の検証**: DOM 実描画・keyring 実動作・OAuth/GDrive 実フロー・release/`tauri build`。→ 承認済みライブラリ（happy-dom/insta/tauri-driver+WebdriverIO）導入で順次自動化予定。
- **GitHub Project**: model-router/diary-engine 未登録（Wave1 のみ）。
- **要件4.3 ストリーミング**: MVP 範囲外（`ModelProvider` に拡張点コメント）。

## 6. マイルストーン地図

| M   | 内容                 | 状態                                              | 依存 spec       |
| --- | -------------------- | ------------------------------------------------- | --------------- |
| M1  | アプリ起動+2窓表示   | **達成**                                          | bootstrap       |
| M2  | キャラ選択が UI 反映 | **サインオフ済**（character window 実機ログ実証） | character-layer |
| M3  | モデルと実対話       | **未**（実APIキー+GUI目視）                       | model-router    |
| M4  | 観察日記生成         | 未（diary 実装＋汎用生成コマンド）                | diary-engine    |
| M5  | GDrive 同期          | 未（OAuth 実資格情報）                            | storage-manager |

## 7. 次世代への引き継ぎ（このセッションで整備）

- **`.claude/rules/`**: 3 spec で繰り返した設計判断を rules 化（rust-backend / frontend-state）。glob で対象ファイル編集時に自動読込。
- **`.kiro/steering/orchestration.md`**: 運用モデル「コンダクター・オーケストレーション」を正式化（一般用語・委譲規律＝さぼり/肩代わり/隠蔽の防止）。
- **memory**: 作業様式（コンダクター・オーケストレーション）・進捗を記録。
- 各 spec の実装振り返りは `.claude/reports/2026-06-27-*-impl.md` を参照。

## 8. 推奨する次アクション（優先順）

1. **push**（未公開コミットを origin へ・人間判断）。
2. **検証自動化ライブラリ導入**（happy-dom/insta/tauri-driver）— 未自動化ギャップを証拠化。
3. **M3 実機サインオフ**（実 API キー）。
4. **diary-engine 実装**（先に model-router へ汎用生成 Tauri コマンドを追加）。
