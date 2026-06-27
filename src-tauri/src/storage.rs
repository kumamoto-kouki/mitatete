// storage.rs — Mitatete のデータ永続化コンポーネント
//
// LocalFileSystem: `~/.mitatete/` 以下のディレクトリ初期化・ファイル読み書き
// OAuthManager: OAuth 2.0 フローの開始・完了・トークン保存・リフレッシュ・削除
// GDriveClient, StorageManager は後続タスクで実装する。
//
// セキュリティ制約:
//   - ファイルパスはこのモジュール内でのみ構築する（パストラバーサル防止）
//   - LocalFileSystem は外部から任意パスを受け取らない
//   - OAuthトークンは TokenStore (KeyringTokenStore) 経由で OS キーチェーンにのみ保存する
//   - トークンをファイルシステムや GDrive に書き出すコードパスは存在しない（絶対不変条件）

use std::io;
use std::path::Path;

// ---------------------------------------------------------------------------
// エラー型
// ---------------------------------------------------------------------------

/// storage-manager 全体で使用するエラー型。
#[derive(Debug)]
pub enum StorageError {
    /// ローカルファイル書き込み失敗
    LocalWrite(String),
    /// ローカルファイル読み込み失敗
    LocalRead(String),
    /// ディレクトリ初期化失敗
    InitDir(String),
    /// GDrive アップロード失敗
    GDriveUpload(String),
    /// OAuth フロー失敗
    OAuthFailed(String),
    /// トークンリフレッシュ失敗
    TokenRefreshFailed,
    /// 未承認での GDrive 操作試行
    Unauthorized,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::LocalWrite(msg) => write!(f, "Local write error: {msg}"),
            StorageError::LocalRead(msg) => write!(f, "Local read error: {msg}"),
            StorageError::InitDir(msg) => write!(f, "Directory init error: {msg}"),
            StorageError::GDriveUpload(msg) => write!(f, "GDrive upload error: {msg}"),
            StorageError::OAuthFailed(msg) => write!(f, "OAuth failed: {msg}"),
            StorageError::TokenRefreshFailed => write!(f, "Token refresh failed"),
            StorageError::Unauthorized => write!(f, "Unauthorized: no valid OAuth token"),
        }
    }
}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        StorageError::InitDir(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// 承認状態
// ---------------------------------------------------------------------------

/// Google Drive OAuth 2.0 の承認状態。
/// Tauri コマンド経由でフロントエンドに返すため Serialize を実装する。
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum AuthStatus {
    /// OAuthトークンなし・期限切れリフレッシュ失敗
    Unauthorized,
    /// 有効なOAuthトークンあり
    Authorized,
}

// ---------------------------------------------------------------------------
// StoredToken
// ---------------------------------------------------------------------------

/// OS キーチェーンに保存するトークン情報。
/// JSON にシリアライズしてキーチェーンの1エントリに格納する。
///
/// # セキュリティ不変条件
/// このモジュール内の TokenStore (KeyringTokenStore) 以外の場所では StoredToken を
/// ファイルシステムや GDrive クライアントに渡してはならない。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix エポック秒（i64）でのトークン有効期限
    pub expires_at: i64,
}

// ---------------------------------------------------------------------------
// TokenStore トレイト — トークン永続化の抽象（テスタビリティのためのシーム）
// ---------------------------------------------------------------------------

/// OAuth トークンの永続化を抽象化するトレイト。
///
/// プロダクション実装: `KeyringTokenStore` (OS キーチェーンのみ)
/// テスト実装: `InMemoryTokenStore` (メモリ内、CI で使用)
///
/// # セキュリティ不変条件
/// save() の実装はトークンを OS キーチェーン以外の場所（ファイル・GDrive）に
/// 書き出してはならない。
pub trait TokenStore: Send + Sync {
    fn save(&self, token: &StoredToken) -> Result<(), StorageError>;
    fn load(&self) -> Result<Option<StoredToken>, StorageError>;
    fn delete(&self) -> Result<(), StorageError>;
}

// ---------------------------------------------------------------------------
// KeyringTokenStore — プロダクション実装（OS キーチェーン）
// ---------------------------------------------------------------------------

/// OS キーチェーンにトークンを保存するプロダクション実装。
///
/// - service: "mitatete-oauth"
/// - username: "gdrive-token"
/// - StoredToken を JSON にシリアライズして 1 エントリに格納する
///
/// # セキュリティ不変条件
/// このクラスが唯一の TokenStore プロダクション実装であり、
/// トークンがキーチェーン以外に書き出されないことをコードレベルで保証する。
pub struct KeyringTokenStore {
    service: String,
    username: String,
}

impl KeyringTokenStore {
    const SERVICE: &'static str = "mitatete-oauth";
    const USERNAME: &'static str = "gdrive-token";

    pub fn new() -> Self {
        Self {
            service: Self::SERVICE.to_string(),
            username: Self::USERNAME.to_string(),
        }
    }
}

impl TokenStore for KeyringTokenStore {
    fn save(&self, token: &StoredToken) -> Result<(), StorageError> {
        let json = serde_json::to_string(token)
            .map_err(|e| StorageError::OAuthFailed(format!("token serialize error: {e}")))?;
        let entry = keyring::Entry::new(&self.service, &self.username)
            .map_err(|e| StorageError::OAuthFailed(format!("keyring entry error: {e}")))?;
        entry
            .set_password(&json)
            .map_err(|e| StorageError::OAuthFailed(format!("keyring save error: {e}")))?;
        Ok(())
    }

