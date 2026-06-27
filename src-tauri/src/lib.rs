// Mitatete バックエンドのエントリポイント。
// ウィンドウ（チャットUI / キャラクターウィンドウ）は tauri.conf.json で定義する。

pub mod storage;

use storage::{
    AppStorage, GDriveClient, GoogleTokenExchanger, KeyringTokenStore, LocalFileSystem,
    OAuthManager, ReqwestExecutor, StorageManager,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // アプリ起動時に ~/.mitatete/ ディレクトリ構造を初期化する。
            // 失敗しても（ホームディレクトリ解決不可など）アプリ起動は継続する。
            storage::init_storage_dirs();

            // AppStorage（プロダクション具象型）のインスタンスを生成し、
            // Tauri のマネージドステートに登録する。
            // Tauri コマンドは tauri::State<'_, AppStorage> でこのインスタンスを参照する。
            //
            // Google OAuth クレデンシャル:
            // 現時点では OAuth アプリ登録前のため、環境変数から読み込むか空文字列を使用する。
            // 実際の認証は MITATETE_GOOGLE_CLIENT_ID / MITATETE_GOOGLE_CLIENT_SECRET /
            // MITATETE_GOOGLE_REDIRECT_URI が設定された後に機能する（6.1 受け入れ基準）。
            // ローカルファイル操作コマンド（save_history 等）はクレデンシャル不要で動作する。
            let client_id = std::env::var("MITATETE_GOOGLE_CLIENT_ID").unwrap_or_default();
            let client_secret = std::env::var("MITATETE_GOOGLE_CLIENT_SECRET").unwrap_or_default();
            let redirect_uri = std::env::var("MITATETE_GOOGLE_REDIRECT_URI").unwrap_or_default();

            let local = LocalFileSystem::new().unwrap_or_else(|| {
                // ホームディレクトリ解決不可の場合は一時ディレクトリにフォールバック
                // （アプリ起動を止めないための安全策）
                LocalFileSystem::with_base(std::env::temp_dir().join("mitatete"))
            });

            let token_store = KeyringTokenStore::new();
            let exchanger =
                GoogleTokenExchanger::new(client_id.clone(), client_secret, redirect_uri.clone());
            let oauth = OAuthManager::new(client_id, redirect_uri, token_store, exchanger);
            let gdrive = GDriveClient::new(ReqwestExecutor);

            let app_storage: AppStorage = StorageManager::new(local, oauth, gdrive);
            app.manage(app_storage);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            storage::save_history,
            storage::read_history,
            storage::save_settings,
            storage::read_settings,
            storage::save_character,
            storage::save_diary,
            storage::get_auth_status,
            storage::start_oauth,
            storage::revoke_auth,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
