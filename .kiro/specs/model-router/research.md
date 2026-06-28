# Research Log — model-router

## ディスカバリ範囲

既存コードベースへの拡張（Extension）。Rust バックエンドに `model_router.rs` と `key_manager.rs` を追加し、外部 LLM API（Anthropic / OpenAI / Google）と統合する。外部 API 契約の確認と、storage-manager で確立した trait シーム・Tauri コマンド登録パターンの踏襲が焦点。

## 外部 API 契約

### Anthropic Messages API（claude-api スキルより確定）

- エンドポイント: `POST https://api.anthropic.com/v1/messages`
- 必須ヘッダ: `x-api-key: <key>`、`anthropic-version: 2023-06-01`、`content-type: application/json`
- リクエスト: `{ model, max_tokens, system (トップレベル文字列), messages: [{role, content}], stream? }`
- レスポンス: `{ id, model, stop_reason, content: [{type:"text", text}], usage }`
- 最新モデルID: **`claude-opus-4-8`**（既定。日付サフィックスを付けない）。他に `claude-sonnet-4-6`、`claude-haiku-4-5` 等。
- `max_tokens` は必須。非ストリーミングは ~16000、ストリーミングは ~64000 を既定とする。
- ストリーミング: `stream:true` で SSE。イベント `message_start` / `content_block_start` / `content_block_delta`(text_delta) / `content_block_stop` / `message_delta` / `message_stop`。`event:`/`data:` 行をパース。
- エラー: HTTP ステータス（400/401/403/404/429/500/529）。**リトライ可=429・5xx・529**、不可=400/401/403/404。本文 `{type:"error", error:{type, message}, request_id}`。
- thinking: 4.8 は adaptive のみ（`budget_tokens` は 400）。チャット用途では `thinking` を指定しない簡素な構成で十分。

### OpenAI Chat Completions

- `POST https://api.openai.com/v1/chat/completions`、`Authorization: Bearer <key>`
- system はトップレベルではなく `messages` 配列内の `{role:"system", content}` として渡す。
- 具体的なモデルID・最新パラメータは実装時に確認（OpenAI 公式ドキュメント）。

### Google Gemini

- `POST https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`、API キーは `x-goog-api-key` ヘッダまたはクエリ。
- system は `systemInstruction`、メッセージは `contents` 配列。具体形は実装時に確認。

> 共通化観点: 3社で「system プロンプトの置き場所」「認証ヘッダ」「メッセージ表現」が異なる。共通の `ChatRequest`/`ChatResponse` を定義し、provider ごとのアダプタが各 wire 形式へマップする抽象が妥当。

## 既存パターン（踏襲）

- **HttpExecutor trait シーム**: storage-manager の `ReqwestExecutor`（`HttpExecutor` trait）を再利用し、provider クライアントをネットワーク非依存でテスト可能にする。
- **keyring 秘匿保存**: storage-manager の `KeyringTokenStore` と同型のパターンで API キーを OS キーチェーンに保存（`key_manager.rs`）。OAuth トークン（GDrive）とは別エントリ。
- **Tauri コマンド登録**: `lib.rs` の `invoke_handler![]` にコマンドを追加。`tauri::State<'_, AppStorage>` 相当で provider/key マネージャの状態を管理。
- **エラー型**: storage-manager の `StorageError`(thiserror, serde Serialize) と同様に `ModelError` を定義し、フロントへ `Result<_, ModelError>` で返す。

## 設計判断（synthesis）

- **build-vs-adopt**: Rust に公式 Anthropic SDK は無い → reqwest 生 HTTP を採用（cURL 相当の契約は claude-api スキルで確定済み）。
- **一般化**: provider 差分を `ModelProvider` trait に集約。`ChatRequest`（system+messages+model）→ provider 固有 JSON のマッピングを各実装が担う。追加 provider はクライアント1つの追加で済む。
- **簡素化**: ストリーミングは要件4.3で「利用可能な場合」=任意。MVP は非ストリーミング（全文返却）を正路とし、SSE ストリーミングは Tauri イベント `model:stream-chunk` で段階導入。
- **秘匿境界**: API キーはフロントへ平文を返さない。`get_api_key_status` は有無のみ返す。実キーは Rust 内 provider クライアントのみが参照。

## リスク

- 外部 API のレート制限・課金: ユーザー自身のキー前提。リトライは指数バックオフ＋上限回数で暴走を防ぐ。
- OpenAI/Gemini の wire 形式は実装時に公式ドキュメントで要確認（本設計は抽象と Claude 経路を確定し、他2社はアダプタ実装で具体化）。
- 原則8（aiDisclosure）の常時挿入はプロンプト構築の不変条件としてテストで担保。
