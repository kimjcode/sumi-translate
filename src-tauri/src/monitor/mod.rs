//! 全域監聽層：雙擊 ⌘C 偵測、剪貼簿讀取、過濾。
//! OS 層邏輯只放這裡（CLAUDE.md 邊界規則）。

pub mod accessibility;
pub mod double_press;

use std::thread;
use std::time::{Duration, Instant};

use rdev::{Event, EventType, Key};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use double_press::{DoublePressDetector, DOUBLE_PRESS_WINDOW_MS};

/// 雙擊觸發後發給前端的事件名。
pub const CAPTURED_EVENT: &str = "sumi://captured";

#[derive(Clone, Serialize)]
struct CapturedPayload {
    text: String,
    char_count: usize,
}

/// 啟動全域監聽。等到 Accessibility 權限就緒才建立 event tap，
/// 因此授權後不需重啟 App（若實測發現 macOS 仍要求重啟，記入 spike 報告）。
pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let mut waited = false;
        while !accessibility::is_trusted() {
            if !waited {
                log::info!("waiting for Accessibility permission before starting event tap");
                waited = true;
            }
            thread::sleep(Duration::from_secs(1));
        }
        log::info!("Accessibility permission OK, starting global key listener");
        run_listener(app);
    });
}

fn run_listener(app: AppHandle) {
    let mut detector =
        DoublePressDetector::new(Duration::from_millis(DOUBLE_PRESS_WINDOW_MS));
    let mut meta_down = false;

    let callback = move |event: Event| match event.event_type {
        EventType::KeyPress(Key::MetaLeft) | EventType::KeyPress(Key::MetaRight) => {
            meta_down = true;
        }
        EventType::KeyRelease(Key::MetaLeft) | EventType::KeyRelease(Key::MetaRight) => {
            meta_down = false;
        }
        EventType::KeyPress(Key::KeyC) if meta_down => {
            if detector.on_press(Instant::now()) {
                log::info!("double Cmd+C detected");
                on_trigger(&app);
            }
        }
        EventType::KeyRelease(Key::KeyC) => {
            detector.on_release();
        }
        _ => {}
    };

    // rdev::listen 會阻塞並在本執行緒跑 CFRunLoop（macOS 為 CGEventTap）。
    if let Err(e) = rdev::listen(callback) {
        // 監聽失敗時優雅降級：App 照常活著，只是雙擊不會動。
        log::error!("global key listener failed: {e:?}");
    }
}

fn on_trigger(app: &AppHandle) {
    let text = match read_clipboard_text() {
        Some(t) => t,
        None => {
            // 圖片、空值、純空白 → 優雅 no-op，不開窗。
            log::info!("clipboard has no usable text, ignoring trigger");
            return;
        }
    };

    // 紅線：log 只記長度，絕不記內容。
    log::info!("captured clipboard text ({} chars)", text.chars().count());

    let payload = CapturedPayload {
        char_count: text.chars().count(),
        text,
    };
    if let Err(e) = app.emit(CAPTURED_EVENT, payload) {
        log::error!("failed to emit captured event: {e}");
    }

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn read_clipboard_text() -> Option<String> {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            log::error!("failed to access clipboard: {e}");
            return None;
        }
    };
    match clipboard.get_text() {
        Ok(t) if !t.trim().is_empty() => Some(t),
        // get_text() 對圖片/空剪貼簿會回 Err，一律視為無內容。
        _ => None,
    }
}
