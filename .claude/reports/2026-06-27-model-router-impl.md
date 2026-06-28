# 2026-06-27 model-router 実装の振り返り（1.1〜7.1）

## 概要

model-router spec の全タスク（1.1〜7.1）をメインセッションで逐次実装。バックエンド（Rust）と
フロントエンド（TS）を統合し、Claude/GPT/Gemini の切替・プロンプト構築・APIキー秘匿管理・送信表示を実装。
**Rust 78 / frontend 59 テスト pass**、`tsc` クリーン、`vite build`／`cargo build` 成功。diary-engine と
並行（diary-engine は背景 subagent で spec 化）。

## うまくいったこと

- **provider 抽象 + enum dispatch**: `ModelProvider` trait と Claude/OpenAI/Gemini クライアントを
  `HttpClient` モックで全経路テスト。`ModelRouter` は active provider を match 分岐し、`dyn`/async-trait なしで
  3 provider を扱えた。追加 provider はクライアント1つで拡張可能。
- **汎用 generate の分離**: `ModelRouter.generate(system, messages)` を「呼び出し元が system を供給する汎用
  エントリ」として設計。`send_message`（キャラ用 system を構築）はその利用者。diary-engine も同 generate を
  再利用する前提が立った（cross-spec の整合）。
- **秘匿境界の型保証**: `get_api_key_status` は `ApiKeyStatus{provider, has_key}` のみ返し、平文キーを型レベルで
  返せない。`KeyStore` trait + モックで保存/有無を検証。
- **既存パターン踏襲**: storage-manager の `StorageError`（serde タグ）・keyring・HTTP シーム・モック文化を
  そのまま適用でき、立ち上がりが速かった。

## ハマりどころ / 設計からの実装調整

- **async-trait は不採用**: design は `#[async_trait]` 前提だったが、コードベース（storage の `HttpExecutor`）は
  `#[allow(async_fn_in_trait)]` のネイティブ async fn。整合のため async-trait を**追加せず**、enum/ジェネリック
  dispatch にした。`dyn ModelProvider` を避ける設計に変更。
- **HttpExecutor 直接再利用は不可**: `HttpExecutor::execute -> Result<_, StorageError>` でエラー型が storage に
  結合する。model-router 用に同方針の `HttpClient`（`ModelError` 返し・`H: Clone`）を新設した。
- **履歴の成功時保存はフロントへ**: design は `ModelRouter.route` で `save_history` を呼ぶ案だったが、(a)汎用
  generate を保つ、(b)日付/storage 結合を避ける、ため **chat.ts/main.ts が成功時のみ保存**する形に。要件6.1/6.2 は
  chat.test（成功時保存・失敗時非保存）＋ Rust（ApiKeyMissing 非送信）で担保。design レビューで挙げた「履歴は
  呼び出し元供給」の方針と一貫。
- **Mutex を await でまたがない**: `ModelRouter.generate` で active 選択は lock→clone→drop してから await。

## 繰越事項（follow-up）

- **要件4.3 ストリーミングは MVP 範囲外**: `ModelProvider` に拡張点コメントのみ。将来 `send_streaming`＋
  `model:stream-chunk` を追加する際にタスク追補。
- **diary-engine 連携の橋渡し**: diary-engine は `ModelRouter.generate` を使うが、現状 generate は Rust メソッド止まり。
  diary 実装時に「汎用生成 Tauri コマンド（任意 system + 履歴 → text）」を model-router に1つ追加する必要がある
  （`send_message` はキャラ用 system を構築するため流用不可）。
- **OpenAI/Gemini の実モデルID・wire 検証**: クライアントは標準形で実装。実API キーでの疎通（モデルID/レスポンス形）は
  M3 実機確認で要検証。model-ui の default モデルID（gpt-4o/gemini-1.5-pro）は編集可能な暫定値。
- **M3 実機サインオフ**: `pnpm tauri dev`＋実 API キーで「モデルと実対話」を目視確認するのが残（GUI 必須）。
- **GitHub Project**: model-router/diary-engine のタスクは Project 未登録。実装を Project で追跡するなら子Issue作成が必要。

## 体制メモ

- diary-engine の requirements は**背景 subagent**で並行生成（コンダクター＋subagent モード）。spec 化（req→design→tasks）は
  メインで実施。model-router 実装はメインで逐次。両ストリームを1セッションで並行進行できた。
- 関連 [[mitatete-dev-workflow]]・[[github-project-ids]]。
