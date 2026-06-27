# 実装計画

## タスク一覧

- [ ] 1. 基盤：モジュール・共通型・依存
- [ ] 1.1 model-router の土台（依存・モジュール・共通型・エラー型）を用意する
  - `async-trait` 依存を追加し、`model_router`・`key_manager` モジュールを宣言する（既存の `reqwest`/`serde`/`keyring`/`tokio` を再利用）
  - `Provider`（Claude/OpenAI/Gemini）・`ChatRequest`・`ChatResponse`・`ChatMessage`・`PromptCharacter`（TS CharacterSchema の部分ミラー：name/tone/aiDisclosure/principleDefaults）を定義する
  - `ModelError`（ApiKeyMissing/Http/Network/Decode/Keyring）を `thiserror`＋serde Serialize で定義する（storage の `StorageError` と同型）
  - `cargo build` が通り、`schema_json` を `PromptCharacter` へ `serde_json` でデシリアライズできることを単体テストで確認できる状態にする
  - _Requirements: 5.1_

- [ ] 2. コア：システムプロンプト構築
- [ ] 2.1 (P) PromptCharacter と原則値からシステムプロンプトを構築する
  - `あなたは「{name}」です。{tone}\n行動指針：\n- {原則ガイドライン}\n{aiDisclosure}` の構造を生成する（tech.md「プロンプト構造」準拠）
  - 原則ガイドラインを優先度・強度（1〜5）順に生成し、強度の低い原則は省略する
  - **原則8 `aiDisclosure` を必ず末尾に付与**し、空文字でも character-layer と同一の固定文言にフォールバックする（ユーザー入力で上書き不可）
  - 任意の入力で出力に `aiDisclosure` が常に含まれること・name/tone/原則ガイドラインが反映されることを単体テストで確認する
  - _Requirements: 2.1, 2.2, 2.3, 2.4_
  - _Boundary: build_system_prompt_

- [ ] 3. コア：API キー秘匿管理
- [ ] 3.1 (P) API キーの保存・照会と保護コマンドを実装する
  - provider ごとに API キーを OS キーチェーン（keyring）へ保存・取得・有無判定する（GDrive OAuth トークンと別名前空間）
  - キー保存の Tauri コマンドと、**有無のみを返す**照会コマンド（平文キーを返さない）を公開する
  - 平文キーはフロント・ログ・対話履歴へ出力しないことをコメント／テストで担保する
  - 保存→照会で有無が反映され、照会コマンドの戻り値に平文キーが含まれないことを単体テストで確認する
  - _Requirements: 3.1, 3.2, 3.3_
  - _Boundary: key_manager_

- [ ] 4. コア：provider クライアント
- [ ] 4.1 Claude provider と共通 trait を実装する
  - `ModelProvider` trait（非ストリーミング `send`）を定義し、`HttpExecutor`（storage の既存シーム）を再利用する
  - Anthropic Messages API（`x-api-key`・`anthropic-version: 2023-06-01`・`system` トップレベル・`max_tokens` 必須・既定モデル `claude-opus-4-8`）へ送信し、`content[].text` を連結して応答テキストを得る
  - 受領した history ＋新規 message を `messages` 配列へ反映する
  - `HttpExecutor` モックで、上記ヘッダ・`system`・`max_tokens` を含むリクエストが生成され、応答テキストが連結されることを単体テストで確認する
  - _Requirements: 4.1, 4.2_
  - _Boundary: ClaudeClient, ModelProvider_
  - _Depends: 1.1_

- [ ] 4.2 (P) OpenAI provider を実装する
  - `chat/completions` へ `Authorization: Bearer` で送信し、system を `messages` 先頭の `{role:"system"}` として渡す（wire 形式は実装時に公式ドキュメントで確定）
  - 応答テキストを抽出し、`ModelProvider` 契約を満たす
  - `HttpExecutor` モックで Bearer ヘッダと system メッセージ位置を含むリクエスト生成を単体テストで確認する
  - _Requirements: 4.1, 4.2_
  - _Boundary: OpenAIClient_
  - _Depends: 4.1_

- [ ] 4.3 (P) Gemini provider を実装する
  - `:generateContent` へ API キー（ヘッダ／クエリ）で送信し、`systemInstruction`＋`contents` 形式へマップする（wire 形式は実装時に公式ドキュメントで確定）
  - 応答テキストを抽出し、`ModelProvider` 契約を満たす
  - `HttpExecutor` モックで `systemInstruction`／`contents` を含むリクエスト生成を単体テストで確認する
  - _Requirements: 4.1, 4.2_
  - _Boundary: GeminiClient_
  - _Depends: 4.1_

