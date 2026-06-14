//! Workbench 視窗：一般視窗，**會拿鍵盤焦點**（與 Glance 的 non-activating panel 相反），
//! 因為要能編輯原文。啟動時建立（隱藏），展開時 show + focus。

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

pub const WORKBENCH_LABEL: &str = "workbench";

/// 啟動時建立 Workbench 視窗（隱藏）。必須在主執行緒（Tauri setup）呼叫。
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let window =
        WebviewWindowBuilder::new(app, WORKBENCH_LABEL, WebviewUrl::App("index.html".into()))
            .title("Sumi Workbench")
            .inner_size(720.0, 460.0)
            .min_inner_size(360.0, 320.0)
            .visible(false)
            .resizable(true)
            .focused(false)
            .build()?;

    // 關閉鈕：隱藏而非銷毀，視窗才能下次再 show（否則第二次展開找不到視窗）。
    let win = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = win.hide();
        }
    });
    Ok(())
}

/// 顯示 Workbench 並拿到鍵盤焦點（一般視窗，乾淨 activate）。
pub fn show(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = app2.get_webview_window(WORKBENCH_LABEL) {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    });
}

/// 隱藏 Workbench（`Esc` 或關閉鈕）。
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WORKBENCH_LABEL) {
        let _ = window.hide();
    }
}