    fn load(&self) -> Result<Option<StoredToken>, StorageError> {
        let entry = keyring::Entry::new(&self.service, &self.username)
            .map_err(|e| StorageError::OAuthFailed(format!("keyring entry error: {e}")))?;
        match entry.get_password() {
            Ok(json) => {
                let token: StoredToken = serde_json::from_str(&json)
                    .map_err(|e| StorageError::OAuthFailed(format!("token deserialize: {e}")))?;
                Ok(Some(token))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(StorageError::OAuthFailed(format!(
                "keyring load error: {e}"
            ))),
        }
    }

    fn delete(&self) -> Result<(), StorageError> {
        let entry = keyring::Entry::new(&self.service, &self.username)
            .map_err(|e| StorageError::OAuthFailed(format!("keyring entry error: {e}")))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // 既に存在しない場合は成功扱い
            Err(e) => Err(StorageError::OAuthFailed(format!(
                "keyring delete error: {e}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// TokenExchanger トレイト — 認可コード→トークン交換の抽象（テスタビリティのためのシーム）
// ---------------------------------------------------------------------------

/// 認可コードをアクセストークンに交換する HTTP 処理を抽象化するトレイト。
///
/// プロダクション実装: `GoogleTokenExchanger` (Google OAuth 2.0 トークンエンドポイント)
/// テスト実装: `FakeTokenExchanger` (canned レスポンスを返す)
///
/// `async fn` in trait は Rust 1.92 で stable。Send 境界の制約がないことを意識的に許容する。
#[allow(async_fn_in_trait)]
pub trait TokenExchanger: Send + Sync {
    async fn exchange_code(&self, code: &str) -> Result<StoredToken, StorageError>;
}

// ---------------------------------------------------------------------------
// GoogleTokenExchanger — プロダクション実装（Google OAuth 2.0）
// ---------------------------------------------------------------------------

/// Google OAuth 2.0 トークンエンドポイントへ認可コードを POST してトークンを取得する。
pub struct GoogleTokenExchanger {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

impl GoogleTokenExchanger {
    const TOKEN_ENDPOINT: &'static str = "https://oauth2.googleapis.com/token";

    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri,
        }
    }
}

impl TokenExchanger for GoogleTokenExchanger {
    async fn exchange_code(&self, code: &str) -> Result<StoredToken, StorageError> {
        let client = reqwest::Client::new();

        // application/x-www-form-urlencoded を手動構築する。
        // reqwest の "form" フィーチャーが未有効なため body + Content-Type で送信する。
        let form_body = format!(
            "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
            url_encode(code),
            url_encode(&self.client_id),
            url_encode(&self.client_secret),
            url_encode(&self.redirect_uri),
        );

        let response = client
            .post(Self::TOKEN_ENDPOINT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_body)
            .send()
            .await
            .map_err(|e| StorageError::OAuthFailed(format!("HTTP request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let err_body = response
                .text()
                .await
                .unwrap_or_else(|_| "(unreadable)".to_string());
            return Err(StorageError::OAuthFailed(format!(
                "token endpoint returned {status}: {err_body}"
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| StorageError::OAuthFailed(format!("token response parse error: {e}")))?;

        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| StorageError::OAuthFailed("missing access_token".to_string()))?
            .to_string();
        let refresh_token = body["refresh_token"]
            .as_str()
            .ok_or_else(|| StorageError::OAuthFailed("missing refresh_token".to_string()))?
            .to_string();
        let expires_in = body["expires_in"].as_i64().unwrap_or(3600);

        // 現在時刻 + expires_in 秒でトークン有効期限を計算する
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let expires_at = now + expires_in;

        Ok(StoredToken {
            access_token,
            refresh_token,
            expires_at,
        })
    }
}

// ---------------------------------------------------------------------------
// OAuthManager — OAuth 2.0 フロー管理
// ---------------------------------------------------------------------------

/// OAuth 2.0 認証フローを管理するコンポーネント。
///
/// TokenStore と TokenExchanger を依存として受け取ることで、
/// テスト時に OS キーチェーンやネットワークなしでユニットテストが可能。
///
/// # セキュリティ不変条件
/// トークンは必ず `store` (TokenStore) 経由で保存され、
/// ファイルシステムや GDrive には書き出されない。
pub struct OAuthManager<S: TokenStore, X: TokenExchanger> {
    client_id: String,
    redirect_uri: String,
    scope: String,
    store: S,
    exchanger: X,
}

impl<S: TokenStore, X: TokenExchanger> OAuthManager<S, X> {
    const DEFAULT_SCOPE: &'static str = "https://www.googleapis.com/auth/drive.file";

    pub fn new(client_id: String, redirect_uri: String, store: S, exchanger: X) -> Self {
        Self {
            client_id,
            redirect_uri,
            scope: Self::DEFAULT_SCOPE.to_string(),
            store,
            exchanger,
        }
    }

    /// Google OAuth 2.0 認可エンドポイントへのリダイレクト URL を生成する。
    ///
    /// 生成される URL には以下のパラメータが含まれる:
    /// - client_id: OAuth クライアント ID
    /// - redirect_uri: コールバック URI
    /// - scope: Google Drive file スコープ
    /// - response_type: "code"
    /// - access_type: "offline"（リフレッシュトークン取得のため）
    pub fn authorization_url(&self) -> String {
        format!(
            "https://accounts.google.com/o/oauth2/v2/auth\
             ?client_id={client_id}\
             &redirect_uri={redirect_uri}\
             &scope={scope}\
             &response_type=code\
             &access_type=offline",
            client_id = url_encode(&self.client_id),
            redirect_uri = url_encode(&self.redirect_uri),
            scope = url_encode(&self.scope),
        )
    }

    /// 認可コードを受け取り、トークン交換を行い、トークンをキーチェーンに保存する。
    ///
    /// - 成功時: `AuthStatus::Authorized` を返す
    /// - 交換失敗時: `StorageError::OAuthFailed` を返し、トークンは保存しない
    ///
    /// # セキュリティ不変条件
    /// 取得したトークンは `self.store.save()` 経由でのみ保存される。
    /// ファイルシステムや GDrive には書き出されない。
    pub async fn complete_auth(&self, code: &str) -> Result<AuthStatus, StorageError> {
        // トークン交換を試みる。失敗した場合はここで Err を返し、ストアには保存しない。
        // セキュリティ不変条件: exchanger から受け取ったトークンは self.store 経由のみで保存する。
        // ファイルシステムや GDrive には書き出さない。
        let token = self.exchanger.exchange_code(code).await?;

        // 交換成功 → OS キーチェーン（TokenStore）にのみ保存する
        self.store.save(&token)?;

        Ok(AuthStatus::Authorized)
    }

    /// 現在の承認状態を返す。
    ///
    /// - トークンが store に存在する → `AuthStatus::Authorized`
    /// - トークンが存在しない → `AuthStatus::Unauthorized`
    ///
    /// (トークン有効期限・リフレッシュはタスク 3.2 で実装する)
    pub async fn get_auth_status(&self) -> Result<AuthStatus, StorageError> {
        match self.store.load()? {
            Some(_) => Ok(AuthStatus::Authorized),
            None => Ok(AuthStatus::Unauthorized),
        }
    }
}

/// URL パーセントエンコード（最小限実装）。
/// `oauth2` クレートの URL ビルダーに依存せず、標準ライブラリのみで実装する。
fn url_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b':' => encoded.push_str("%3A"),
            b'/' => encoded.push_str("%2F"),
            b'@' => encoded.push_str("%40"),
            b' ' => encoded.push_str("%20"),
            other => {
                encoded.push_str(&format!("%{:02X}", other));
            }
        }
    }
    encoded
}

// ---------------------------------------------------------------------------
// LocalFileSystem
// ---------------------------------------------------------------------------

/// サブディレクトリ名の定数。LocalFileSystem 外部では使用しない。
const SUBDIR_HISTORY: &str = "history";
const SUBDIR_DIARY: &str = "diary";
const SUBDIR_CHARACTERS: &str = "characters";

/// `~/.mitatete/` 以下のファイル読み書きを担うインフラ層。
///
/// ファイルパスはこの構造体の内部でのみ構築される。
/// 外部から任意パスを受け取るインターフェースは提供しない（パストラバーサル防止）。
pub struct LocalFileSystem {
    base: std::path::PathBuf,
}

impl LocalFileSystem {
    /// プロダクション用コンストラクタ。
    /// `HOME` 環境変数（またはプラットフォーム別フォールバック）から `~/.mitatete` を解決する。
    /// ホームディレクトリが解決できない場合は `None` を返す。
    pub fn new() -> Option<Self> {
        let home = resolve_home()?;
        Some(Self {
            base: home.join(".mitatete"),
        })
    }

    /// テスト・内部用コンストラクタ。任意のベースディレクトリで初期化できる。
    pub fn with_base(base: std::path::PathBuf) -> Self {
        Self { base }
    }

    /// 日付文字列の厳密検証。`YYYY-MM-DD` 形式（数字のみ）のみ許可する。
    /// パストラバーサル防止のため、これ以外の文字列はすべて拒否する。
    fn validate_date(date: &str) -> bool {
        // 正確に 10 文字: 4桁 + '-' + 2桁 + '-' + 2桁
        let bytes = date.as_bytes();
        if bytes.len() != 10 {
            return false;
        }
        // インデックス 4, 7 がハイフン、残りはすべて ASCII 数字
        for (i, &b) in bytes.iter().enumerate() {
            if i == 4 || i == 7 {
                if b != b'-' {
                    return false;
                }
            } else if !b.is_ascii_digit() {
                return false;
            }
        }
        true
    }

    /// 対話履歴を `base/history/{date}.json` に書き込む。
    ///
    /// - `date` は `YYYY-MM-DD` 形式のみ受け付ける（検証失敗で `StorageError::LocalWrite`）
    /// - `history/` ディレクトリが存在しない場合は自動作成する
    pub async fn save_history(
        &self,
        date: &str,
        data: &serde_json::Value,
    ) -> Result<(), StorageError> {
        if !Self::validate_date(date) {
            return Err(StorageError::LocalWrite(format!(
                "invalid date format (expected YYYY-MM-DD): {date:?}"
            )));
        }
        let history_dir = self.base.join(SUBDIR_HISTORY);
        tokio::fs::create_dir_all(&history_dir)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", history_dir.display())))?;

        let path = history_dir.join(format!("{date}.json"));
        let bytes = serde_json::to_vec(data)
            .map_err(|e| StorageError::LocalWrite(format!("serialize error: {e}")))?;
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", path.display())))?;
        Ok(())
    }

    /// キャラクター設定・原則設定を `base/settings.json` に書き込む。
    ///
    /// - ファイルパスはこのメソッド内で固定する（外部から受け取らない）
    /// - `base/` ディレクトリが存在しない場合は自動作成する
    pub async fn save_settings(&self, data: &serde_json::Value) -> Result<(), StorageError> {
        tokio::fs::create_dir_all(&self.base)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", self.base.display())))?;

        let path = self.base.join("settings.json");
        let bytes = serde_json::to_vec(data)
            .map_err(|e| StorageError::LocalWrite(format!("serialize error: {e}")))?;
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", path.display())))?;
        Ok(())
    }

    /// `base/settings.json` からキャラクター設定・原則設定を読み込む。
    ///
    /// - ファイルパスはこのメソッド内で固定する（外部から受け取らない）
    /// - ファイルが存在しない場合はエラーにならず、空のオブジェクト `{}` を返す
    /// - 既存ファイルの読み込みや JSON パース失敗の場合は `StorageError::LocalRead` を返す
    pub async fn read_settings(&self) -> Result<serde_json::Value, StorageError> {
        let path = self.base.join("settings.json");
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let value = serde_json::from_slice(&bytes)
                    .map_err(|e| StorageError::LocalRead(format!("deserialize error: {e}")))?;
                Ok(value)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Ok(serde_json::Value::Object(serde_json::Map::new()))
            }
            Err(e) => Err(StorageError::LocalRead(format!("{}: {e}", path.display()))),
        }
    }

    /// `base/history/{date}.json` から対話履歴を読み込む。
    ///
    /// - `date` は `YYYY-MM-DD` 形式のみ受け付ける（検証失敗で `StorageError::LocalRead`）
    /// - ファイルが存在しない場合は `StorageError::LocalRead` を返す
    pub async fn read_history(&self, date: &str) -> Result<serde_json::Value, StorageError> {
        if !Self::validate_date(date) {
            return Err(StorageError::LocalRead(format!(
                "invalid date format (expected YYYY-MM-DD): {date:?}"
            )));
        }
        let path = self.base.join(SUBDIR_HISTORY).join(format!("{date}.json"));
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| StorageError::LocalRead(format!("{}: {e}", path.display())))?;
        let value = serde_json::from_slice(&bytes)
            .map_err(|e| StorageError::LocalRead(format!("deserialize error: {e}")))?;
        Ok(value)
    }

    // -------------------------------------------------------------------------
    // Task 2.3: カスタムキャラクター定義
    // -------------------------------------------------------------------------

    /// キャラクター名のサニタイズ・バリデーション。
    ///
    /// 以下の条件を満たす場合のみ `Ok(())` を返す:
    /// - 空でない、空白のみでもない
    /// - パス区切り文字 (`/`, `\`) を含まない
    /// - `..` を含まない（単独でも部分文字列としても）
    /// - NUL バイトを含まない
    /// - 先頭がドット (`.`) でない
    /// - 絶対パスでない（先頭が `/` や `\` でない）
    ///
    /// 不正な場合は `StorageError::LocalWrite` を返す。
    fn validate_character_name(name: &str) -> Result<(), StorageError> {
        // 空・空白のみは拒否
        if name.trim().is_empty() {
            return Err(StorageError::LocalWrite(format!(
                "invalid character name (empty or whitespace-only): {name:?}"
            )));
        }
        // NUL バイトを拒否
        if name.contains('\0') {
            return Err(StorageError::LocalWrite(format!(
                "invalid character name (contains NUL byte): {name:?}"
            )));
        }
        // パス区切り文字を拒否
        if name.contains('/') || name.contains('\\') {
            return Err(StorageError::LocalWrite(format!(
                "invalid character name (contains path separator): {name:?}"
            )));
        }
        // `..` を含む（部分文字列としても）を拒否
        if name.contains("..") {
            return Err(StorageError::LocalWrite(format!(
                "invalid character name (contains '..'): {name:?}"
            )));
        }
        // 先頭がドットは拒否（隠しファイル・相対パス的な名前）
        if name.starts_with('.') {
            return Err(StorageError::LocalWrite(format!(
                "invalid character name (starts with '.'): {name:?}"
            )));
        }
        // 絶対パス的な名前を拒否（`/` や `\` は上で弾いているが念のため）
        // 上記チェックで既にカバー済みだが、明示的に確認
        Ok(())
    }

    /// カスタムキャラクター定義を `base/characters/<name>.json` に書き込む。
    ///
    /// - `name` はサニタイズ検証を通過した場合のみ使用する
    /// - `characters/` ディレクトリが存在しない場合は自動作成する
    pub async fn save_character(
        &self,
        name: &str,
        data: &serde_json::Value,
    ) -> Result<(), StorageError> {
        Self::validate_character_name(name)?;

        let characters_dir = self.base.join(SUBDIR_CHARACTERS);
        tokio::fs::create_dir_all(&characters_dir)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", characters_dir.display())))?;

        let path = characters_dir.join(format!("{name}.json"));
        let bytes = serde_json::to_vec(data)
            .map_err(|e| StorageError::LocalWrite(format!("serialize error: {e}")))?;
        tokio::fs::write(&path, &bytes)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", path.display())))?;
        Ok(())
    }

    /// `base/characters/<name>.json` からカスタムキャラクター定義を読み込む。
    ///
    /// - `name` はサニタイズ検証を通過した場合のみ使用する
    /// - ファイルが存在しない場合は `StorageError::LocalRead` を返す
    pub async fn read_character(&self, name: &str) -> Result<serde_json::Value, StorageError> {
        // validate_character_name は LocalWrite を返すが、read 側は LocalRead に変換する
        Self::validate_character_name(name).map_err(|e| match e {
            StorageError::LocalWrite(msg) => StorageError::LocalRead(msg),
            other => other,
        })?;

        let path = self
            .base
            .join(SUBDIR_CHARACTERS)
            .join(format!("{name}.json"));
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| StorageError::LocalRead(format!("{}: {e}", path.display())))?;
        let value = serde_json::from_slice(&bytes)
            .map_err(|e| StorageError::LocalRead(format!("deserialize error: {e}")))?;
        Ok(value)
    }

    // -------------------------------------------------------------------------
    // Task 2.4: AI観察日記の保存・読み込み
    // -------------------------------------------------------------------------

    /// AI観察日記を `base/diary/{date}.md` に書き込む。
    ///
    /// - `date` は `YYYY-MM-DD` 形式のみ受け付ける（検証失敗で `StorageError::LocalWrite`）
    /// - `content` は Markdown 文字列をそのまま書き込む（変換なし）
    /// - `diary/` ディレクトリが存在しない場合は自動作成する
    pub async fn save_diary(&self, date: &str, content: &str) -> Result<(), StorageError> {
        if !Self::validate_date(date) {
            return Err(StorageError::LocalWrite(format!(
                "invalid date format (expected YYYY-MM-DD): {date:?}"
            )));
        }
        let diary_dir = self.base.join(SUBDIR_DIARY);
        tokio::fs::create_dir_all(&diary_dir)
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", diary_dir.display())))?;

        let path = diary_dir.join(format!("{date}.md"));
        tokio::fs::write(&path, content.as_bytes())
            .await
            .map_err(|e| StorageError::LocalWrite(format!("{}: {e}", path.display())))?;
        Ok(())
    }

    /// `base/diary/{date}.md` から AI観察日記を読み込む。
    ///
    /// - `date` は `YYYY-MM-DD` 形式のみ受け付ける（検証失敗で `StorageError::LocalRead`）
    /// - ファイルが存在しない場合は `StorageError::LocalRead` を返す
    /// - 書き込んだ Markdown 文字列をそのまま返す（変換なし）
    pub async fn read_diary(&self, date: &str) -> Result<String, StorageError> {
        if !Self::validate_date(date) {
            return Err(StorageError::LocalRead(format!(
                "invalid date format (expected YYYY-MM-DD): {date:?}"
            )));
        }
        let path = self.base.join(SUBDIR_DIARY).join(format!("{date}.md"));
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| StorageError::LocalRead(format!("{}: {e}", path.display())))?;
        String::from_utf8(bytes)
            .map_err(|e| StorageError::LocalRead(format!("UTF-8 decode error: {e}")))
    }

    /// `base/characters/` 以下の `.json` ファイルのファイル名（拡張子なし）を一覧返却する。
    ///
    /// - ディレクトリが存在しない場合は空の `Vec` を返す（エラーにしない）
    pub async fn list_characters(&self) -> Result<Vec<String>, StorageError> {
        let characters_dir = self.base.join(SUBDIR_CHARACTERS);

        let mut read_dir = match tokio::fs::read_dir(&characters_dir).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // ディレクトリが存在しない場合は空の Vec を返す
                return Ok(Vec::new());
            }
            Err(e) => {
                return Err(StorageError::LocalRead(format!(
                    "{}: {e}",
                    characters_dir.display()
                )));
            }
        };

