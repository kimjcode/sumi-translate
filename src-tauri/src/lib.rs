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

/// 顯示設定視窗並拿到焦點（accessory 下需先 activate）。tray「設定」、Dock reopen、首次引導共用。
fn show_settings(app: &AppHandle) {
    windows::activate_app();
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}

/// 建立選單列 tray icon + 選單（設定 / 版本 / 結束）。
fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::TrayIconBuilder;

    let settings_i = MenuItem::with_id(app, "settings", "設定…", true, None::<&str>)?;
    // 停用的版本列（兼作「關於」）。
    let version_i = MenuItem::with_id(
        app,
        "version",
        format!("Sumi v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let quit_i = PredefinedMenuItem::quit(app, Some("結束 Sumi"))?;
    let menu = Menu::with_items(
        app,
        &[
            &settings_i,
            &PredefinedMenuItem::separator(app)?,
            &version_i,
            &quit_i,
        ],
    )?;

    // 單色 template icon，隨深/淺色選單列自動配色。
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id("sumi-tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Sumi")
        .menu(&menu)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "settings" {
                show_settings(app);
            }
        })
        .build(app)?;
    Ok(())
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
            // 選單列常駐：隱藏 Dock 圖示、不進 ⌘Tab。
            windows::set_accessory_activation_policy();
            build_tray(&handle)?;
            // 設定（main）視窗關閉 → 隱藏而非銷毀，確保之後（選單列 / Dock reopen）能重新叫出。
            if let Some(main) = handle.get_webview_window("main") {
                let w = main.clone();
                main.on_window_event(move |e| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = e {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
            }
            // 首次啟動（尚未取得輔助使用權限）→ 顯示設定視窗做 onboarding；
            // 已授權則維持隱形，只留選單列（平常隱形的常駐工具）。
            if !monitor::accessibility::is_trusted() {
                show_settings(&handle);
            }
            monitor::spawn(handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // 點 Dock icon（applicationShouldHandleReopen）→ 重新顯示設定視窗。
            // accessory 沒 Dock 圖示，這條主要是保險；正規入口是選單列「設定」。
            if let tauri::RunEvent::Reopen { .. } = event {
                show_settings(app);
            }
        });
}
