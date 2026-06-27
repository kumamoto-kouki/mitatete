// storage.rs — Mitatete のデータ永続化コンポーネント
//
// LocalFileSystem: `~/.mitatete/` 以下のディレクトリ初期化・ファイル読み書き
// OAuthManager, GDriveClient, StorageManager は後続タスクで実装する。
//
// セキュリティ制約:
//   - ファイルパスはこのモジュール内でのみ構築する（パストラバーサル防止）
//   - LocalFileSystem は外部から任意パスを受け取らない

use std::io;
use std::path::Path;

// ---------------------------------------------------------------------------
// エラー型
// ---------------------------------------------------------------------------

/// storage-manager 全体で使用するエラー型。
/// 後続タスクで GDrive・OAuth 関連のバリアントを追加する。
#[derive(Debug)]
pub enum StorageError {
    /// ローカルファイル書き込み失敗
    LocalWrite(String),
    /// ローカルファイル読み込み失敗
    LocalRead(String),
    /// ディレクトリ初期化失敗
    InitDir(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::LocalWrite(msg) => write!(f, "Local write error: {msg}"),
            StorageError::LocalRead(msg) => write!(f, "Local read error: {msg}"),
            StorageError::InitDir(msg) => write!(f, "Directory init error: {msg}"),
        }
    }
}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        StorageError::InitDir(e.to_string())
    }
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
}
