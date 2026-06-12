mod monitor;

use env_logger::Env;

/// 前端輪詢用：目前是否已有 Accessibility 權限。
#[tauri::command]
fn accessibility_status() -> bool {
    monitor::accessibility::is_trusted()
}

/// 打開「系統設定 → 隱私權與安全性 → 輔助使用」。
#[tauri::command]
fn open_accessibility_settings() {
    monitor::accessibility::open_settings_pane();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            accessibility_status,
            open_accessibility_settings
        ])
        .setup(|app| {
            monitor::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
