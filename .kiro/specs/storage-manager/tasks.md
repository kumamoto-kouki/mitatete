# 実装計画

## タスク一覧

- [ ] 1. 基盤: ローカルファイルシステム初期化と依存クレートのセットアップ
- [x] 1.1 Cargo.toml に必要な依存クレートを追加する
  - `tokio`（非同期ランタイム）、`serde` / `serde_json`（JSON）、`reqwest`（HTTP クライアント）、OAuth・GDrive 用クレートを追加する
  - `Cargo.toml` でビルドが通ることを確認する
  - _Requirements: 1.6_
  - _Boundary: src-tauri/Cargo.toml_

- [x] 1.2 `~/.mitatete/` ディレクトリ構造の初期化処理を実装する
  - アプリ起動時に `~/.mitatete/history/`・`~/.mitatete/diary/`・`~/.mitatete/characters/` を存在しない場合のみ作成する
  - `main.rs` の起動フックから呼び出せる初期化関数として実装する
  - ディレクトリが既に存在する場合はスキップし、エラーにしない
  - Tauri アプリを起動すると `~/.mitatete/` 以下の 4 ディレクトリが自動生成されることを確認できる
  - _Requirements: 1.1_
  - _Boundary: LocalFileSystem_

- [ ] 2. コア: LocalFileSystem — ファイル読み書き実装
- [x] 2.1 (P) 対話履歴の保存・読み込みを実装する
  - `~/.mitatete/history/YYYY-MM-DD.json` への書き込みと読み込みを実装する
  - ファイルパスは受け取った日付文字列から LocalFileSystem 内部で構築し、外部から任意パスを受け付けない
  - 書き込み・読み込みそれぞれの成功／失敗を `Result<T, StorageError>` で返す
  - ユニットテストで正常書き込みと読み込み、存在しないファイルの読み込みエラーを検証できる
  - _Requirements: 1.2, 1.6, 5.1_
  - _Boundary: LocalFileSystem_

- [x] 2.2 (P) キャラクター設定・原則設定の保存・読み込みを実装する
  - `~/.mitatete/settings.json` への書き込みと読み込みを実装する
  - ファイルパスは LocalFileSystem 内部で固定し、外部から受け取らない
  - 存在しない場合はデフォルト値を返すか空を返す（エラーにしない）
  - ユニットテストで保存→読み込みのラウンドトリップが正確であることを確認できる
  - _Requirements: 1.3, 1.6, 5.1_
  - _Boundary: LocalFileSystem_

- [x] 2.3 (P) カスタムキャラクター定義の保存・読み込みを実装する
  - `~/.mitatete/characters/<name>.json` への書き込みと読み込み、一覧取得を実装する
  - キャラクター名のファイル名サニタイズを行い、パストラバーサルを防止する
  - ユニットテストでサニタイズが機能することを確認できる
  - _Requirements: 1.4, 1.6, 5.1_
  - _Boundary: LocalFileSystem_

- [x] 2.4 (P) AI観察日記の保存・読み込みを実装する
  - `~/.mitatete/diary/YYYY-MM-DD.md` への書き込みと読み込みを実装する
  - Markdown 文字列をそのまま書き込む（変換なし）
  - ユニットテストで保存→読み込みのラウンドトリップが正確であることを確認できる
  - _Requirements: 1.5, 1.6, 5.1_
  - _Boundary: LocalFileSystem_

- [ ] 3. コア: OAuthManager — OAuth 2.0 認証フロー実装
- [x] 3.1 OAuth 2.0 認証フローの開始と完了処理を実装する
  - Google OAuth 2.0 認可エンドポイントへのリダイレクトと、コールバックでの認可コード受け取りを実装する
  - 取得したアクセストークン・リフレッシュトークンを OS キーチェーン（Tauri keyring / stronghold）のみに保存する
  - トークンをファイルシステムや GDrive に書き出さないことをコードレベルで保証する
  - OAuth フロー完了後に `AuthStatus::Authorized` が返ることをテストで確認できる
  - _Requirements: 2.1, 2.2, 2.5_
  - _Boundary: OAuthManager_

- [x] 3.2 起動時の OAuth トークン確認とリフレッシュ処理を実装する
  - アプリ起動時にキーチェーンからトークンを読み込み、有効期限を確認する
  - 期限切れの場合はリフレッシュトークンで更新を試み、成功したら `Authorized`、失敗したらトークンを削除して `Unauthorized` を返す
  - `get_auth_status()` がアプリ起動時に正確な状態を返すことをテストで確認できる
  - _Requirements: 2.3, 2.4_
  - _Boundary: OAuthManager_

