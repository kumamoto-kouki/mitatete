// Mitatete バックエンドのエントリポイント。
// ウィンドウ（チャットUI / キャラクターウィンドウ）は tauri.conf.json で定義する。
// model_router / key_manager / storage 等のモジュールは各 spec の実装時に追加する。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