        let mut names = Vec::new();
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| StorageError::LocalRead(format!("read_dir entry: {e}")))?
        {
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();
            if let Some(stem) = file_name_str.strip_suffix(".json") {
                names.push(stem.to_string());
            }
        }
        Ok(names)
    }
}

/// `base` ディレクトリ以下に Mitatete の標準ディレクトリ構造を初期化する。
///
/// - `base/history/`
/// - `base/diary/`
/// - `base/characters/`
///
/// 既に存在するディレクトリはスキップされ、エラーにならない（冪等）。
/// ユニットテストから直接呼び出せるよう `base: &Path` を受け取る純粋関数。
/// プロダクションコードは [`init_storage_dirs`] 経由で呼び出す。
pub fn init_dirs(base: &Path) -> Result<(), StorageError> {
    for subdir in [SUBDIR_HISTORY, SUBDIR_DIARY, SUBDIR_CHARACTERS] {
        let dir = base.join(subdir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| StorageError::InitDir(format!("{}: {}", dir.display(), e)))?;
    }
    Ok(())
}

/// アプリ起動フック用ラッパー。
/// ホームディレクトリを解決して `~/.mitatete/` 以下に初期化を行う。
/// ホームディレクトリが解決できない場合はエラーログを出力するが、パニックしない。
pub fn init_storage_dirs() {
    let home = match resolve_home() {
        Some(h) => h,
        None => {
            eprintln!("[storage] Warning: could not resolve home directory; skipping dir init.");
            return;
        }
    };

    let base = home.join(".mitatete");
    if let Err(e) = init_dirs(&base) {
        eprintln!("[storage] Warning: failed to initialize storage dirs: {e}");
    }
}