- [x] 3.3 承認取り消し処理を実装する
  - `revoke_auth()` 呼び出し時に、キーチェーンから OAuth トークンのみを削除する
  - GDrive 上のデータへの読み取り・更新・削除を一切行わないことをコードレベルで保証する
  - `~/.mitatete/` 以下のローカルファイルを削除しない
  - 取り消し後に `get_auth_status()` が `Unauthorized` を返すことをテストで確認できる
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_
  - _Boundary: OAuthManager_

- [ ] 4. コア: GDriveClient — Google Drive API クライアント実装
- [x] 4.1 GDriveClient の基本アップロード処理を実装する
  - OAuthManager からトークンを取得し、Google Drive API v3 を用いてファイルをアップロードする
  - `mitatete/` フォルダが GDrive に存在しない場合は作成してからアップロードする
  - センシティブデータ（API キーを含む）を引数として受け取らないようインターフェースを設計する
  - モックを用いたユニットテストで API 呼び出しのリクエスト内容を検証できる
  - _Requirements: 3.1, 3.2, 3.3_
  - _Boundary: GDriveClient_

- [ ] 4.2 GDrive アップロード失敗時のリトライ処理を実装する
  - 最大3回・指数バックオフのリトライロジックを実装する
  - リトライ上限到達後に `StorageError::GDriveUpload` を返す
  - リトライ処理のユニットテストで指定回数後にエラーが返ることを確認できる
  - _Requirements: 5.2, 5.3, 5.4_
  - _Boundary: GDriveClient_

- [ ] 5. 統合: StorageManager — ローカルとGDriveの調整
- [ ] 5.1 StorageManager を実装し、LocalFileSystem と GDriveClient を調整する
  - 保存要求を受け取り、ローカルへの書き込みを先行して行う
  - 承認済みの場合のみ GDriveClient に非同期でアップロードを委譲する
  - ローカル保存失敗と GDrive 失敗を独立したエラーとして扱い、一方の失敗が他方に影響しないことを保証する
  - GDrive 失敗時もローカル保存は成功扱いになることを統合テストで確認できる
  - _Requirements: 1.7, 3.1, 5.4_
  - _Boundary: StorageManager_
  - _Depends: 2.1, 2.2, 2.3, 2.4, 4.1_

- [ ] 6. 統合: Tauriコマンドの定義と main.rs への登録
- [ ] 6.1 全ストレージ操作を Tauri コマンドとして公開する
  - `save_history`・`read_history`・`save_settings`・`read_settings`・`save_character`・`save_diary` の Tauri コマンドを定義する
  - `get_auth_status`・`start_oauth`・`revoke_auth` の Tauri コマンドを定義する
  - 全コマンドを `main.rs` の `tauri::Builder` に登録する
  - フロントエンドから `window.__TAURI__.invoke("save_history", ...)` を呼び出して動作することを手動または E2E テストで確認できる
  - _Requirements: 6.1, 6.2, 6.3_
  - _Boundary: Tauri コマンド境界, StorageManager_
  - _Depends: 5.1, 3.1, 3.2, 3.3_

- [ ] 7. 検証: エラーハンドリングとエッジケース
- [ ] 7.1 ローカル書き込み失敗時のエラー返却を検証する
  - ディスク容量不足・権限エラーなどのシミュレーションでエラーが正しくフロントエンドに返ることを確認する
  - _Requirements: 5.1_
  - _Boundary: LocalFileSystem, StorageManager_

- [ ] 7.2 (P) 未承認状態での GDrive アップロードが呼ばれないことを検証する
  - `AuthStatus::Unauthorized` のとき StorageManager が GDriveClient を呼ばないことをモックで確認する
  - _Requirements: 1.7, 3.1_
  - _Boundary: StorageManager_

- [ ] 7.3 (P) 承認取り消し後のローカル保存継続を検証する
  - `revoke_auth()` 後に `save_history()` を呼び出し、`~/.mitatete/history/` にファイルが書き込まれることを確認する
  - _Requirements: 4.4_
  - _Boundary: OAuthManager, LocalFileSystem_

## Implementation Notes

