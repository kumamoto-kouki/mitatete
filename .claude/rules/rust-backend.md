---
paths:
  - src-tauri/**
---

## Rust バックエンドの設計判断（storage-manager / model-router で確立）

- **外部依存は trait シームで抽象化し、`#[cfg(test)]` のモックでテストする**。keyring・HTTP・OAuth トークン交換などネットワーク/OS依存は `HttpExecutor`・`TokenStore`・`KeyStore` のような trait に切り、本番実装とモックを分ける。理由: ネットワーク・キーチェーン・実時刻に依存せず全経路をユニットテストできる（さぼり検出＝証拠で受理に直結）。
- **`async-trait` を足さない**。trait の async は `#[allow(async_fn_in_trait)]` のネイティブ async fn で書く。`dyn` が要る場面は **enum dispatch かジェネリック**で回避する（`ModelRouter` が provider を match 分岐）。理由: 依存を増やさずコードベースの既存パターン（storage.rs の `HttpExecutor`）と揃う。
- **エラー型は `thiserror` を使わず `#[derive(Debug, serde::Serialize)] #[serde(tag="kind", content="message")]` + 手動 `Display`**。理由: フロントへ `Result<_, E>` でそのまま返せ（隣接タグJSON）、依存も増えない。`StorageError`/`ModelError` が手本。
- **広域 `From` 変換に頼らず、明示 `map_err` で意図したエラー種別を付ける**。理由: `From<io::Error>` 等に依存すると read/write の取り違えが起きる（storage 1.2 の教訓）。
- **シークレットをエラー型・ステータス型・Display に載せない**。API キー/トークンは専用 store（keyring）内のみ。照会は有無のみ返す（`ApiKeyStatus{has_key}`）。`Serialize` を持つトークン型は store 外へ渡さない不変条件をコメントで明示。
- **Tauri 管理状態を await でロックしたまま跨がない**。`Mutex` は lock→clone→drop してから `.await`（`ModelRouter.generate`）。理由: `MutexGuard` は Send でなく、保持したまま await するとコンパイル不可・デッドロック温床。
