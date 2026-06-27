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
    let home = match std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .or_else(|| {
            // Windows/Linux 両対応のフォールバック
            dirs_home()
        }) {
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

/// ホームディレクトリの解決（`HOME` 環境変数が使えない場合のフォールバック）。
/// 標準ライブラリのみを使用し、外部クレートに依存しない。
fn dirs_home() -> Option<std::path::PathBuf> {
    // Windows では USERPROFILE または HOMEDRIVE+HOMEPATH を試みる
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
}