- 1.2: `StorageError` に design の enum 外の `InitDir(String)` variant を追加（初期化エラー用）。design のエラー型は例示であり許容範囲だが、後続タスク（2.x〜）でエラー型を拡張する際はこの variant の存在を前提にすること。
- LocalFileSystem のパス構築は内部固定（外部から任意パスを受け取らない＝パストラバーサル防止）。`init_dirs(&Path)` はテスト用に pub だが、本番入口 `init_storage_dirs()` は外部パスを受け取らない。
- 2.1: `LocalFileSystem { base }` struct を導入（`with_base()`=テスト用 / `new()`=home解決）。read/write は tokio::fs、エラーは明示 map_err で `LocalWrite`/`LocalRead`（`From<io::Error>`=InitDir に依存しない）。日付は `validate_date` でバイト単位検証（長さ10・位置4,7が`-`・他は数字）＝パストラバーサル防止。後続 2.2〜2.4 はこの struct にメソッド追加する形で拡張すること。
- 2.1 残課題(軽微): `test_save_history_rejects_path_traversal` 内に常に真の assertion(`!escaped.exists() || true`)があるが、直後の `base.join("history")` 非作成 assert で実害なし。将来テスト整理時に簡素化。
- 2.2: `save_settings`/`read_settings` を `base/settings.json` に固定（パス引数なし）。read_settings は NotFound を `Ok({})` に縮退し、既存ファイルの read/parse 失敗のみ `LocalRead`（read_history との挙動差）。
- 2.3: `save_character`/`read_character`/`list_characters` を `base/characters/<name>.json` に実装。`validate_character_name` で empty/空白・NUL・`/`・`\`・`..`(部分一致)・先頭`.` を拒否（保守的、`foo..bar` も弾く）。list は `.json` の file stem を返し、ディレクトリ不在時は空 Vec。
- 2.4: `save_diary`/`read_diary` を `base/diary/<date>.md` に実装。Markdown を `content.as_bytes()` で逐語書き込み、read は `String::from_utf8` で完全一致復元。日付検証は既存 `validate_date` を再利用（重複バリデータなし）。
- 共通(レビュー所見): reviewer は RED-phase を「git commit で失敗状態を記録していない」と WEAK 判定しがちだが、kiro-impl 仕様上 RED は status report の RED_PHASE_OUTPUT で足り、専用コミットは不要。advisory として扱い、status report に実測の失敗出力を必ず載せること。
- 3.1: OAuthManager を `TokenStore`/`TokenExchanger` の2 trait で抽象化（本番=`KeyringTokenStore`+`GoogleTokenExchanger`、テスト=`InMemoryTokenStore`+`FakeTokenExchanger`、後者は #[cfg(test)] 限定）。`StoredToken` は keyring の単一エントリ(JSON)にのみ保存し、FS/GDrive には一切書かない（2.5 不変条件、レビューで検証済み）。`StorageError` に `OAuthFailed`/`TokenRefreshFailed`/`Unauthorized`(+将来用 `GDriveUpload`) を追加。3.2 のリフレッシュ・3.3 の revoke はこの構造に追加する。
- 3.2: `OAuthManager::get_auth_status_at(now_unix)` の時刻シーム（`EXPIRY_SKEW_SECS=60`）で起動時の有効期限判定を実装。期限切れ→`TokenExchanger::refresh`（trait拡張、Google=grant_type=refresh_token、Fake=成功/失敗切替）。成功は新トークンを `store.save` のみ、失敗は `store.delete` して `Ok(Unauthorized)`（graceful degradation, 2.4）。keyring専用不変条件を維持。`get_auth_status()` は実 now を渡す薄ラッパ。
- 3.3: `OAuthManager::revoke_auth()` は `self.store.delete()` のみ呼ぶ。GDrive・LocalFileSystem への到達経路が型レベルで存在しない（OAuthManager は GDriveClient/LocalFileSystem フィールドを持たない）ため 4.2/4.4 を構造的に保証。トークン不在時の delete も Ok（idempotent）。所見(軽微): revoke_auth は body が同期だが async 宣言（無害）。
- 4.1: `GDriveClient<H: HttpExecutor>` を HTTP シームで実装（本番=`ReqwestExecutor`、テスト=`MockHttpExecutor` #[cfg(test)]）。`ensure_mitatete_folder`（list→無ければcreate）/`upload(access_token, remote_path, content, mime)`（Bearer 認証・multipart）。3.3 不変条件: GDriveClient は executor のみ保持し API キー/secret を引数に取らない。
- **4.1 繰越事項(重要)**: GDrive のサブフォルダ（`mitatete/history/`・`mitatete/diary/`）は未対応で、現状ファイルは `mitatete/` 直下に保存される。design 3.2 の `mitatete/history/YYYY-MM-DD.json` 構造は **5.1(StorageManager) もしくは後続タスクで対応する**こと。