- [ ] 5. 統合：ModelRouter と Tauri コマンド
- [ ] 5.1 ModelRouter（選択・ルーティング・履歴連携・エラー処理）を実装する
  - アクティブ provider／モデルを内部可変性（Mutex/RwLock）で保持し、**ユーザー操作起点でのみ**切替・自動変更しない。切替は次リクエストから反映する
  - `route`：プロンプト構築 →（選択 provider の）キー取得 → `provider.send` → **成功時のみ** `save_history` を依頼する
  - キー未設定なら `ApiKeyMissing` を返し送信・履歴記録を行わない。retryable（429/5xx/529）のみ指数バックオフ＋上限で再試行し、不可エラーは即返却する
  - キー未設定で送信されない・成功時のみ履歴記録・エラー時に履歴記録しない・retryable で再試行されることを `HttpExecutor` モックで単体テストする
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 3.4, 5.1, 5.2, 5.3, 6.1, 6.2_
  - _Boundary: ModelRouter_
  - _Depends: 2.1, 3.1, 4.1_

- [ ] 5.2 Tauri コマンドを公開し lib.rs へ配線する
  - `send_message`（`schema_json`→PromptCharacter、`history_json`→Vec\<ChatMessage\>、新規 message）・`set_active_model`・`get_active_model` を公開する
  - `ModelRouter`・`KeyManager` を `manage()` し、`invoke_handler![]` に新コマンドを登録する
  - フロントから `invoke` でき、`send_message` がモック provider 経由で応答テキストを返すことを確認できる状態にする
  - _Requirements: 1.1, 4.1, 4.2, 4.4_
  - _Boundary: lib.rs, model_router コマンド_
  - _Depends: 5.1_

- [ ] 6. 統合：フロントエンド UI
- [ ] 6.1 (P) モデル選択・API キー設定 UI を実装する
  - provider／モデルのセレクターを描画し、選択で `set_active_model` を呼ぶ（状態は `get_active_model`）
  - API キー設定フォームを描画し `set_api_key` を呼ぶ。`get_api_key_status` で有無を表示し、キー未設定 provider 選択時は設定を促す
  - 画面上でモデル切替と API キー設定ができ、未設定 provider が視覚的に分かる状態にする（実機 `pnpm dev` で確認）
  - _Requirements: 1.1, 3.1, 3.4_
  - _Boundary: model-ui.ts_
  - _Depends: 5.2_

- [ ] 6.2 (P) チャット送信フローを配線する
  - 送信時に character-store の `getActive()` から `CharacterSchema`（原則値内包）を取得し、過去ターン（history）＋新規メッセージとともに `send_message` を呼ぶ
  - 応答全文をチャット UI に表示し、待機中であることを提示する。MVP は非ストリーミング表示とする
  - API エラーは UI に表示し履歴へ残さない（バックエンドが非記録）。キー未設定時は設定 UI へ誘導する
  - メッセージ送信→応答表示、エラー時の表示、キー未設定時の誘導が動作する状態にする（実機 `pnpm dev` で確認）
  - _Requirements: 4.1, 4.2, 4.4, 5.1, 3.4_
  - _Boundary: main.ts_
  - _Depends: 5.2_

- [ ] 7. 検証：統合と縮退
- [ ] 7.1 ルーティング・履歴・エラー縮退の統合テスト
  - `set_active_model`→`get_active_model` がユーザー選択を反映し、自動変更されないことを確認する
  - モック provider で `send_message` が応答を返し、成功時に履歴記録されることを確認する
  - キー未設定モデルで送信されず履歴も記録されないこと、API エラー時に履歴へ記録されないことを確認する
  - 上記が自動テストで pass する状態にする
  - _Requirements: 1.3, 3.4, 4.2, 6.1, 6.2_
  - _Depends: 5.2_

## Implementation Notes

- **依存・シーム再利用**：`reqwest`/`serde`/`keyring`/`tokio` は storage-manager で導入済み。`async-trait` のみ新規追加。HTTP は既存 `HttpExecutor`（`pub`、storage.rs:1031）とそのモックパターンを再利用してネットワーク非依存にテストする。
- **要件4.3（ストリーミング）は MVP 範囲外**：design の「ModelProvider 拡張点」に従い未実装。将来 `send_streaming`＋`model:stream-chunk` を追加する際にタスクを追補する（意図的な繰越）。
- **OpenAI/Gemini の wire 形式**：4.2/4.3 は trait 契約と認証・プロンプト配置の骨子を確定し、具体的な JSON／モデルID は実装時に各公式ドキュメントで確定する（research.md 記載・ユーザー承認済み）。
- **境界**：履歴の読み取りは呼び出し元（main.ts）の責務。model-router は履歴を読まず成功時に追記依頼のみ行う。
