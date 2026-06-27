// Mitatete バックエンドのエントリポイント。
// ウィンドウ（チャットUI / キャラクターウィンドウ）は tauri.conf.json で定義する。

pub mod storage;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|_app| {
            // アプリ起動時に ~/.mitatete/ ディレクトリ構造を初期化する。
            // 失敗しても（ホームディレクトリ解決不可など）アプリ起動は継続する。
            storage::init_storage_dirs();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
