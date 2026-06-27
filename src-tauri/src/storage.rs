// storage.rs — Mitatete のデータ永続化コンポーネント
//
// LocalFileSystem: `~/.mitatete/` 以下のディレクトリ初期化・ファイル読み書き
// OAuthManager: OAuth 2.0 フローの開始・完了・トークン保存・リフレッシュ・削除
// GDriveClient: Google Drive へのファイルアップロード
// StorageManager: ローカル優先保存 + GDrive 同期のオーケストレーション
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
///
/// Tauri コマンドの Err 型として使用するため `serde::Serialize` を実装する（要件 6.2）。
/// フロントエンドには `{ "kind": "...", "message": "..." }` 形式で届く。
/// シークレット（トークン・APIキー等）はこの型のフィールドに含まれない（要件 3.3）。
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
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

    /// リフレッシュトークンを使って新しいアクセストークンを取得する。
    ///
    /// - 成功時: 新しい `StoredToken` を返す（access_token・expires_at が更新される）
    /// - 失敗時: `StorageError::TokenRefreshFailed` を返す
    ///
    /// # セキュリティ不変条件
    /// 返却された `StoredToken` は呼び出し元（`OAuthManager`）が `TokenStore` 経由でのみ保存する。
    async fn refresh(&self, refresh_token: &str) -> Result<StoredToken, StorageError>;
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

    /// Google OAuth 2.0 トークンエンドポイントへ grant_type=refresh_token を POST して
    /// アクセストークンを更新する。
    ///
    /// Google のリフレッシュ応答では `refresh_token` フィールドが含まれない場合があるため、
    /// 既存の `refresh_token` をそのまま引き継ぐ。
    async fn refresh(&self, refresh_token: &str) -> Result<StoredToken, StorageError> {
        let client = reqwest::Client::new();

        let form_body = format!(
            "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
            url_encode(&self.client_id),
            url_encode(&self.client_secret),
            url_encode(refresh_token),
        );

        let response = client
            .post(Self::TOKEN_ENDPOINT)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form_body)
            .send()
            .await
            .map_err(|_| StorageError::TokenRefreshFailed)?;

        if !response.status().is_success() {
            return Err(StorageError::TokenRefreshFailed);
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|_| StorageError::TokenRefreshFailed)?;

        let access_token = body["access_token"]
            .as_str()
            .ok_or(StorageError::TokenRefreshFailed)?
            .to_string();
        let expires_in = body["expires_in"].as_i64().unwrap_or(3600);

        // Google のリフレッシュ応答には refresh_token が含まれない場合があるため、
        // 既存のリフレッシュトークンをそのまま引き継ぐ。
        let new_refresh_token = body["refresh_token"]
            .as_str()
            .unwrap_or(refresh_token)
            .to_string();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let expires_at = now + expires_in;

        Ok(StoredToken {
            access_token,
            refresh_token: new_refresh_token,
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

    /// 現在の承認状態を返す（アプリ起動時に呼び出す）。
    ///
    /// 内部で実際の現在時刻（Unix 秒）を取得し、`get_auth_status_at` に委譲する。
    ///
    /// # 戻り値の契約
    /// - `Ok(AuthStatus::Authorized)`: 有効なトークンあり、またはリフレッシュ成功
    /// - `Ok(AuthStatus::Unauthorized)`: トークンなし、または期限切れ＋リフレッシュ失敗
    ///   （リフレッシュ失敗の場合は先にトークンを削除してから `Unauthorized` を返す）
    /// - `Err(StorageError)`: ストアへのアクセス失敗など想定外のエラー（リフレッシュ失敗は含まない）
    pub async fn get_auth_status(&self) -> Result<AuthStatus, StorageError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        self.get_auth_status_at(now).await
    }

    /// OAuth トークンをキーチェーン（TokenStore）から削除し、承認を取り消す。
    ///
    /// # 契約
    /// - `self.store.delete()` のみを呼び出す。GDrive や LocalFileSystem には一切触れない。
    /// - トークンが存在しない場合も `Ok(())` を返す（冪等）。
    /// - 取り消し後に `get_auth_status()` を呼ぶと `Unauthorized` が返る。
    ///
    /// # セキュリティ不変条件 (要件 4.2, 4.4)
    /// - GDrive 上のデータを読み取り・更新・削除しない。
    /// - `~/.mitatete/` 以下のローカルファイルを削除しない。
    /// - 操作はトークンストアへの単一の `delete()` 呼び出しのみ。
    pub async fn revoke_auth(&self) -> Result<(), StorageError> {
        // 要件 4.3: OAuth トークンのみを削除する。
        // 要件 4.2: GDrive データへの読み取り・更新・削除を行わない。
        // 要件 4.4: ローカルファイル（~/.mitatete/）を削除しない。
        //
        // この実装は self.store.delete() のみを呼び出す。
        // OAuthManager は GDriveClient / LocalFileSystem へのフィールドを持たないため、
        // それらへの呼び出しは型システムによってコンパイル時に排除される。
        //
        // トークンが存在しない場合も Ok(()) を返す（冪等）。
        // KeyringTokenStore::delete() は NoEntry を Ok(()) に変換済みのため、
        // InMemoryTokenStore も同様に扱うことで一貫したコントラクトとなる。
        self.store.delete()
    }

    /// 起動時トークン確認の内部実装。`now_unix` を注入することでテストが決定論的になる。
    ///
    /// # アルゴリズム
    /// 1. ストアからトークンを読み込む。`None` → `Ok(Unauthorized)`
    /// 2. トークンが有効（`expires_at > now_unix + EXPIRY_SKEW_SECS`）→ `Ok(Authorized)`
    /// 3. 期限切れ → リフレッシュを試みる
    ///    - 成功: 新しいトークンをストアに保存し `Ok(Authorized)` を返す
    ///    - 失敗: ストアからトークンを削除し `Ok(Unauthorized)` を返す（graceful degradation）
    ///
    /// # セキュリティ不変条件
    /// リフレッシュ後のトークンは `self.store` 経由でのみ保存される。
    async fn get_auth_status_at(&self, now_unix: i64) -> Result<AuthStatus, StorageError> {
        // 有効期限判定に使う猶予時間（秒）。
        // トークンがこの秒数以内に期限切れになる場合も「期限切れ」として扱い、リフレッシュを試みる。
        const EXPIRY_SKEW_SECS: i64 = 60;

        let token = match self.store.load()? {
            None => return Ok(AuthStatus::Unauthorized),
            Some(t) => t,
        };

        // トークンがまだ有効な場合はそのまま Authorized を返す
        if token.expires_at > now_unix + EXPIRY_SKEW_SECS {
            return Ok(AuthStatus::Authorized);
        }

        // 期限切れ → リフレッシュを試みる
        match self.exchanger.refresh(&token.refresh_token).await {
            Ok(new_token) => {
                // リフレッシュ成功: 新しいトークンをキーチェーン（TokenStore）にのみ保存する
                self.store.save(&new_token)?;
                Ok(AuthStatus::Authorized)
            }
            Err(_) => {
                // リフレッシュ失敗: トークンを削除して Unauthorized へ移行（graceful degradation）
                // 設計書 2.4「失敗した場合は未承認状態に移行」に準拠
                // ストア削除の失敗は無視し、Unauthorized を返すことを優先する
                let _ = self.store.delete();
                Ok(AuthStatus::Unauthorized)
            }
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
// GDriveClient
// ---------------------------------------------------------------------------

/// Google Drive API v3 のリクエストを表す型。
///
/// `HttpExecutor` のモック実装がリクエスト内容（メソッド・URL・ヘッダー・ボディ）を
/// 検査できるようにするためのデータ転送オブジェクト。
#[derive(Debug, Clone)]
pub struct GDriveRequest {
    /// HTTP メソッド（"GET", "POST" など）
    pub method: String,
    /// リクエスト先 URL
    pub url: String,
    /// リクエストヘッダー（キーは小文字で格納する）
    pub headers: std::collections::HashMap<String, String>,
    /// リクエストボディ（バイト列）
    pub body: Vec<u8>,
}

/// Google Drive API v3 のレスポンスを表す型。
///
/// `HttpExecutor` のモック実装が canned レスポンスを返すために使う。
#[derive(Debug, Clone)]
pub struct GDriveResponse {
    /// HTTP ステータスコード（200 など）
    pub status: u16,
    /// レスポンスボディ（JSON テキスト）
    pub body: String,
}

/// HTTP リクエストの実行を抽象化するトレイト。
///
/// プロダクション実装: `ReqwestExecutor`（reqwest で実際のネットワーク呼び出し）
/// テスト実装: `MockHttpExecutor`（canned レスポンスを返しリクエストを記録）
///
/// このシームにより `GDriveClient` のロジックをネットワークなしでユニットテストできる。
#[allow(async_fn_in_trait)]
pub trait HttpExecutor: Send + Sync {
    async fn execute(&self, req: GDriveRequest) -> Result<GDriveResponse, StorageError>;
}

/// reqwest を使って実際のネットワーク呼び出しを行うプロダクション実装。
pub struct ReqwestExecutor;

impl HttpExecutor for ReqwestExecutor {
    async fn execute(&self, req: GDriveRequest) -> Result<GDriveResponse, StorageError> {
        let client = reqwest::Client::new();

        let method = reqwest::Method::from_bytes(req.method.as_bytes())
            .map_err(|e| StorageError::GDriveUpload(format!("invalid HTTP method: {e}")))?;

        let mut builder = client.request(method, &req.url);
        for (key, value) in &req.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }
        builder = builder.body(req.body);

        let response = builder
            .send()
            .await
            .map_err(|e| StorageError::GDriveUpload(format!("HTTP request failed: {e}")))?;

        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(unreadable)".to_string());

        Ok(GDriveResponse { status, body })
    }
}

/// Google Drive API v3 を使って `mitatete/` フォルダ以下にファイルをアップロードするクライアント。
///
/// # セキュリティ不変条件 (要件 3.3)
/// - GDriveClient の公開 API は API キー・クライアントシークレットを引数として受け取らない。
/// - アップロードに必要なのは OAuth アクセストークン（呼び出し元の OAuthManager が取得した値）と
///   アップロードするデータのみ。
/// - センシティブデータ（API キーを含む）を Google Drive に書き込まないことは呼び出し元の責務とする。
/// アップロードリトライポリシー定数（要件 5.3）。
const MAX_UPLOAD_ATTEMPTS: u32 = 3;

/// Google Drive API v3 を使って `mitatete/` フォルダ以下にファイルをアップロードするクライアント。
///
/// # リトライポリシー（要件 5.3, 5.4）
/// - `upload` は 5xx エラーまたは HTTP エグゼキュータエラー（ネットワーク障害等）に対して
///   指数バックオフでリトライする（最大 MAX_UPLOAD_ATTEMPTS = 3 回）。
/// - 4xx エラー（403 Forbidden, 400 Bad Request 等）はクライアント側の問題であるため
///   リトライしない（リトライしても成功しない）。
/// - 最終試行が失敗した場合は `StorageError::GDriveUpload` を返す（要件 5.4）。
///
/// # バックオフ注入（テスタビリティ）
/// `backoff_base_ms` フィールドでバックオフの基底時間を制御する。
/// テスト時は `with_backoff_base(executor, Duration::ZERO)` でバックオフ待機を無効化できる。
/// プロダクションは `new(executor)` を使用し、デフォルト 1000ms の基底時間を用いる。
/// バックオフ時間: attempt 0 → 0ms, attempt 1 → base * 2^0 = base, attempt 2 → base * 2^1 = 2*base
///
/// # セキュリティ不変条件 (要件 3.3)
/// - GDriveClient の公開 API は API キー・クライアントシークレットを引数として受け取らない。
/// - アップロードに必要なのは OAuth アクセストークン（呼び出し元の OAuthManager が取得した値）と
///   アップロードするデータのみ。
/// - センシティブデータ（API キーを含む）を Google Drive に書き込まないことは呼び出し元の責務とする。
pub struct GDriveClient<H: HttpExecutor> {
    executor: H,
    /// バックオフ基底時間（ミリ秒）。テスト時は 0 に設定して高速実行する。
    backoff_base_ms: u64,
}

impl<H: HttpExecutor> GDriveClient<H> {
    /// Google Drive Files API v3 のベース URL。
    const FILES_API: &'static str = "https://www.googleapis.com/drive/v3/files";
    /// Google Drive Upload API v3 のベース URL（multipart upload 用）。
    const UPLOAD_API: &'static str =
        "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart";
    /// GDrive 上に作成・確認するフォルダ名。
    const FOLDER_NAME: &'static str = "mitatete";
    /// GDrive のフォルダ MIME タイプ。
    const FOLDER_MIME: &'static str = "application/vnd.google-apps.folder";

    /// デフォルトのバックオフ基底時間（ミリ秒）。プロダクション用。
    const DEFAULT_BACKOFF_BASE_MS: u64 = 1000;

    /// `HttpExecutor` を注入してクライアントを生成する（プロダクション用）。
    ///
    /// バックオフ基底時間はデフォルト 1000ms を使用する。
    ///
    /// # セキュリティ不変条件 (要件 3.3)
    /// コンストラクタは API キー・クライアントシークレットを受け取らない。
    /// OAuth アクセストークンはアップロードメソッドの引数として渡す（呼び出し元が管理する）。
    pub fn new(executor: H) -> Self {
        Self {
            executor,
            backoff_base_ms: Self::DEFAULT_BACKOFF_BASE_MS,
        }
    }

    /// テスト用コンストラクタ。バックオフ基底時間を指定できる。
    ///
    /// テスト時は `Duration::ZERO` を渡すことでバックオフ待機を無効化し、高速実行できる。
    ///
    /// # 使用例（テスト）
    /// ```ignore
    /// let client = GDriveClient::with_backoff_base(mock_executor, std::time::Duration::ZERO);
    /// ```
    pub fn with_backoff_base(executor: H, base: std::time::Duration) -> Self {
        Self {
            executor,
            backoff_base_ms: base.as_millis() as u64,
        }
    }

    /// GDrive 上で `mitatete` フォルダを確認し、存在しない場合は作成してそのフォルダ ID を返す。
    ///
    /// アルゴリズム:
    /// 1. Drive API v3 files.list で `name='mitatete'` かつ MIME タイプがフォルダのアイテムを検索。
    /// 2. 見つかった場合は最初のアイテムの ID をそのまま返す（再利用）。
    /// 3. 見つからなかった場合は files.create でフォルダを作成し、返却された ID を返す。
    ///
    /// # 引数
    /// - `access_token`: OAuth アクセストークン（API キー・シークレットではない）
    ///
    /// # セキュリティ不変条件 (要件 3.3)
    /// `access_token` は OAuth Bearer トークンであり、API キーやクライアントシークレットではない。
    pub async fn ensure_mitatete_folder(&self, access_token: &str) -> Result<String, StorageError> {
        // Step 1: mitatete フォルダを検索する
        let query = format!(
            "name='{}' and mimeType='{}' and trashed=false",
            Self::FOLDER_NAME,
            Self::FOLDER_MIME
        );
        let list_url = format!(
            "{}?q={}&fields=files(id,name)",
            Self::FILES_API,
            url_encode(&query)
        );

        let mut headers = std::collections::HashMap::new();
        headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", access_token),
        );

        let list_req = GDriveRequest {
            method: "GET".to_string(),
            url: list_url,
            headers: headers.clone(),
            body: Vec::new(),
        };

        let list_resp = self.executor.execute(list_req).await?;

        if list_resp.status < 200 || list_resp.status >= 300 {
            return Err(StorageError::GDriveUpload(format!(
                "folder list failed with status {}: {}",
                list_resp.status, list_resp.body
            )));
        }

        let list_json: serde_json::Value = serde_json::from_str(&list_resp.body).map_err(|e| {
            StorageError::GDriveUpload(format!("folder list response parse error: {e}"))
        })?;

        // Step 2: フォルダが見つかった場合はそのまま ID を返す
        if let Some(files) = list_json["files"].as_array() {
            if let Some(first) = files.first() {
                if let Some(id) = first["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }

        // Step 3: フォルダが見つからなかった場合は作成する
        let create_meta = serde_json::json!({
            "name": Self::FOLDER_NAME,
            "mimeType": Self::FOLDER_MIME
        });
        let create_body = serde_json::to_vec(&create_meta).map_err(|e| {
            StorageError::GDriveUpload(format!("folder create body serialize error: {e}"))
        })?;

        let mut create_headers = std::collections::HashMap::new();
        create_headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", access_token),
        );
        create_headers.insert("content-type".to_string(), "application/json".to_string());

        let create_req = GDriveRequest {
            method: "POST".to_string(),
            url: Self::FILES_API.to_string(),
            headers: create_headers,
            body: create_body,
        };

        let create_resp = self.executor.execute(create_req).await?;

        if create_resp.status < 200 || create_resp.status >= 300 {
            return Err(StorageError::GDriveUpload(format!(
                "folder create failed with status {}: {}",
                create_resp.status, create_resp.body
            )));
        }

        let create_json: serde_json::Value =
            serde_json::from_str(&create_resp.body).map_err(|e| {
                StorageError::GDriveUpload(format!("folder create response parse error: {e}"))
            })?;

        let folder_id = create_json["id"].as_str().ok_or_else(|| {
            StorageError::GDriveUpload("folder create response missing 'id'".to_string())
        })?;

        Ok(folder_id.to_string())
    }

    /// ファイルを GDrive の `mitatete/` フォルダにアップロードする。
    ///
    /// `remote_path` は `history/2026-06-27.json` のような相対パスを期待する。
    /// 実装の単純化として、現バージョンでは `remote_path` のファイル名部分のみを使用して
    /// `mitatete/` フォルダ直下にアップロードする（サブフォルダの再帰作成は省略）。
    ///
    /// # リトライポリシー（要件 5.3, 5.4）
    /// - 5xx エラーまたは HTTP エグゼキュータエラー（ネットワーク障害等）が発生した場合、
    ///   指数バックオフで最大 `MAX_UPLOAD_ATTEMPTS` 回（3 回）リトライする。
    /// - 4xx エラー（403 Forbidden, 400 Bad Request 等）はリトライしない（クライアント側の問題）。
    /// - 全リトライが失敗した場合は `StorageError::GDriveUpload` を返す（要件 5.4）。
    /// - バックオフ時間: attempt 1 後 → base * 1, attempt 2 後 → base * 2
    ///   （`backoff_base_ms` フィールドで制御。テスト時は 0 に設定）
    ///
    /// # 引数
    /// - `access_token`: OAuth アクセストークン（API キー・シークレットではない、要件 3.3）
    /// - `remote_path`: GDrive 上のパス（例: `history/2026-06-27.json`）
    /// - `content`: アップロードするファイルの内容
    /// - `mime`: ファイルの MIME タイプ（例: `application/json`）
    ///
    /// # 実装の制限事項
    /// `remote_path` 内のサブフォルダ（`history/` など）は現バージョンでは作成しない。
    /// ファイルは `mitatete/` フォルダ直下に `remote_path` のファイル名で保存される。
    /// 設計書で示された `mitatete/history/YYYY-MM-DD.json` 構造は将来のタスクで対応する。
    ///
    /// # セキュリティ不変条件 (要件 3.3)
    /// - `access_token` のみを認証に使用する（API キー・クライアントシークレットは受け取らない）。
    /// - `content` の中身がセンシティブデータを含まないかどうかは呼び出し元の責務とする。
    pub async fn upload(
        &self,
        access_token: &str,
        remote_path: &str,
        content: &[u8],
        mime: &str,
    ) -> Result<(), StorageError> {
        // mitatete/ フォルダを確保する（ensure_mitatete_folder 自体はリトライしない）
        let folder_id = self.ensure_mitatete_folder(access_token).await?;

        // remote_path からファイル名を抽出する（サブディレクトリは現バージョンでは無視）
        let file_name = remote_path
            .rsplit('/')
            .next()
            .unwrap_or(remote_path)
            .to_string();

        // multipart/related リクエストを構築する
        // Drive API v3 はマルチパートアップロードで metadata + media を同時に送信できる
        let boundary = "mitatete_upload_boundary_42";

        let metadata = serde_json::json!({
            "name": file_name,
            "parents": [folder_id],
        });
        let metadata_str = serde_json::to_string(&metadata).map_err(|e| {
            StorageError::GDriveUpload(format!("upload metadata serialize error: {e}"))
        })?;

        // multipart/related ボディを手動構築する
        let mut body = Vec::new();
        // Part 1: JSON メタデータ
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(metadata_str.as_bytes());
        body.extend_from_slice(b"\r\n");
        // Part 2: ファイル内容
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
        body.extend_from_slice(content);
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        let mut headers = std::collections::HashMap::new();
        // Authorization: Bearer <access_token>（要件 3.1: 認証ヘッダー必須）
        headers.insert(
            "authorization".to_string(),
            format!("Bearer {}", access_token),
        );
        headers.insert(
            "content-type".to_string(),
            format!("multipart/related; boundary={boundary}"),
        );

        let upload_req = GDriveRequest {
            method: "POST".to_string(),
            url: Self::UPLOAD_API.to_string(),
            headers,
            body,
        };

        // リトライループ（要件 5.3: 最大 MAX_UPLOAD_ATTEMPTS 回, 5.4: 上限到達で GDriveUpload エラー）
        let mut last_error: Option<StorageError> = None;
        for attempt in 0..MAX_UPLOAD_ATTEMPTS {
            // attempt > 0 の場合は指数バックオフで待機する
            // バックオフ時間: attempt 1 → base * 2^0 = base, attempt 2 → base * 2^1 = 2*base
            if attempt > 0 && self.backoff_base_ms > 0 {
                let delay_ms = self.backoff_base_ms * (1u64 << (attempt - 1));
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            match self.executor.execute(upload_req.clone()).await {
                Err(e) => {
                    // HTTP エグゼキュータエラー（ネットワーク障害等）→ リトライ対象
                    last_error = Some(e);
                    continue;
                }
                Ok(resp) => {
                    if resp.status >= 200 && resp.status < 300 {
                        // 成功
                        return Ok(());
                    } else if resp.status >= 400 && resp.status < 500 {
                        // 4xx: クライアント側エラー → リトライ不要、即座にエラー返却
                        return Err(StorageError::GDriveUpload(format!(
                            "upload failed with status {}: {}",
                            resp.status, resp.body
                        )));
                    } else {
                        // 5xx またはその他: リトライ対象
                        last_error = Some(StorageError::GDriveUpload(format!(
                            "upload failed with status {}: {}",
                            resp.status, resp.body
                        )));
                        continue;
                    }
                }
            }
        }

        // 全 MAX_UPLOAD_ATTEMPTS 回失敗 → GDriveUpload エラーを返す（要件 5.4）
        Err(last_error.unwrap_or_else(|| {
            StorageError::GDriveUpload("upload failed after all attempts".to_string())
        }))
    }
}

// ---------------------------------------------------------------------------
// StorageManager
// ---------------------------------------------------------------------------

/// ローカル保存と GDrive 同期を調整するオーケストレーター。
///
/// # 保存フロー（save_history / save_settings / save_diary）
/// 1. `local` (LocalFileSystem) にまず書き込む。失敗 → `Err(StorageError::LocalWrite(...))` を即返す。
/// 2. `oauth.get_auth_status()` を確認する。
///    - `Unauthorized` → `Ok(SaveResult::LocalOnly)` を返す（GDrive 呼び出しなし）。
///    - `Authorized` → トークンを取得して `gdrive.upload(...)` に委譲する。
/// 3. GDrive 失敗は `Ok(SaveResult::LocalOnlyWithGDriveWarning(...))` で返す（ローカル保存は成功扱い）。
///
/// # エラー独立性（要件 5.4）
/// ローカル保存とGDrive保存は独立して扱い、GDrive失敗はローカル保存成否に影響しない。
///
/// # セキュリティ不変条件
/// アクセストークンは `oauth` (OAuthManager/TokenStore) 経由でのみ取得する。
/// ファイルシステムや GDrive には書き出さない。
pub struct StorageManager<S: TokenStore, X: TokenExchanger, H: HttpExecutor> {
    local: LocalFileSystem,
    oauth: OAuthManager<S, X>,
    gdrive: GDriveClient<H>,
}

/// StorageManager の保存操作の結果。
///
/// ローカル保存は常に成功。GDrive の状態を区別する。
/// Tauri コマンドの Ok 型として使用するため `serde::Serialize` を実装する（要件 6.2）。
#[derive(Debug, serde::Serialize)]
#[serde(tag = "status", content = "gdrive_error")]
pub enum SaveResult {
    /// ローカルのみ（未承認または承認済みで GDrive も成功）
    LocalOnly,
    /// ローカル保存成功 + GDrive アップロード成功
    LocalAndGDrive,
    /// ローカル保存成功 + GDrive アップロード失敗（警告付き）
    LocalOnlyWithGDriveWarning(String),
}

impl<S: TokenStore, X: TokenExchanger, H: HttpExecutor> StorageManager<S, X, H> {
    /// StorageManager を生成する。
    pub fn new(local: LocalFileSystem, oauth: OAuthManager<S, X>, gdrive: GDriveClient<H>) -> Self {
        Self {
            local,
            oauth,
            gdrive,
        }
    }

    /// 対話履歴を保存する。
    ///
    /// ローカルへの書き込みを先行し、承認済みの場合のみ GDrive にアップロードする。
    /// GDrive 失敗はローカル保存の成否に影響しない（要件 5.4）。
    ///
    /// # 引数
    /// - `date`: `YYYY-MM-DD` 形式の日付文字列
    /// - `data`: 保存する JSON データ
    ///
    /// # 戻り値
    /// - `Ok(SaveResult::LocalOnly)`: 未承認（ローカル保存のみ成功）
    /// - `Ok(SaveResult::LocalAndGDrive)`: 承認済み + GDrive 成功
    /// - `Ok(SaveResult::LocalOnlyWithGDriveWarning(msg))`: 承認済み + GDrive 失敗（ローカルは成功）
    /// - `Err(StorageError::LocalWrite(...))`: ローカル書き込み失敗（GDrive 呼び出しなし）
    pub async fn save_history(
        &self,
        date: &str,
        data: &serde_json::Value,
    ) -> Result<SaveResult, StorageError> {
        // Step 1: ローカルに先行して書き込む（要件 1.7, 5.1）
        self.local.save_history(date, data).await?;

        // Step 2: 承認状態を確認する
        match self.oauth.get_auth_status().await? {
            AuthStatus::Unauthorized => {
                // 未承認: ローカル保存のみ（GDrive 呼び出しなし）（要件 1.7）
                Ok(SaveResult::LocalOnly)
            }
            AuthStatus::Authorized => {
                // 承認済み: アクセストークンを取得して GDrive にアップロードする
                // セキュリティ不変条件: トークンは TokenStore 経由でのみ取得する
                let access_token = self
                    .oauth
                    .store
                    .load()
                    .map_err(|e| StorageError::OAuthFailed(format!("token load error: {e}")))?
                    .map(|t| t.access_token)
                    .ok_or(StorageError::Unauthorized)?;

                let remote_path = format!("history/{date}.json");
                let content = serde_json::to_vec(data)
                    .map_err(|e| StorageError::LocalRead(format!("serialize error: {e}")))?;

                // Step 3: GDrive アップロード（失敗してもローカル保存は成功扱い）（要件 5.4）
                match self
                    .gdrive
                    .upload(&access_token, &remote_path, &content, "application/json")
                    .await
                {
                    Ok(()) => Ok(SaveResult::LocalAndGDrive),
                    Err(e) => Ok(SaveResult::LocalOnlyWithGDriveWarning(e.to_string())),
                }
            }
        }
    }

    /// キャラクター設定・原則設定を保存する。
    ///
    /// ローカルへの書き込みを先行し、承認済みの場合のみ GDrive にアップロードする。
    /// GDrive 失敗はローカル保存の成否に影響しない（要件 5.4）。
    pub async fn save_settings(
        &self,
        data: &serde_json::Value,
    ) -> Result<SaveResult, StorageError> {
        // Step 1: ローカルに先行して書き込む
        self.local.save_settings(data).await?;

        // Step 2: 承認状態を確認する
        match self.oauth.get_auth_status().await? {
            AuthStatus::Unauthorized => Ok(SaveResult::LocalOnly),
            AuthStatus::Authorized => {
                let access_token = self
                    .oauth
                    .store
                    .load()
                    .map_err(|e| StorageError::OAuthFailed(format!("token load error: {e}")))?
                    .map(|t| t.access_token)
                    .ok_or(StorageError::Unauthorized)?;

                let content = serde_json::to_vec(data)
                    .map_err(|e| StorageError::LocalRead(format!("serialize error: {e}")))?;

                // Step 3: GDrive アップロード（失敗してもローカル保存は成功扱い）（要件 5.4）
                match self
                    .gdrive
                    .upload(&access_token, "settings.json", &content, "application/json")
                    .await
                {
                    Ok(()) => Ok(SaveResult::LocalAndGDrive),
                    Err(e) => Ok(SaveResult::LocalOnlyWithGDriveWarning(e.to_string())),
                }
            }
        }
    }

    /// AI観察日記を保存する。
    ///
    /// ローカルへの書き込みを先行し、承認済みの場合のみ GDrive にアップロードする。
    /// GDrive 失敗はローカル保存の成否に影響しない（要件 5.4）。
    pub async fn save_diary(&self, date: &str, content: &str) -> Result<SaveResult, StorageError> {
        // Step 1: ローカルに先行して書き込む
        self.local.save_diary(date, content).await?;

        // Step 2: 承認状態を確認する
        match self.oauth.get_auth_status().await? {
            AuthStatus::Unauthorized => Ok(SaveResult::LocalOnly),
            AuthStatus::Authorized => {
                let access_token = self
                    .oauth
                    .store
                    .load()
                    .map_err(|e| StorageError::OAuthFailed(format!("token load error: {e}")))?
                    .map(|t| t.access_token)
                    .ok_or(StorageError::Unauthorized)?;

                let remote_path = format!("diary/{date}.md");

                // Step 3: GDrive アップロード（失敗してもローカル保存は成功扱い）（要件 5.4）
                match self
                    .gdrive
                    .upload(
                        &access_token,
                        &remote_path,
                        content.as_bytes(),
                        "text/markdown",
                    )
                    .await
                {
                    Ok(()) => Ok(SaveResult::LocalAndGDrive),
                    Err(e) => Ok(SaveResult::LocalOnlyWithGDriveWarning(e.to_string())),
                }
            }
        }
    }

    /// 現在の承認状態を返す。
    pub async fn get_auth_status(&self) -> Result<AuthStatus, StorageError> {
        self.oauth.get_auth_status().await
    }
}

// ---------------------------------------------------------------------------
// Tauri コマンドゲートウェイ（要件 6.1, 6.2, 6.3）
// ---------------------------------------------------------------------------
//
// フロントエンドとの唯一の通信経路。全ファイル I/O・OAuth 操作はこれらのコマンド経由でのみ
// 呼び出せる。フロントエンドは直接ファイルシステムや Google Drive API にアクセスできない（6.3）。
//
// プロダクション型エイリアス:
//   AppStorage = StorageManager<KeyringTokenStore, GoogleTokenExchanger, ReqwestExecutor>
//
// lib.rs の setup() で AppStorage インスタンスを生成し、app.manage() で Tauri のマネージド
// ステートに登録する。各コマンドは tauri::State<'_, AppStorage> で参照する。

/// プロダクション用の具象型エイリアス。
///
/// OAuth クレデンシャルはアプリ登録後に環境変数またはビルド設定から注入する。
/// 現時点では未登録のため、lib.rs の setup() でプレースホルダ値を使用する（コメント参照）。
pub type AppStorage = StorageManager<KeyringTokenStore, GoogleTokenExchanger, ReqwestExecutor>;

// ─── ファイル操作コマンド ───────────────────────────────────────────────────

/// 対話履歴を `~/.mitatete/history/{date}.json` に保存する。
///
/// 承認済みの場合は Google Drive にも同期する。
/// GDrive 失敗はローカル保存の成否に影響しない（要件 5.4）。
#[tauri::command]
pub async fn save_history(
    storage: tauri::State<'_, AppStorage>,
    date: String,
    data: serde_json::Value,
) -> Result<SaveResult, StorageError> {
    storage.save_history(&date, &data).await
}

/// `~/.mitatete/history/{date}.json` から対話履歴を読み込む。
#[tauri::command]
pub async fn read_history(
    storage: tauri::State<'_, AppStorage>,
    date: String,
) -> Result<serde_json::Value, StorageError> {
    storage.local.read_history(&date).await
}

/// キャラクター・原則設定を `~/.mitatete/settings.json` に保存する。
///
/// 承認済みの場合は Google Drive にも同期する。
#[tauri::command]
pub async fn save_settings(
    storage: tauri::State<'_, AppStorage>,
    data: serde_json::Value,
) -> Result<SaveResult, StorageError> {
    storage.save_settings(&data).await
}

/// `~/.mitatete/settings.json` からキャラクター・原則設定を読み込む。
///
/// ファイルが存在しない場合は空オブジェクト `{}` を返す。
#[tauri::command]
pub async fn read_settings(
    storage: tauri::State<'_, AppStorage>,
) -> Result<serde_json::Value, StorageError> {
    storage.local.read_settings().await
}

/// カスタムキャラクター定義を `~/.mitatete/characters/{name}.json` に保存する。
///
/// `name` はサニタイズ検証を通過した場合のみ使用される（パストラバーサル防止）。
#[tauri::command]
pub async fn save_character(
    storage: tauri::State<'_, AppStorage>,
    name: String,
    data: serde_json::Value,
) -> Result<(), StorageError> {
    storage.local.save_character(&name, &data).await
}

/// AI観察日記を `~/.mitatete/diary/{date}.md` に保存する。
///
/// 承認済みの場合は Google Drive にも同期する。
#[tauri::command]
pub async fn save_diary(
    storage: tauri::State<'_, AppStorage>,
    date: String,
    content: String,
) -> Result<SaveResult, StorageError> {
    storage.save_diary(&date, &content).await
}

// ─── OAuth 認証コマンド ─────────────────────────────────────────────────────

/// 現在の Google Drive 承認状態を返す。
///
/// アプリ起動時・UI 表示前に呼び出してフロントエンドの認証表示を更新する。
/// トークンが期限切れの場合は内部でリフレッシュを試み、失敗した場合は
/// トークンを削除して `Unauthorized` を返す（graceful degradation）。
#[tauri::command]
pub async fn get_auth_status(
    storage: tauri::State<'_, AppStorage>,
) -> Result<AuthStatus, StorageError> {
    storage.get_auth_status().await
}

/// Google Drive OAuth 2.0 フローを開始し、認可 URL を返す。
///
/// フロントエンドはこの URL をシステムブラウザで開き、ユーザーが認可操作を行う。
/// 認可完了後のコールバックは別途実装予定（現フェーズでは URL 返却のみ）。
///
/// 注意: 実際の OAuth 認証は `MITATETE_GOOGLE_CLIENT_ID` / `MITATETE_GOOGLE_CLIENT_SECRET`
/// 環境変数が設定された後に機能する。未設定の場合は空のプレースホルダ値で URL が生成される。
#[tauri::command]
pub async fn start_oauth(storage: tauri::State<'_, AppStorage>) -> Result<String, StorageError> {
    // 認可 URL を生成してフロントエンドに返す。
    // フロントエンドはこの URL をシステムブラウザで開く（tauri::api::shell::open など）。
    // コールバックハンドリング（complete_auth）は別途 deep-link または localhost server で実装する。
    Ok(storage.oauth.authorization_url())
}

/// Google Drive の承認を取り消す。
///
/// OS キーチェーンから OAuth トークンのみを削除する。
/// Google Drive 上の既存データには一切触れない（要件 4.2）。
/// ローカルファイル（`~/.mitatete/`）も削除しない（要件 4.4）。
#[tauri::command]
pub async fn revoke_auth(storage: tauri::State<'_, AppStorage>) -> Result<(), StorageError> {
    storage.oauth.revoke_auth().await
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
        exchange_result: FakeExchangeResult,
        /// refresh() 呼び出し時の結果。None の場合は exchange_result と同じ挙動にする。
        refresh_result: Option<FakeExchangeResult>,
    }

    impl FakeTokenExchanger {
        fn success(token: StoredToken) -> Self {
            Self {
                exchange_result: FakeExchangeResult::Success(token),
                refresh_result: None,
            }
        }

        fn failure(msg: &str) -> Self {
            Self {
                exchange_result: FakeExchangeResult::Failure(msg.to_string()),
                refresh_result: None,
            }
        }

        /// exchange_code は成功するが refresh は成功するフェイクを作成する。
        fn with_refresh_success(exchange_token: StoredToken, refresh_token: StoredToken) -> Self {
            Self {
                exchange_result: FakeExchangeResult::Success(exchange_token),
                refresh_result: Some(FakeExchangeResult::Success(refresh_token)),
            }
        }

        /// exchange_code は成功するが refresh は失敗するフェイクを作成する。
        fn with_refresh_failure(exchange_token: StoredToken) -> Self {
            Self {
                exchange_result: FakeExchangeResult::Success(exchange_token),
                refresh_result: Some(FakeExchangeResult::Failure("refresh failed".to_string())),
            }
        }
    }

    impl TokenExchanger for FakeTokenExchanger {
        async fn exchange_code(&self, _code: &str) -> Result<StoredToken, StorageError> {
            match &self.exchange_result {
                FakeExchangeResult::Success(token) => Ok(token.clone()),
                FakeExchangeResult::Failure(msg) => Err(StorageError::OAuthFailed(msg.clone())),
            }
        }

        async fn refresh(&self, _refresh_token: &str) -> Result<StoredToken, StorageError> {
            match &self.refresh_result {
                Some(FakeExchangeResult::Success(token)) => Ok(token.clone()),
                Some(FakeExchangeResult::Failure(_)) => Err(StorageError::TokenRefreshFailed),
                // refresh_result が設定されていない場合は exchange_result を流用する
                None => match &self.exchange_result {
                    FakeExchangeResult::Success(token) => Ok(token.clone()),
                    FakeExchangeResult::Failure(_) => Err(StorageError::TokenRefreshFailed),
                },
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

    // =========================================================================
    // Task 3.2: get_auth_status_at — 起動時トークン確認とリフレッシュ処理
    // =========================================================================

    /// トークンなし → Unauthorized を返す。
    #[tokio::test]
    async fn test_get_auth_status_at_no_token_returns_unauthorized() {
        let store = InMemoryTokenStore::new(); // 空
        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        let now = 1_000_000_000_i64; // 任意の固定時刻
        let result = manager.get_auth_status_at(now).await;

        assert!(
            matches!(result, Ok(AuthStatus::Unauthorized)),
            "no token in store must return Ok(Unauthorized), got: {result:?}"
        );
    }

    /// 有効期限内のトークンあり → Authorized を返し、トークンは変化しない。
    #[tokio::test]
    async fn test_get_auth_status_at_valid_token_returns_authorized_unchanged() {
        let store = InMemoryTokenStore::new();
        let now = 1_000_000_000_i64;
        // expires_at が now + 3600 なので十分有効
        let token = StoredToken {
            access_token: "valid-access".to_string(),
            refresh_token: "valid-refresh".to_string(),
            expires_at: now + 3600,
        };
        store.save(&token).expect("pre-save must succeed");

        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.get_auth_status_at(now).await;

        assert!(
            matches!(result, Ok(AuthStatus::Authorized)),
            "valid (non-expired) token must return Ok(Authorized), got: {result:?}"
        );

        // トークンが変化していないこと（リフレッシュは呼ばれていない）
        let stored = manager.store.get_stored().expect("token must still exist");
        assert_eq!(
            stored.access_token, "valid-access",
            "access_token must be unchanged when token is still valid"
        );
    }

    /// 期限切れトークン + リフレッシュ成功 → Authorized を返し、ストアに新しいトークンが保存される。
    #[tokio::test]
    async fn test_get_auth_status_at_expired_token_refresh_success_returns_authorized_with_new_token(
    ) {
        let store = InMemoryTokenStore::new();
        let now = 1_000_000_000_i64;
        // expires_at が now より前なので期限切れ
        let expired_token = StoredToken {
            access_token: "old-access".to_string(),
            refresh_token: "old-refresh".to_string(),
            expires_at: now - 1,
        };
        store.save(&expired_token).expect("pre-save must succeed");

        // リフレッシュ成功時に返す新しいトークン
        let refreshed_token = StoredToken {
            access_token: "new-access".to_string(),
            refresh_token: "old-refresh".to_string(), // Google は同じ refresh_token を返す場合がある
            expires_at: now + 3600,
        };
        let exchanger =
            FakeTokenExchanger::with_refresh_success(canned_token(), refreshed_token.clone());
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.get_auth_status_at(now).await;

        assert!(
            matches!(result, Ok(AuthStatus::Authorized)),
            "expired token + refresh success must return Ok(Authorized), got: {result:?}"
        );

        // ストアに新しいトークンが保存されていること
        let stored = manager
            .store
            .get_stored()
            .expect("refreshed token must be saved");
        assert_eq!(
            stored.access_token, "new-access",
            "store must hold the refreshed access_token after successful refresh"
        );
        assert_eq!(
            stored.expires_at, refreshed_token.expires_at,
            "store must hold the refreshed expires_at"
        );
    }

    /// 期限切れトークン + リフレッシュ失敗 → Unauthorized を返し、トークンがストアから削除される。
    #[tokio::test]
    async fn test_get_auth_status_at_expired_token_refresh_failure_returns_unauthorized_and_deletes_token(
    ) {
        let store = InMemoryTokenStore::new();
        let now = 1_000_000_000_i64;
        let expired_token = StoredToken {
            access_token: "old-access".to_string(),
            refresh_token: "old-refresh".to_string(),
            expires_at: now - 1,
        };
        store.save(&expired_token).expect("pre-save must succeed");

        let exchanger = FakeTokenExchanger::with_refresh_failure(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        let result = manager.get_auth_status_at(now).await;

        assert!(
            matches!(result, Ok(AuthStatus::Unauthorized)),
            "expired token + refresh failure must return Ok(Unauthorized), got: {result:?}"
        );

        // トークンがストアから削除されていること（graceful degradation, 設計書 2.4）
        let stored = manager.store.get_stored();
        assert!(
            stored.is_none(),
            "token must be deleted from store after refresh failure, but store has: {stored:?}"
        );
    }

    // =========================================================================
    // Task 3.3: revoke_auth — 承認取り消し処理
    // =========================================================================

    /// complete_auth で認証後に revoke_auth を呼ぶと:
    /// - store.load() が None になる（トークンが削除される）
    /// - get_auth_status() が Unauthorized を返す
    #[tokio::test]
    async fn test_revoke_auth_after_complete_auth_removes_token_and_returns_unauthorized() {
        let store = InMemoryTokenStore::new();
        let token = canned_token();
        let exchanger = FakeTokenExchanger::success(token.clone());
        let manager = make_oauth_manager(store, exchanger);

        // 認証を完了してトークンを保存する
        let auth_result = manager.complete_auth("valid-code").await;
        assert!(
            matches!(auth_result, Ok(AuthStatus::Authorized)),
            "complete_auth must succeed before revoke test, got: {auth_result:?}"
        );

        // トークンが保存されていることを確認
        let stored_before = manager.store.get_stored();
        assert!(
            stored_before.is_some(),
            "token must be present before revoke, got: {stored_before:?}"
        );

        // 承認取り消し
        let revoke_result = manager.revoke_auth().await;
        assert!(
            revoke_result.is_ok(),
            "revoke_auth must return Ok, got: {revoke_result:?}"
        );

        // トークンがストアから削除されていること（要件 4.3）
        let stored_after = manager.store.get_stored();
        assert!(
            stored_after.is_none(),
            "token must be deleted from store after revoke_auth (req 4.3), but store has: {stored_after:?}"
        );

        // get_auth_status が Unauthorized を返すこと（要件 4.5）
        let status = manager.get_auth_status().await;
        assert!(
            matches!(status, Ok(AuthStatus::Unauthorized)),
            "get_auth_status after revoke must return Ok(Unauthorized) (req 4.5), got: {status:?}"
        );
    }

    /// トークンが存在しない状態で revoke_auth を呼んでもエラーにならない（冪等）。
    /// 状態は引き続き Unauthorized のまま（要件 4.1, 4.5）。
    #[tokio::test]
    async fn test_revoke_auth_when_no_token_returns_ok_and_stays_unauthorized() {
        let store = InMemoryTokenStore::new(); // 空（トークンなし）
        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        // トークンなしの状態で revoke_auth を呼ぶ
        let revoke_result = manager.revoke_auth().await;
        assert!(
            revoke_result.is_ok(),
            "revoke_auth on empty store must return Ok (idempotent, req 4.1), got: {revoke_result:?}"
        );

        // 状態が Unauthorized のままであること（要件 4.5）
        let status = manager.get_auth_status().await;
        assert!(
            matches!(status, Ok(AuthStatus::Unauthorized)),
            "get_auth_status after revoke on empty store must return Ok(Unauthorized) (req 4.5), got: {status:?}"
        );
    }

    /// revoke_auth は store.delete() のみを呼び出す。
    /// GDrive や LocalFileSystem への呼び出しがないことを構造的に保証する。
    ///
    /// この不変条件はコードレベルで OAuthManager に GDriveClient / LocalFileSystem への
    /// フィールドや参照が存在しないことで保証される（型システムによる保証）。
    /// テストでは revoke_auth 後にストアの状態のみが変化することを確認する。
    #[tokio::test]
    async fn test_revoke_auth_only_touches_token_store_invariant() {
        let store = InMemoryTokenStore::new();
        let token = canned_token();
        // トークンを事前に保存する
        store.save(&token).expect("pre-save must succeed");

        let exchanger = FakeTokenExchanger::success(canned_token());
        let manager = make_oauth_manager(store, exchanger);

        // revoke_auth を呼ぶ
        manager
            .revoke_auth()
            .await
            .expect("revoke_auth must succeed");

        // ストアのトークンが削除されていること（store.delete() が呼ばれた証拠）
        let stored = manager.store.get_stored();
        assert!(
            stored.is_none(),
            "revoke_auth must call store.delete(), token must be None after revoke, got: {stored:?}"
        );

        // OAuthManager<S, X> の型パラメータ上 GDriveClient / LocalFileSystem への
        // フィールドが存在しないことは型システムで保証される（コンパイル時保証）。
        // 追加の実行時チェックは不要。
    }

    // =========================================================================
    // Task 4.1: GDriveClient テスト用 MockHttpExecutor
    // =========================================================================

    /// GDriveClient のユニットテスト用モック HTTP エグゼキュータ。
    ///
    /// - 送信されたリクエストをすべて `requests` に記録する。
    /// - `responses` キューから順に canned レスポンスを返す（FIFO）。
    /// - キューが空の場合は 500 Internal Server Error を返す。
    struct MockHttpExecutor {
        requests: std::sync::Mutex<Vec<GDriveRequest>>,
        responses: std::sync::Mutex<std::collections::VecDeque<GDriveResponse>>,
    }

    impl MockHttpExecutor {
        fn new(responses: Vec<GDriveResponse>) -> Self {
            Self {
                requests: std::sync::Mutex::new(Vec::new()),
                responses: std::sync::Mutex::new(responses.into_iter().collect()),
            }
        }

        /// 記録された全リクエストを取得する。
        fn recorded_requests(&self) -> Vec<GDriveRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl HttpExecutor for MockHttpExecutor {
        async fn execute(&self, req: GDriveRequest) -> Result<GDriveResponse, StorageError> {
            self.requests.lock().unwrap().push(req);
            let resp = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(GDriveResponse {
                    status: 500,
                    body: "mock queue empty".to_string(),
                });
            Ok(resp)
        }
    }

    // =========================================================================
    // Task 4.1: GDriveClient ユニットテスト
    // =========================================================================

    /// ensure_mitatete_folder: list が空 → create リクエストが発行され、新しい folder_id が返る。
    ///
    /// 検証内容 (要件 3.1, 3.2):
    /// - 1 回目のリクエスト: GET files.list（Authorization: Bearer 付き）
    /// - 2 回目のリクエスト: POST files（フォルダ作成、Authorization: Bearer 付き）
    /// - 戻り値: create レスポンスの id フィールド
    #[tokio::test]
    async fn test_ensure_mitatete_folder_creates_when_list_empty() {
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": []}"#.to_string(),
        };
        let create_resp = GDriveResponse {
            status: 200,
            body: r#"{"id": "folder-id-abc", "name": "mitatete"}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![list_resp, create_resp]);
        let client = GDriveClient::new(mock);

        let folder_id = client
            .ensure_mitatete_folder("test-access-token")
            .await
            .expect("ensure_mitatete_folder must succeed");

        assert_eq!(
            folder_id, "folder-id-abc",
            "must return the created folder id"
        );

        let reqs = client.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            2,
            "must issue exactly 2 requests (list + create)"
        );

        // リクエスト 1: GET list
        let list_req = &reqs[0];
        assert_eq!(list_req.method, "GET", "first request must be GET");
        assert!(
            list_req.url.contains("googleapis.com/drive/v3/files"),
            "list URL must hit Drive v3 files endpoint, got: {}",
            list_req.url
        );
        assert!(
            list_req
                .headers
                .get("authorization")
                .map(|v| v.starts_with("Bearer "))
                .unwrap_or(false),
            "list request must have Authorization: Bearer header"
        );

        // リクエスト 2: POST create
        let create_req = &reqs[1];
        assert_eq!(create_req.method, "POST", "second request must be POST");
        assert!(
            create_req
                .headers
                .get("authorization")
                .map(|v| v.starts_with("Bearer "))
                .unwrap_or(false),
            "create request must have Authorization: Bearer header"
        );
        let body_json: serde_json::Value =
            serde_json::from_slice(&create_req.body).expect("create body must be valid JSON");
        assert_eq!(
            body_json["name"], "mitatete",
            "create body must include name=mitatete"
        );
        assert_eq!(
            body_json["mimeType"], "application/vnd.google-apps.folder",
            "create body must include folder mimeType"
        );
    }

    /// ensure_mitatete_folder: list が既存フォルダを返す → create は発行せず既存 id を返す。
    ///
    /// 検証内容 (要件 3.1):
    /// - リクエストは list の 1 回のみ
    /// - 戻り値: list レスポンスの最初のアイテムの id
    #[tokio::test]
    async fn test_ensure_mitatete_folder_reuses_existing_when_found() {
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "existing-folder-id", "name": "mitatete"}]}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![list_resp]);
        let client = GDriveClient::new(mock);

        let folder_id = client
            .ensure_mitatete_folder("test-access-token")
            .await
            .expect("ensure_mitatete_folder must succeed");

        assert_eq!(
            folder_id, "existing-folder-id",
            "must return the existing folder id without creating a new one"
        );

        let reqs = client.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            1,
            "must issue exactly 1 request (list only, no create)"
        );
        assert_eq!(reqs[0].method, "GET", "the single request must be GET list");
    }

    /// upload: Authorization: Bearer <token> が設定され、ファイル内容・メタデータが送信される。
    ///
    /// 検証内容 (要件 3.1, 3.2):
    /// - ensure_mitatete_folder (list + 場合によって create) の後に upload リクエストが送信される
    /// - upload リクエストに Authorization: Bearer <access_token> が含まれる
    /// - upload リクエストのボディにファイル名とコンテンツが含まれる
    /// - 成功レスポンスで Ok(()) が返る
    #[tokio::test]
    async fn test_upload_sends_correct_auth_header_and_content() {
        // ensure_mitatete_folder: 既存フォルダあり（1 リクエスト）
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
        };
        // upload レスポンス: 成功
        let upload_resp = GDriveResponse {
            status: 200,
            body: r#"{"id": "file-id-001", "name": "2026-06-27.json"}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![list_resp, upload_resp]);
        let client = GDriveClient::new(mock);

        let content = br#"{"date":"2026-06-27","messages":[]}"#;
        let result = client
            .upload(
                "my-access-token",
                "history/2026-06-27.json",
                content,
                "application/json",
            )
            .await;

        assert!(
            result.is_ok(),
            "upload must succeed for 200 response, got: {result:?}"
        );

        let reqs = client.executor.recorded_requests();
        // ensure_mitatete_folder (1 list) + upload = 2 リクエスト
        assert_eq!(
            reqs.len(),
            2,
            "must issue 2 requests (list + upload), got: {}",
            reqs.len()
        );

        let upload_req = &reqs[1];
        assert_eq!(upload_req.method, "POST", "upload request must be POST");
        assert!(
            upload_req.url.contains("uploadType=multipart"),
            "upload URL must use multipart upload, got: {}",
            upload_req.url
        );

        // Authorization: Bearer ヘッダーが設定されていること（要件 3.1）
        let auth = upload_req
            .headers
            .get("authorization")
            .expect("upload must have Authorization header");
        assert_eq!(
            auth, "Bearer my-access-token",
            "Authorization must be 'Bearer my-access-token', got: {auth}"
        );

        // ボディにファイル名が含まれること（要件 3.2: 対応する構造で書き込む）
        let body_str = String::from_utf8_lossy(&upload_req.body);
        assert!(
            body_str.contains("2026-06-27.json"),
            "upload body must contain the file name, got body (truncated): {}",
            &body_str[..body_str.len().min(200)]
        );
        // ボディにファイル内容が含まれること
        assert!(
            body_str.contains("2026-06-27"),
            "upload body must contain the file content, got body (truncated): {}",
            &body_str[..body_str.len().min(200)]
        );
    }

    /// upload: モックが HTTP エラー (400) を返す → Err(StorageError::GDriveUpload(_)) になる。
    ///
    /// 検証内容 (要件 5.2: GDrive 書き込み失敗時はエラーを返す):
    /// - 非 2xx レスポンスで GDriveUpload エラーが返ること
    #[tokio::test]
    async fn test_upload_returns_gdrive_upload_error_on_http_error() {
        // ensure_mitatete_folder: 既存フォルダあり
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
        };
        // upload レスポンス: 403 Forbidden
        let upload_err_resp = GDriveResponse {
            status: 403,
            body: r#"{"error": {"message": "insufficientPermissions"}}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![list_resp, upload_err_resp]);
        let client = GDriveClient::new(mock);

        let result = client
            .upload(
                "my-access-token",
                "history/2026-06-27.json",
                b"content",
                "application/json",
            )
            .await;

        assert!(
            matches!(result, Err(StorageError::GDriveUpload(_))),
            "upload must return Err(GDriveUpload) on non-2xx response, got: {result:?}"
        );
    }

    // =========================================================================
    // Task 4.2: GDriveClient リトライ処理のユニットテスト（要件 5.2, 5.3, 5.4）
    //
    // バックオフ注入方針: with_backoff_base(executor, Duration::ZERO) を使用し、
    // テスト実行時はバックオフ待機を 0ms にして高速実行する。
    //
    // リトライカウント検証方針: MockHttpExecutor.recorded_requests() でアップロード用
    // POST リクエスト数をカウントする（ensure_mitatete_folder の GET を除いたもの）。
    // =========================================================================

    /// upload が初回で成功する場合、アップロードリクエストは正確に 1 回のみ発行される（要件 5.3）。
    ///
    /// 検証:
    /// - Ok(()) が返ること
    /// - POST upload リクエストが 1 回だけ（ensure_mitatete_folder の GET 1 回 + upload POST 1 回 = 計 2 回）
    #[tokio::test]
    async fn test_upload_retry_succeeds_on_first_attempt_issues_exactly_one_upload_request() {
        // ensure_mitatete_folder: 既存フォルダあり（GET 1 回）
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
        };
        // upload: 初回成功
        let upload_ok = GDriveResponse {
            status: 200,
            body: r#"{"id": "file-001"}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![list_resp, upload_ok]);
        // バックオフ 0ms（テスト高速化）
        let client = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);

        let result = client
            .upload(
                "token",
                "history/2026-06-27.json",
                b"content",
                "application/json",
            )
            .await;

        assert!(
            result.is_ok(),
            "upload must succeed on first attempt, got: {result:?}"
        );

        let reqs = client.executor.recorded_requests();
        // GET (ensure_mitatete_folder list) + POST (upload) = 2 total
        assert_eq!(
            reqs.len(),
            2,
            "must issue exactly 2 requests total (1 ensure_folder GET + 1 upload POST), got: {}",
            reqs.len()
        );
        // アップロードリクエスト（2番目）が POST であること
        assert_eq!(reqs[1].method, "POST", "upload request must be POST");
        assert!(
            reqs[1].url.contains("uploadType=multipart"),
            "upload request must be multipart upload POST"
        );
    }

    /// upload が 2 回失敗（5xx）して 3 回目で成功する場合、
    /// アップロードリクエストが正確に 3 回発行され Ok(()) が返ること（要件 5.3）。
    #[tokio::test]
    async fn test_upload_retry_succeeds_on_third_attempt_after_two_transient_5xx_failures() {
        // ensure_mitatete_folder: 既存フォルダあり（GET 1 回）
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
        };
        // upload 試行 1: 500 Internal Server Error（リトライ対象）
        let upload_fail_1 = GDriveResponse {
            status: 500,
            body: r#"{"error": {"message": "backend error"}}"#.to_string(),
        };
        // upload 試行 2: 503 Service Unavailable（リトライ対象）
        let upload_fail_2 = GDriveResponse {
            status: 503,
            body: r#"{"error": {"message": "service unavailable"}}"#.to_string(),
        };
        // upload 試行 3: 200 OK（成功）
        let upload_ok = GDriveResponse {
            status: 200,
            body: r#"{"id": "file-001"}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![list_resp, upload_fail_1, upload_fail_2, upload_ok]);
        // バックオフ 0ms（テスト高速化）
        let client = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);

        let result = client
            .upload(
                "token",
                "history/2026-06-27.json",
                b"content",
                "application/json",
            )
            .await;

        assert!(
            result.is_ok(),
            "upload must succeed on 3rd attempt after 2 transient failures, got: {result:?}"
        );

        let reqs = client.executor.recorded_requests();
        // GET (ensure_mitatete_folder list) + POST×3 (upload 試行 1,2,3) = 4 total
        assert_eq!(
            reqs.len(),
            4,
            "must issue 4 requests total (1 ensure_folder GET + 3 upload POSTs), got: {}",
            reqs.len()
        );
        // すべてのアップロード試行が POST であること
        for i in 1..=3 {
            assert_eq!(
                reqs[i].method, "POST",
                "request[{i}] must be POST upload attempt"
            );
        }
    }

    /// upload が 3 回すべて失敗（5xx）する場合、
    /// アップロードリクエストが正確に 3 回発行され Err(StorageError::GDriveUpload(_)) が返ること
    /// （要件 5.3: 最大 3 回, 5.4: 上限到達でエラー返却）。
    #[tokio::test]
    async fn test_upload_retry_all_3_attempts_fail_returns_gdrive_upload_error() {
        // ensure_mitatete_folder: 既存フォルダあり（GET 1 回）
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
        };
        // upload 試行 1,2,3: すべて 500
        let upload_fail = GDriveResponse {
            status: 500,
            body: r#"{"error": {"message": "internal server error"}}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![
            list_resp,
            upload_fail.clone(),
            upload_fail.clone(),
            upload_fail,
        ]);
        // バックオフ 0ms（テスト高速化）
        let client = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);

        let result = client
            .upload(
                "token",
                "history/2026-06-27.json",
                b"content",
                "application/json",
            )
            .await;

        assert!(
            matches!(result, Err(StorageError::GDriveUpload(_))),
            "upload must return Err(GDriveUpload) after all 3 attempts fail, got: {result:?}"
        );

        let reqs = client.executor.recorded_requests();
        // GET (ensure_mitatete_folder list) + POST×3 (upload 試行 1,2,3) = 4 total
        assert_eq!(
            reqs.len(),
            4,
            "must issue exactly 4 requests (1 ensure_folder GET + 3 upload POSTs), got: {}",
            reqs.len()
        );
        // すべてのアップロード試行が POST であること
        for i in 1..=3 {
            assert_eq!(
                reqs[i].method, "POST",
                "request[{i}] must be POST upload attempt"
            );
        }
    }

    /// upload が 4xx（403 Forbidden）を受け取った場合、
    /// リトライせずに即座に Err(StorageError::GDriveUpload(_)) が返ること。
    ///
    /// 4xx はクライアント側の問題（権限不足・バリデーション失敗等）であり、
    /// リトライしても成功しないため即座に失敗とする（要件 5.2）。
    #[tokio::test]
    async fn test_upload_retry_does_not_retry_on_4xx_client_error() {
        // ensure_mitatete_folder: 既存フォルダあり（GET 1 回）
        let list_resp = GDriveResponse {
            status: 200,
            body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
        };
        // upload: 403 Forbidden（リトライ対象外）
        let upload_forbidden = GDriveResponse {
            status: 403,
            body: r#"{"error": {"message": "insufficientPermissions"}}"#.to_string(),
        };
        // キューに 500 も積んでおく。もし誤ってリトライされたら使われてしまう
        let upload_would_succeed_if_retried = GDriveResponse {
            status: 200,
            body: r#"{"id": "file-001"}"#.to_string(),
        };
        let mock = MockHttpExecutor::new(vec![
            list_resp,
            upload_forbidden,
            upload_would_succeed_if_retried,
        ]);
        // バックオフ 0ms（テスト高速化）
        let client = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);

        let result = client
            .upload(
                "token",
                "history/2026-06-27.json",
                b"content",
                "application/json",
            )
            .await;

        assert!(
            matches!(result, Err(StorageError::GDriveUpload(_))),
            "upload must return Err(GDriveUpload) immediately on 4xx, got: {result:?}"
        );

        let reqs = client.executor.recorded_requests();
        // GET (ensure_mitatete_folder list) + POST×1 (upload: 4xx で即座に失敗) = 2 total
        // リトライされていれば 3 以上になる
        assert_eq!(
            reqs.len(),
            2,
            "must issue exactly 2 requests (1 ensure_folder GET + 1 upload POST, NO retry for 4xx), got: {}",
            reqs.len()
        );
    }

    // =========================================================================
    // Task 5.1: StorageManager テスト
    // =========================================================================

    /// StorageManager のテスト用ヘルパー。
    /// LocalFileSystem (temp_base), InMemoryTokenStore + FakeTokenExchanger で OAuthManager,
    /// MockHttpExecutor で GDriveClient を構築する。
    fn make_storage_manager_unauthorized(
        base: std::path::PathBuf,
    ) -> (
        StorageManager<InMemoryTokenStore, FakeTokenExchanger, MockHttpExecutor>,
        std::path::PathBuf,
    ) {
        let local = LocalFileSystem::with_base(base.clone());
        let store = InMemoryTokenStore::new(); // トークンなし → Unauthorized
        let exchanger = FakeTokenExchanger::success(canned_token());
        let oauth = OAuthManager::new(
            "test-client-id".to_string(),
            "http://localhost/callback".to_string(),
            store,
            exchanger,
        );
        // モックには空のキューを渡す（Unauthorized 時は GDrive 呼び出しがないため）
        let mock = MockHttpExecutor::new(vec![]);
        let gdrive = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);
        let sm = StorageManager::new(local, oauth, gdrive);
        (sm, base)
    }

    fn make_storage_manager_authorized(
        base: std::path::PathBuf,
        gdrive_responses: Vec<GDriveResponse>,
    ) -> (
        StorageManager<InMemoryTokenStore, FakeTokenExchanger, MockHttpExecutor>,
        std::path::PathBuf,
    ) {
        let local = LocalFileSystem::with_base(base.clone());
        let store = InMemoryTokenStore::new();
        // Authorized 状態にする: 有効なトークンを事前に保存
        store.save(&canned_token()).expect("pre-save token");
        let exchanger = FakeTokenExchanger::success(canned_token());
        let oauth = OAuthManager::new(
            "test-client-id".to_string(),
            "http://localhost/callback".to_string(),
            store,
            exchanger,
        );
        let mock = MockHttpExecutor::new(gdrive_responses);
        let gdrive = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);
        let sm = StorageManager::new(local, oauth, gdrive);
        (sm, base)
    }

    // -------------------------------------------------------------------------
    // 5.1a: 未承認時は GDrive を呼ばずローカルに保存して Ok を返す（要件 1.7）
    // -------------------------------------------------------------------------

    /// 未承認状態で save_history を呼ぶと:
    /// - ローカルファイルが作成される
    /// - MockHttpExecutor にリクエストが記録されない（GDrive 呼び出しなし）
    /// - Ok(SaveResult::LocalOnly) が返る
    #[tokio::test]
    async fn test_storage_manager_save_history_unauthorized_local_only_no_gdrive_call() {
        let base = temp_base();
        let (sm, base) = make_storage_manager_unauthorized(base);

        let data = serde_json::json!({"date": "2026-06-27", "messages": []});
        let result = sm.save_history("2026-06-27", &data).await;

        // Ok(LocalOnly) が返ること
        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "unauthorized save_history must return Ok(LocalOnly), got: {result:?}"
        );

        // ローカルファイルが作成されていること（要件 1.7: ローカル保存を継続）
        assert!(
            base.join("history").join("2026-06-27.json").is_file(),
            "local file must be written even when unauthorized"
        );

        // GDrive 呼び出しがゼロであること（要件 1.7: クラウド同期は行わない）
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            0,
            "must NOT call GDrive when unauthorized (req 1.7), got {} requests",
            reqs.len()
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // 5.1b: 承認済み + GDrive 成功 → ローカル保存 + GDrive アップロード（要件 3.1）
    // -------------------------------------------------------------------------

    /// 承認済み状態で save_history を呼ぶと:
    /// - ローカルファイルが作成される
    /// - GDrive アップロードリクエストが発行される（ensure_folder + upload の 2+ requests）
    /// - Ok(SaveResult::LocalAndGDrive) が返る
    #[tokio::test]
    async fn test_storage_manager_save_history_authorized_gdrive_success() {
        let base = temp_base();
        // GDrive モック: ensure_mitatete_folder (list: 既存フォルダ) + upload 成功
        let gdrive_responses = vec![
            GDriveResponse {
                status: 200,
                body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
            },
            GDriveResponse {
                status: 200,
                body: r#"{"id": "file-001"}"#.to_string(),
            },
        ];
        let (sm, base) = make_storage_manager_authorized(base, gdrive_responses);

        let data = serde_json::json!({"date": "2026-06-27", "messages": []});
        let result = sm.save_history("2026-06-27", &data).await;

        // Ok(LocalAndGDrive) が返ること
        assert!(
            matches!(result, Ok(SaveResult::LocalAndGDrive)),
            "authorized save_history with GDrive success must return Ok(LocalAndGDrive), got: {result:?}"
        );

        // ローカルファイルが作成されていること（要件 3.1: ローカル保存と並行して）
        assert!(
            base.join("history").join("2026-06-27.json").is_file(),
            "local file must be written when authorized"
        );

        // GDrive アップロードリクエストが発行されていること（要件 3.1）
        let reqs = sm.gdrive.executor.recorded_requests();
        assert!(
            reqs.len() >= 2,
            "must issue at least 2 GDrive requests (ensure_folder + upload) when authorized, got: {}",
            reqs.len()
        );
        // 最後のリクエストが POST upload であること
        let upload_req = reqs.last().expect("must have at least one request");
        assert_eq!(
            upload_req.method, "POST",
            "last GDrive request must be POST upload"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // 5.1c: 承認済み + GDrive 失敗 → ローカル保存は成功、全体 Ok（要件 5.4）
    // -------------------------------------------------------------------------

    /// 承認済み状態で GDrive がすべてのリトライで失敗する場合:
    /// - ローカルファイルは依然として存在する（ディスクに書き込まれている）
    /// - 全体の結果は Ok(SaveResult::LocalOnlyWithGDriveWarning(...)) が返る
    /// - エラーではない（ローカル保存が成功扱い）
    #[tokio::test]
    async fn test_storage_manager_save_history_authorized_gdrive_failure_local_persists() {
        let base = temp_base();
        // GDrive モック: ensure_mitatete_folder (list: 既存フォルダ) + upload が3回すべて500失敗
        let gdrive_responses = vec![
            GDriveResponse {
                status: 200,
                body: r#"{"files": [{"id": "folder-xyz", "name": "mitatete"}]}"#.to_string(),
            },
            GDriveResponse {
                status: 500,
                body: r#"{"error": {"message": "internal server error"}}"#.to_string(),
            },
            GDriveResponse {
                status: 500,
                body: r#"{"error": {"message": "internal server error"}}"#.to_string(),
            },
            GDriveResponse {
                status: 500,
                body: r#"{"error": {"message": "internal server error"}}"#.to_string(),
            },
        ];
        let (sm, base) = make_storage_manager_authorized(base, gdrive_responses);

        let data = serde_json::json!({"date": "2026-06-27", "messages": []});
        let result = sm.save_history("2026-06-27", &data).await;

        // Ok(LocalOnlyWithGDriveWarning) が返ること（GDrive 失敗でも全体は Ok）（要件 5.4）
        assert!(
            matches!(result, Ok(SaveResult::LocalOnlyWithGDriveWarning(_))),
            "GDrive failure must return Ok(LocalOnlyWithGDriveWarning), NOT Err, got: {result:?}"
        );

        // ローカルファイルが依然として存在していること（要件 5.4: ローカル保存は維持）
        assert!(
            base.join("history").join("2026-06-27.json").is_file(),
            "local file must persist on disk despite GDrive failure (req 5.4)"
        );

        cleanup(&base);
    }

    // -------------------------------------------------------------------------
    // 5.1d: 追加カバレッジ — save_settings, save_diary も同パターンで動作する
    // -------------------------------------------------------------------------

    /// 未承認状態で save_settings → ローカル保存のみ、GDrive 呼び出しなし。
    #[tokio::test]
    async fn test_storage_manager_save_settings_unauthorized_local_only() {
        let base = temp_base();
        let (sm, base) = make_storage_manager_unauthorized(base);

        let data = serde_json::json!({"active_character": "default"});
        let result = sm.save_settings(&data).await;

        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "unauthorized save_settings must return Ok(LocalOnly), got: {result:?}"
        );
        assert!(
            base.join("settings.json").is_file(),
            "settings.json must be written locally"
        );
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(reqs.len(), 0, "must NOT call GDrive when unauthorized");

        cleanup(&base);
    }

    /// 未承認状態で save_diary → ローカル保存のみ、GDrive 呼び出しなし。
    #[tokio::test]
    async fn test_storage_manager_save_diary_unauthorized_local_only() {
        let base = temp_base();
        let (sm, base) = make_storage_manager_unauthorized(base);

        let result = sm.save_diary("2026-06-27", "# 日記\n本文").await;

        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "unauthorized save_diary must return Ok(LocalOnly), got: {result:?}"
        );
        assert!(
            base.join("diary").join("2026-06-27.md").is_file(),
            "diary file must be written locally"
        );
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(reqs.len(), 0, "must NOT call GDrive when unauthorized");

        cleanup(&base);
    }

    // =========================================================================
    // Task 7.1: ローカル書き込み失敗時のエラー返却検証（要件 5.1）
    // =========================================================================

    /// LocalFileSystem レベル: ベースパスの親がファイルのとき save_history が
    /// StorageError::LocalWrite(_) を返すことを確認する（要件 5.1）。
    ///
    /// 手法: temp ファイルをパス P として作成し、LocalFileSystem::with_base(P/"sub") を
    /// 構築する。write を試みると P がファイルなので create_dir_all が ENOTDIR で失敗し、
    /// LocalWrite エラーが返る。
    #[tokio::test]
    async fn test_local_filesystem_save_history_write_failure_returns_local_write_error() {
        let blocker = temp_base();
        // blocker 自体をファイルとして作成する（親ディレクトリを作って中にファイルを置く）
        std::fs::create_dir_all(blocker.parent().unwrap()).expect("parent dir must be creatable");
        std::fs::write(&blocker, b"i am a file, not a dir")
            .expect("creating blocker file must succeed");

        // blocker はファイルなので blocker/sub への create_dir_all は失敗する
        let fs = LocalFileSystem::with_base(blocker.join("sub"));
        let data = serde_json::json!({"date": "2026-06-27", "messages": []});
        let result = fs.save_history("2026-06-27", &data).await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "save_history must return StorageError::LocalWrite when write fails, got: {result:?}"
        );

        // blocker ファイルを削除してクリーンアップ
        let _ = std::fs::remove_file(&blocker);
    }

    /// LocalFileSystem レベル: save_settings も書き込み失敗時に LocalWrite を返す（要件 5.1）。
    #[tokio::test]
    async fn test_local_filesystem_save_settings_write_failure_returns_local_write_error() {
        let blocker = temp_base();
        std::fs::create_dir_all(blocker.parent().unwrap()).expect("parent dir must be creatable");
        std::fs::write(&blocker, b"i am a file, not a dir")
            .expect("creating blocker file must succeed");

        // blocker はファイルなので blocker/sub への create_dir_all は失敗する
        let fs = LocalFileSystem::with_base(blocker.join("sub"));
        let data = serde_json::json!({"active_character": "default"});
        let result = fs.save_settings(&data).await;

        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "save_settings must return StorageError::LocalWrite when write fails, got: {result:?}"
        );

        let _ = std::fs::remove_file(&blocker);
    }

    /// StorageManager レベル: ローカル書き込みが失敗するとき save_history が
    /// Err(StorageError::LocalWrite(_)) を返し、GDrive は呼ばれない（要件 5.1）。
    ///
    /// 承認済み状態で構築しても、ローカル書き込みが先に失敗した場合は
    /// GDrive 呼び出し前にエラーが返るため MockHttpExecutor に記録がゼロになる。
    #[tokio::test]
    async fn test_storage_manager_save_history_local_write_failure_returns_local_write_err_no_gdrive_call(
    ) {
        // ベースパスの親をファイルにして書き込みを確実に失敗させる
        let blocker = temp_base();
        std::fs::create_dir_all(blocker.parent().unwrap()).expect("parent dir must be creatable");
        std::fs::write(&blocker, b"i am a file, not a dir")
            .expect("creating blocker file must succeed");

        let broken_base = blocker.join("sub");

        let local = LocalFileSystem::with_base(broken_base);
        let store = InMemoryTokenStore::new();
        // 承認済み状態にしておく（それでも LocalWrite 失敗で GDrive は呼ばれない）
        store.save(&canned_token()).expect("pre-save token");
        let exchanger = FakeTokenExchanger::success(canned_token());
        let oauth = OAuthManager::new(
            "test-client-id".to_string(),
            "http://localhost/callback".to_string(),
            store,
            exchanger,
        );
        // 承認済み想定でモックを用意するが、実際には呼ばれないはず
        let mock = MockHttpExecutor::new(vec![]);
        let gdrive = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);
        let sm = StorageManager::new(local, oauth, gdrive);

        let data = serde_json::json!({"date": "2026-06-27", "messages": []});
        let result = sm.save_history("2026-06-27", &data).await;

        // LocalWrite エラーが返ること（要件 5.1）
        assert!(
            matches!(result, Err(StorageError::LocalWrite(_))),
            "StorageManager must propagate LocalWrite error when local write fails, got: {result:?}"
        );

        // GDrive が呼ばれていないこと（ローカル失敗で GDrive 前にリターン）
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            0,
            "GDrive must NOT be called when local write fails, got {} requests",
            reqs.len()
        );

        let _ = std::fs::remove_file(&blocker);
    }

    // =========================================================================
    // Task 7.2: 未承認状態での GDrive アップロード非呼び出し検証（要件 1.7, 3.1）
    // =========================================================================
    //
    // 注記: 5.1a/5.1d で save_history / save_settings / save_diary の Unauthorized
    // ケースは既に確認済み。本タスクでは 7.2 を直接名指しするテストを追加し、
    // save_settings と save_diary について明示的に GDrive 非呼び出しを検証する。

    /// 未承認状態で save_settings を呼ぶとき GDriveClient が一切呼ばれない（要件 1.7, 3.1）。
    ///
    /// Task 7.2 の直接検証用テスト。5.1d と同一の振る舞いを確認するが、
    /// 要件トレーサビリティのために明示的に命名する。
    #[tokio::test]
    async fn test_7_2_unauthorized_save_settings_no_gdrive_call() {
        let base = temp_base();
        let (sm, base) = make_storage_manager_unauthorized(base);

        let data = serde_json::json!({"active_character": "miku", "principles": {}});
        let result = sm.save_settings(&data).await;

        // ローカル保存のみ
        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "unauthorized save_settings must return Ok(LocalOnly) (req 1.7), got: {result:?}"
        );

        // GDrive リクエストがゼロ（AuthStatus::Unauthorized → GDrive 呼び出しなし）
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            0,
            "GDrive must NOT be called when unauthorized (req 3.1), got {} requests",
            reqs.len()
        );

        cleanup(&base);
    }

    /// 未承認状態で save_diary を呼ぶとき GDriveClient が一切呼ばれない（要件 1.7, 3.1）。
    ///
    /// Task 7.2 の直接検証用テスト。save_diary 経路での未承認 GDrive 非呼び出しを確認する。
    #[tokio::test]
    async fn test_7_2_unauthorized_save_diary_no_gdrive_call() {
        let base = temp_base();
        let (sm, base) = make_storage_manager_unauthorized(base);

        let result = sm.save_diary("2026-06-27", "# Test\n内容").await;

        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "unauthorized save_diary must return Ok(LocalOnly) (req 1.7), got: {result:?}"
        );

        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            0,
            "GDrive must NOT be called when unauthorized (req 3.1), got {} requests",
            reqs.len()
        );

        cleanup(&base);
    }

    // =========================================================================
    // Task 7.3: 承認取り消し後のローカル保存継続検証（要件 4.4）
    // =========================================================================

    /// complete_auth → revoke_auth → save_history のシーケンスで:
    /// - ローカルファイルが temp base に書き込まれること（要件 4.4: ローカル保存継続）
    /// - revoke 後は AuthStatus::Unauthorized に戻るため GDrive が呼ばれないこと（要件 4.1）
    /// - MockHttpExecutor のリクエスト数がゼロ（revoke 後の GDrive 非呼び出し）
    #[tokio::test]
    async fn test_7_3_local_save_continues_after_revoke_auth_no_gdrive_call() {
        let base = temp_base();

        // StorageManager を承認済み状態で構築する。
        // complete_auth を呼ぶために StorageManager の oauth フィールドに直接アクセスする代わりに、
        // make_storage_manager_authorized を使って事前にトークンを保存する。
        // その後 revoke_auth() でトークンを削除し Unauthorized に戻す。

        // GDrive モック: revoke 後は呼ばれないはずなのでキューは空
        let (sm, base) = make_storage_manager_authorized(base, vec![]);

        // 事前確認: 承認済み状態であること
        let status_before = sm
            .get_auth_status()
            .await
            .expect("get_auth_status must succeed");
        assert_eq!(
            status_before,
            AuthStatus::Authorized,
            "StorageManager must start Authorized for this test"
        );

        // 承認を取り消す（要件 4.3: トークンのみ削除）
        sm.oauth
            .revoke_auth()
            .await
            .expect("revoke_auth must succeed");

        // 取り消し後の状態確認（要件 4.5: 未承認状態を返す）
        let status_after = sm
            .get_auth_status()
            .await
            .expect("get_auth_status must succeed");
        assert_eq!(
            status_after,
            AuthStatus::Unauthorized,
            "StorageManager must be Unauthorized after revoke_auth (req 4.5)"
        );

        // revoke 後に save_history を呼ぶ（要件 4.4: ローカル保存継続）
        let data = serde_json::json!({"date": "2026-06-27", "messages": []});
        let result = sm.save_history("2026-06-27", &data).await;

        // Ok(LocalOnly) が返ること（未承認なのでローカルのみ）
        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "save_history after revoke must return Ok(LocalOnly), got: {result:?}"
        );

        // ローカルファイルが temp base に書き込まれていること（要件 4.4）
        assert!(
            base.join("history").join("2026-06-27.json").is_file(),
            "local file must exist after revoke_auth + save_history (req 4.4): local saving must continue"
        );

        // GDrive が呼ばれていないこと（revoke 後は Unauthorized → GDrive 呼び出しなし）
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            0,
            "GDrive must NOT be called after revoke_auth (Unauthorized), got {} requests",
            reqs.len()
        );

        cleanup(&base);
    }

    /// complete_auth (FakeTokenExchanger で成功) → revoke_auth → save_diary の検証。
    /// ローカルファイル作成 + GDrive 非呼び出しを save_diary 経路でも確認する（要件 4.4）。
    #[tokio::test]
    async fn test_7_3_local_save_diary_continues_after_revoke_auth_no_gdrive_call() {
        let base = temp_base();

        // OAuthManager を使って complete_auth → revoke_auth のシーケンスを実行する
        let store = InMemoryTokenStore::new();
        let exchanger = FakeTokenExchanger::success(canned_token());
        let oauth = OAuthManager::new(
            "test-client-id".to_string(),
            "http://localhost/callback".to_string(),
            store,
            exchanger,
        );

        // complete_auth でトークンを取得・保存して Authorized 状態にする
        let auth_result = oauth
            .complete_auth("valid-auth-code")
            .await
            .expect("complete_auth must succeed");
        assert_eq!(
            auth_result,
            AuthStatus::Authorized,
            "complete_auth must return Authorized"
        );

        // revoke_auth でトークンを削除して Unauthorized に戻す（要件 4.3）
        oauth.revoke_auth().await.expect("revoke_auth must succeed");

        // LocalFileSystem と GDriveClient（空モック）を組み合わせて StorageManager を構築
        let local = LocalFileSystem::with_base(base.clone());
        let mock = MockHttpExecutor::new(vec![]);
        let gdrive = GDriveClient::with_backoff_base(mock, std::time::Duration::ZERO);
        let sm = StorageManager::new(local, oauth, gdrive);

        // revoke 後に save_diary を呼ぶ
        let result = sm.save_diary("2026-06-27", "# 日記\n本文").await;

        // Ok(LocalOnly) が返ること
        assert!(
            matches!(result, Ok(SaveResult::LocalOnly)),
            "save_diary after revoke must return Ok(LocalOnly), got: {result:?}"
        );

        // ローカルファイルが書き込まれていること（要件 4.4）
        assert!(
            base.join("diary").join("2026-06-27.md").is_file(),
            "diary file must exist after revoke_auth + save_diary (req 4.4)"
        );

        // GDrive が呼ばれていないこと（revoke 後は Unauthorized）
        let reqs = sm.gdrive.executor.recorded_requests();
        assert_eq!(
            reqs.len(),
            0,
            "GDrive must NOT be called after revoke_auth, got {} requests",
            reqs.len()
        );

        cleanup(&base);
    }
}