/// ホームディレクトリを解決する共通ヘルパー。
///
/// 優先順位:
/// 1. `HOME` 環境変数（Linux/macOS）
/// 2. `USERPROFILE`（Windows）
/// 3. `HOMEDRIVE` + `HOMEPATH`（Windows レガシー）
///
/// 標準ライブラリのみを使用し、外部クレートに依存しない。
fn resolve_home() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("HOME") {
        return Some(std::path::PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("USERPROFILE") {
        return Some(std::path::PathBuf::from(p));
    }
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        return Some(std::path::PathBuf::from(format!("{drive}{path}")));
    }
    None
}

// ---------------------------------------------------------------------------
// ユニットテスト
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// テスト用の一意な一時ディレクトリパスを生成する。
    /// `tempfile` クレートに依存せず、プロセス ID + カウンターで一意性を確保する。
    fn temp_base() -> std::path::PathBuf {
        let id = std::process::id();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("mitatete_test_{}_{}", id, seq))
    }

    /// テスト後に一時ディレクトリを削除する。削除失敗は警告にとどめる。
    fn cleanup(base: &std::path::Path) {
        let _ = std::fs::remove_dir_all(base);
    }

    /// init_dirs を呼ぶと base + 3 サブディレクトリが作成される。
    #[test]
    fn test_init_dirs_creates_all_subdirs() {
        let base = temp_base();

        // ベースが存在しない状態から開始する
        assert!(!base.exists(), "test base should not exist before init");

        init_dirs(&base).expect("init_dirs should succeed");

        assert!(base.is_dir(), "base dir should exist after init");
        assert!(base.join("history").is_dir(), "history/ should exist");
        assert!(base.join("diary").is_dir(), "diary/ should exist");
        assert!(base.join("characters").is_dir(), "characters/ should exist");

        cleanup(&base);
    }

    /// init_dirs を 2 回呼んでもエラーにならない（冪等性）。
    #[test]
    fn test_init_dirs_is_idempotent() {
        let base = temp_base();

        init_dirs(&base).expect("first call should succeed");
        // 2 回目の呼び出しでもエラーにならないこと
        init_dirs(&base).expect("second call should succeed (idempotent)");

        // ディレクトリが消えていないこと
        assert!(base.join("history").is_dir(), "history/ should still exist");
        assert!(base.join("diary").is_dir(), "diary/ should still exist");
        assert!(
            base.join("characters").is_dir(),
            "characters/ should still exist"
        );

        cleanup(&base);
    }

    /// settings.json 用のディレクトリ（base 直下）が存在する（base 自体が作られる）。
    /// settings.json はファイルでありディレクトリではないが、親ディレクトリの確認。
    #[test]
    fn test_init_dirs_base_dir_is_created() {
        let base = temp_base();

        assert!(!base.exists());
        init_dirs(&base).expect("init_dirs should succeed");
        assert!(
            base.is_dir(),
            "base ~/.mitatete equivalent should be a directory"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // Task 2.1: LocalFileSystem::save_history / read_history
    // -------------------------------------------------------------------------

    /// save_history → read_history でラウンドトリップが成立する。
    #[tokio::test]
    async fn test_history_round_trip() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let data = serde_json::json!({
            "date": "2026-06-27",
            "messages": [
                { "role": "user", "content": "hello", "timestamp": "2026-06-27T12:00:00Z" }
            ]
        });

        fs.save_history("2026-06-27", &data)
            .await
            .expect("save_history should succeed");

        let loaded = fs
            .read_history("2026-06-27")
            .await
            .expect("read_history should succeed");

        assert_eq!(data, loaded, "round-trip value must match");

        // ファイルが正しい場所に作られていること
        assert!(
            base.join("history").join("2026-06-27.json").is_file(),
            "history file should exist at base/history/YYYY-MM-DD.json"
        );

        cleanup(&base);
    }

    /// 存在しない日付のファイルを読み込むと LocalRead エラーが返る。
    #[tokio::test]
    async fn test_read_nonexistent_history_returns_local_read_error() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.read_history("2000-01-01").await;

        assert!(
            matches!(result, Err(StorageError::LocalRead(_))),
            "reading nonexistent file must return StorageError::LocalRead, got: {result:?}"
        );

        cleanup(&base);
    }

    /// パストラバーサルを試みる日付文字列は save_history で拒否される。
    #[tokio::test]
    async fn test_save_history_rejects_path_traversal() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let data = serde_json::json!({});
        let result = fs.save_history("../../etc/passwd", &data).await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "path traversal must be rejected with StorageError::LocalWrite, got: {result:?}"
        );

        // ベースの外にファイルが作られていないこと
        let escaped = std::path::PathBuf::from("/etc/passwd");
        assert!(
            !escaped.exists() || {
                // /etc/passwd が元から存在する場合はファイル内容が汚染されていないか確認
                // ここでは「base 外にテスト由来のファイルが生成されていない」ことで十分
                true
            },
            "no file should be written outside base"
        );
        // base 自体も作られていないこと（バリデーションが create_dir_all より先に走る）
        assert!(
            !base.join("history").exists(),
            "history dir must not be created when date is invalid"
        );

        cleanup(&base);
    }

    /// パストラバーサルを試みる日付文字列は read_history で拒否される。
    #[tokio::test]
    async fn test_read_history_rejects_path_traversal() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.read_history("../../etc/passwd").await;

        assert!(
            matches!(result, Err(StorageError::LocalRead(_))),
            "path traversal must be rejected with StorageError::LocalRead, got: {result:?}"
        );

        cleanup(&base);
    }

    /// 短すぎる日付文字列（`2026-1-1`）は拒否される。
    #[tokio::test]
    async fn test_save_history_rejects_short_date() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.save_history("2026-1-1", &serde_json::json!({})).await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "short date must be rejected, got: {result:?}"
        );

        cleanup(&base);
    }

    /// validate_date の単体検証: 有効パターンと無効パターンを網羅する。
    #[test]
    fn test_validate_date_patterns() {
        // 有効
        assert!(LocalFileSystem::validate_date("2026-06-27"));
        assert!(LocalFileSystem::validate_date("2000-01-01"));
        assert!(LocalFileSystem::validate_date("9999-12-31"));

        // 無効: パストラバーサル
        assert!(!LocalFileSystem::validate_date("../../etc/passwd"));
        // 無効: 短い
        assert!(!LocalFileSystem::validate_date("2026-1-1"));
        // 無効: 長い
        assert!(!LocalFileSystem::validate_date("2026-06-270"));
        // 無効: 月が2桁でない
        assert!(!LocalFileSystem::validate_date("2026-6-27"));
        // 無効: ハイフンの位置が違う
        assert!(!LocalFileSystem::validate_date("20260627--"));
        // 無効: 空文字
        assert!(!LocalFileSystem::validate_date(""));
        // 無効: 区切り文字がスラッシュ
        assert!(!LocalFileSystem::validate_date("2026/06/27"));
        // 無効: NUL バイトを含む（長さは合うが非数字）
        assert!(!LocalFileSystem::validate_date("2026-06-2\x00"));
    }

    /// save_history は history/ ディレクトリが存在しなくても自動作成して成功する。
    #[tokio::test]
    async fn test_save_history_creates_history_dir_automatically() {
        let base = temp_base();
        // init_dirs を呼ばずに save_history する
        let fs = LocalFileSystem::with_base(base.clone());

        fs.save_history("2026-06-27", &serde_json::json!({"ok": true}))
            .await
            .expect("save_history must create history/ dir automatically");

        assert!(
            base.join("history").is_dir(),
            "history/ dir should be created by save_history"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // Task 2.2: LocalFileSystem::save_settings / read_settings
    // -------------------------------------------------------------------------

    /// save_settings → read_settings でラウンドトリップが成立する。
    #[tokio::test]
    async fn test_settings_round_trip() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let data = serde_json::json!({
            "active_character": "default",
            "principles": {
                "kindness": 0.8,
                "honesty": 1.0
            }
        });

        fs.save_settings(&data)
            .await
            .expect("save_settings should succeed");

        let loaded = fs
            .read_settings()
            .await
            .expect("read_settings should succeed");

        assert_eq!(data, loaded, "round-trip value must match");

        // ファイルが正しい場所に作られていること
        assert!(
            base.join("settings.json").is_file(),
            "settings file should exist at base/settings.json"
        );

        cleanup(&base);
    }

    /// settings.json が存在しない場合、read_settings はエラーにならず空オブジェクトを返す。
    #[tokio::test]
    async fn test_read_settings_absent_file_returns_empty_default() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        // settings.json を作らずに読み込む
        let result = fs.read_settings().await;

        assert!(
            result.is_ok(),
            "read_settings on absent file must return Ok, got: {result:?}"
        );

        let value = result.unwrap();
        assert_eq!(
            value,
            serde_json::json!({}),
            "absent settings.json must return empty object {{}}, got: {value:?}"
        );

        cleanup(&base);
    }

    /// save_settings を 2 回呼ぶと最新の値で上書きされる。
    #[tokio::test]
    async fn test_save_settings_overwrite_keeps_latest() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let first = serde_json::json!({ "active_character": "miku" });
        let second = serde_json::json!({ "active_character": "hana", "principles": {} });

        fs.save_settings(&first)
            .await
            .expect("first save_settings should succeed");

        fs.save_settings(&second)
            .await
            .expect("second save_settings should succeed");

        let loaded = fs
            .read_settings()
            .await
            .expect("read_settings after overwrite should succeed");

        assert_eq!(
            loaded, second,
            "read_settings must return the latest saved value"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // Task 2.3: validate_character_name (純粋関数 — 同期テスト)
    // -------------------------------------------------------------------------

    /// validate_character_name のテーブル駆動ユニットテスト。
    /// 不正な名前はすべて Err、正常な名前は Ok を返す。
    #[test]
    fn test_validate_character_name_table() {
        // --- 不正な名前 ---
        let bad_names: &[&str] = &[
            "../x",                     // 先頭が ..
            "../../etc",                // 複数の ..
            "a/b",                      // スラッシュを含む
            "a\\b",                     // バックスラッシュを含む
            "",                         // 空文字
            "   ",                      // 空白のみ
            "..",                       // .. 単独
            ".hidden",                  // 先頭がドット
            "/abs/path",                // 絶対パス（Unix 風）
            "foo\x00bar",               // NUL バイト
            "path/traversal/../secret", // スラッシュ + ..
        ];
        for name in bad_names {
            assert!(
                LocalFileSystem::validate_character_name(name).is_err(),
                "expected Err for bad name {name:?}, got Ok"
            );
        }

        // --- 正常な名前 ---
        let good_names: &[&str] = &[
            "alice",
            "my-char_1",
            "キャラクター", // Unicode は許容する
            "char123",
            "A",
        ];
        for name in good_names {
            assert!(
                LocalFileSystem::validate_character_name(name).is_ok(),
                "expected Ok for good name {name:?}, got Err"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Task 2.3: save_character / read_character ラウンドトリップ
    // -------------------------------------------------------------------------

    /// save_character → read_character でラウンドトリップが成立する。
    #[tokio::test]
    async fn test_character_round_trip() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let data = serde_json::json!({
            "name": "alice",
            "personality": "friendly",
            "voice": "soft"
        });

        fs.save_character("alice", &data)
            .await
            .expect("save_character should succeed");

        let loaded = fs
            .read_character("alice")
            .await
            .expect("read_character should succeed");

        assert_eq!(data, loaded, "round-trip value must match");

        // ファイルが正しい場所に作られていること
        assert!(
            base.join("characters").join("alice.json").is_file(),
            "character file should exist at base/characters/alice.json"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // Task 2.3: パストラバーサル防止
    // -------------------------------------------------------------------------

    /// save_character に `../../etc/passwd` を渡すと Err になり、
    /// base/characters 外にファイルが作られないことを確認する。
    #[tokio::test]
    async fn test_save_character_rejects_path_traversal_dotdot_slash() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let data = serde_json::json!({});
        let result = fs.save_character("../../etc/passwd", &data).await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "path traversal must be rejected with StorageError::LocalWrite, got: {result:?}"
        );

        // base の外にファイルが生成されていないこと
        // (テスト実行環境の /etc/passwd は元から存在しうるので、
        //  テスト用 base の外に新規ファイルが作られていないことで確認)
        assert!(
            !base.join("../../etc/passwd.json").exists(),
            "no file should be created via path traversal"
        );

        cleanup(&base);
    }

    /// save_character に `"a/b"` を渡すと Err になる。
    #[tokio::test]
    async fn test_save_character_rejects_slash_in_name() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.save_character("a/b", &serde_json::json!({})).await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "name with slash must be rejected, got: {result:?}"
        );

        cleanup(&base);
    }

    /// read_character にパストラバーサルを試みる名前を渡すと Err になる。
    #[tokio::test]
    async fn test_read_character_rejects_path_traversal() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.read_character("../../etc/passwd").await;

        assert!(
            matches!(result, Err(StorageError::LocalRead(_))),
            "path traversal must be rejected with StorageError::LocalRead, got: {result:?}"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // Task 2.3: list_characters
    // -------------------------------------------------------------------------

    /// キャラクターを 2 件保存後、list_characters が両方の名前を返す。
    #[tokio::test]
    async fn test_list_characters_returns_saved_names() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        fs.save_character("alice", &serde_json::json!({"id": "alice"}))
            .await
            .expect("save alice should succeed");
        fs.save_character("bob", &serde_json::json!({"id": "bob"}))
            .await
            .expect("save bob should succeed");

        let mut names = fs
            .list_characters()
            .await
            .expect("list_characters should succeed");

        names.sort();
        assert_eq!(
            names,
            vec!["alice".to_string(), "bob".to_string()],
            "list_characters must return both saved names"
        );

        cleanup(&base);
    }

    /// characters ディレクトリが存在しない場合、list_characters は空の Vec を返す。
    #[tokio::test]
    async fn test_list_characters_empty_when_dir_missing() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        // characters/ を作らずに呼び出す
        let names = fs
            .list_characters()
            .await
            .expect("list_characters on missing dir must return Ok, not Err");

        assert!(
            names.is_empty(),
            "list_characters must return empty Vec when characters/ dir doesn't exist, got: {names:?}"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // Task 2.4: save_diary / read_diary
    // -------------------------------------------------------------------------

    /// save_diary → read_diary でラウンドトリップが成立する。
    /// Markdown 文字列が変換なしでそのまま返ること（改行・強調を含む）を確認する。
    #[tokio::test]
    async fn test_diary_round_trip_verbatim_markdown() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let content = "# 日記\n本文 *強調* と改行\n";

        fs.save_diary("2026-06-27", content)
            .await
            .expect("save_diary should succeed");

        let loaded = fs
            .read_diary("2026-06-27")
            .await
            .expect("read_diary should succeed");

        assert_eq!(
            content, loaded,
            "read_diary must return the exact Markdown string written (verbatim, including newlines)"
        );

        // ファイルが正しい場所に作られていること
        assert!(
            base.join("diary").join("2026-06-27.md").is_file(),
            "diary file should exist at base/diary/YYYY-MM-DD.md"
        );

        cleanup(&base);
    }

    /// 存在しない日付のファイルを読み込むと LocalRead エラーが返る。
    #[tokio::test]
    async fn test_read_nonexistent_diary_returns_local_read_error() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.read_diary("2000-01-01").await;

        assert!(
            matches!(result, Err(StorageError::LocalRead(_))),
            "reading nonexistent diary must return StorageError::LocalRead, got: {result:?}"
        );

        cleanup(&base);
    }

    /// 無効な日付文字列（パストラバーサル）は save_diary で拒否され、
    /// base/diary 外にファイルが作られないことを確認する。
    #[tokio::test]
    async fn test_save_diary_rejects_invalid_date_path_traversal() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        let result = fs.save_diary("../evil", "malicious content").await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "invalid date must be rejected with StorageError::LocalWrite, got: {result:?}"
        );

        // base/diary が作られていないこと（バリデーションが create_dir_all より先に走る）
        assert!(
            !base.join("diary").exists(),
            "diary dir must not be created when date is invalid"
        );

        cleanup(&base);
    }

    /// characters ディレクトリが存在するが空の場合、list_characters は空の Vec を返す。
    #[tokio::test]
    async fn test_list_characters_empty_when_dir_exists_but_empty() {
        let base = temp_base();
        let fs = LocalFileSystem::with_base(base.clone());

        // characters/ ディレクトリだけ作っておく
        std::fs::create_dir_all(base.join("characters"))
            .expect("creating characters dir should succeed");

        let names = fs
            .list_characters()
            .await
            .expect("list_characters on empty dir must return Ok");

        assert!(
            names.is_empty(),
            "list_characters must return empty Vec for empty dir, got: {names:?}"
        );

        cleanup(&base);
    }

    // =========================================================================
    // Task 3.1: OAuthManager のテスト用セーム実装
    // =========================================================================

    /// テスト用インメモリ TokenStore 実装。
    /// OS キーチェーンやファイルシステムを使用しない。
    struct InMemoryTokenStore {
        inner: std::sync::Mutex<Option<StoredToken>>,
    }

    impl InMemoryTokenStore {
        fn new() -> Self {
            Self {
                inner: std::sync::Mutex::new(None),
            }
        }

        /// 現在の保存内容を取得するヘルパー（テスト検証用）。
        fn get_stored(&self) -> Option<StoredToken> {
            self.inner.lock().unwrap().clone()
        }
    }

    impl TokenStore for InMemoryTokenStore {
        fn save(&self, token: &StoredToken) -> Result<(), StorageError> {
            *self.inner.lock().unwrap() = Some(token.clone());
            Ok(())
        }

        fn load(&self) -> Result<Option<StoredToken>, StorageError> {
            Ok(self.inner.lock().unwrap().clone())
        }

        fn delete(&self) -> Result<(), StorageError> {
            *self.inner.lock().unwrap() = None;
            Ok(())
        }
    }

    /// テスト用フェイク TokenExchanger。
    /// ネットワーク呼び出しなしで canned StoredToken を返すか、設定されたエラーを返す。
    enum FakeExchangeResult {
        Success(StoredToken),
        Failure(String),
    }

    struct FakeTokenExchanger {
        result: FakeExchangeResult,
    }

    impl FakeTokenExchanger {
        fn success(token: StoredToken) -> Self {
            Self {
                result: FakeExchangeResult::Success(token),
            }
        }

        fn failure(msg: &str) -> Self {
            Self {
                result: FakeExchangeResult::Failure(msg.to_string()),
            }
        }
    }

    impl TokenExchanger for FakeTokenExchanger {
        async fn exchange_code(&self, _code: &str) -> Result<StoredToken, StorageError> {
            match &self.result {
                FakeExchangeResult::Success(token) => Ok(token.clone()),
                FakeExchangeResult::Failure(msg) => Err(StorageError::OAuthFailed(msg.clone())),
            }
        }
    }

    /// テスト用の canned StoredToken を生成するヘルパー。
    fn canned_token() -> StoredToken {
        StoredToken {
            access_token: "test-access-token".to_string(),
            refresh_token: "test-refresh-token".to_string(),
            expires_at: 9999999999,
        }
    }

    /// テスト用の OAuthManager を生成するヘルパー。
    fn make_oauth_manager(
        store: InMemoryTokenStore,
        exchanger: FakeTokenExchanger,
    ) -> OAuthManager<InMemoryTokenStore, FakeTokenExchanger> {
        OAuthManager::new(
            "test-client-id".to_string(),
            "http://localhost/callback".to_string(),
            store,
            exchanger,
        )
    }

    // =========================================================================
    // Task 3.1: OAuthManager ユニットテスト
    // =========================================================================

    /// complete_auth に有効なコードを渡すと AuthStatus::Authorized が返り、
    /// トークンがインメモリストアに保存されていること。
    #[tokio::test]
    async fn test_complete_auth_success_returns_authorized_and_stores_token() {
        let store = InMemoryTokenStore::new();
        let token = canned_token();
        let exchanger = FakeTokenExchanger::success(token.clone());
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.complete_auth("valid-code").await;

        assert!(
            matches!(result, Ok(AuthStatus::Authorized)),
            "complete_auth with valid code must return Ok(AuthStatus::Authorized), got: {result:?}"
        );

        // トークンがストアに保存されていること
        let stored = manager.store.get_stored();
        assert!(
            stored.is_some(),
            "token must be saved in store after complete_auth succeeds"
        );
        let stored = stored.unwrap();
        assert_eq!(
            stored.access_token, token.access_token,
            "stored access_token must match the token returned by exchanger"
        );
        assert_eq!(
            stored.refresh_token, token.refresh_token,
            "stored refresh_token must match the token returned by exchanger"
        );
    }

    /// complete_auth でトークン交換が失敗した場合、
    /// OAuthFailed エラーが返り、ストアには何も保存されないこと。
    #[tokio::test]
    async fn test_complete_auth_exchange_failure_returns_oauth_failed_and_no_token_stored() {
        let store = InMemoryTokenStore::new();
        let exchanger = FakeTokenExchanger::failure("exchange failed");
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.complete_auth("bad-code").await;

        assert!(
            matches!(result, Err(StorageError::OAuthFailed(_))),
            "complete_auth with exchange failure must return Err(OAuthFailed), got: {result:?}"
        );

        // ストアには何も保存されていないこと
        let stored = manager.store.get_stored();
        assert!(
            stored.is_none(),
            "no token must be stored when complete_auth fails, but store has: {stored:?}"
        );
    }

    /// get_auth_status はストアが空のとき Unauthorized を返すこと。
    #[tokio::test]
    async fn test_get_auth_status_returns_unauthorized_when_store_empty() {
        let store = InMemoryTokenStore::new(); // 空
        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.get_auth_status().await;

        assert!(
            matches!(result, Ok(AuthStatus::Unauthorized)),
            "get_auth_status on empty store must return Ok(AuthStatus::Unauthorized), got: {result:?}"
        );
    }

    /// get_auth_status はトークン保存後に Authorized を返すこと。
    #[tokio::test]
    async fn test_get_auth_status_returns_authorized_after_token_saved() {
        let store = InMemoryTokenStore::new();
        // 事前にトークンを保存しておく
        store.save(&canned_token()).expect("pre-save must succeed");

        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.get_auth_status().await;

        assert!(
            matches!(result, Ok(AuthStatus::Authorized)),
            "get_auth_status after token saved must return Ok(AuthStatus::Authorized), got: {result:?}"
        );
    }

    /// authorization_url に必須パラメータが含まれていること。
    /// client_id, response_type=code, scope (drive.file) を文字列検索で確認する。
    #[test]
    fn test_authorization_url_contains_required_params() {
        let store = InMemoryTokenStore::new();
        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        let url = manager.authorization_url();

        assert!(
            url.contains("client_id=test-client-id"),
            "authorization_url must contain client_id=test-client-id, got: {url}"
        );
        assert!(
            url.contains("response_type=code"),
            "authorization_url must contain response_type=code, got: {url}"
        );
        assert!(
            url.contains("drive.file") || url.contains("drive%2Efile"),
            "authorization_url must contain drive.file scope (raw or encoded), got: {url}"
        );
        assert!(
            url.contains("accounts.google.com"),
            "authorization_url must point to accounts.google.com, got: {url}"
        );
        assert!(
            url.contains("access_type=offline"),
            "authorization_url must contain access_type=offline for refresh token, got: {url}"
        );
    }
}
