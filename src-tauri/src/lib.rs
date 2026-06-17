mod monitor;
mod pipeline;
mod providers;
mod router;
mod settings;
mod windows;
mod workbench;

use env_logger::Env;
use tauri::{AppHandle, Manager};

/// 前端輪詢用：目前是否已有 Accessibility 權限。
#[tauri::command]
fn accessibility_status() -> bool {
    monitor::accessibility::is_trusted()
}

/// 跳系統原生授權框（在 App 自己的說明之後呼叫；一個 session 通常只顯示一次）。
#[tauri::command]
fn request_accessibility(app: AppHandle) {
    let _ = app.run_on_main_thread(|| {
        monitor::accessibility::request_trust_with_prompt();
    });
}

/// 手動後路：打開「系統設定 → 隱私權與安全性 → 輔助使用」。
#[tauri::command]
fn open_accessibility_settings() {
    monitor::accessibility::open_settings_pane();
}

/// Glance 前端回報滑鼠活動 → 重置閒置計時。
#[tauri::command]
fn glance_activity(app: AppHandle) {
    if windows::glance::is_visible(&app) {
        windows::glance::touch_idle(&app);
    }
}

/// Glance 前端隱藏自己（展開到 Workbench 前先收掉浮窗）。
#[tauri::command]
fn hide_glance(app: AppHandle) {
    windows::glance::hide(&app);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            accessibility_status,
            request_accessibility,
            open_accessibility_settings,
            glance_activity,
            hide_glance,
            settings::get_settings,
            settings::set_settings,
            settings::set_api_key,
            settings::api_key_set,
            settings::clear_api_key,
            settings::set_llm_key,
            settings::llm_key_set,
            settings::clear_llm_key,
            workbench::open_workbench,
            workbench::get_workbench_input,
            workbench::close_workbench,
            workbench::workbench_translate,
            workbench::dictionary_lookup,
            workbench::gemini_define,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let loaded = settings::load(&handle);
            app.manage(settings::SettingsState::new(loaded));
            app.manage(windows::glance::GlanceState::default());
            app.manage(pipeline::PipelineState::new());
            app.manage(workbench::WorkbenchState::new());
            windows::glance::init(&handle)?;
            windows::workbench::init(&handle)?;
            // 設定（main）視窗關閉 → 隱藏而非銷毀，確保之後（Dock icon / 選單列）能重新叫出。
            if let Some(main) = handle.get_webview_window("main") {
                let w = main.clone();
                main.on_window_event(move |e| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }
            monitor::spawn(handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // 點 Dock icon（applicationShouldHandleReopen）→ 重新顯示設定視窗。
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.show();
                    let _ = main.set_focus();
                }
            }
        });
}
