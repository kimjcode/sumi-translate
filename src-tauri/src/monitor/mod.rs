//! 全域監聽層：雙擊 ⌘C 偵測、剪貼簿讀取、過濾。
//! OS 層邏輯只放這裡（CLAUDE.md 邊界規則）。
//!
//! 鍵盤監聽用手寫 CGEventTap（core-graphics），只讀 keycode 與 modifier flags，
//! 刻意不做 keycode→字元轉換：TIS/TSM API 在新版 macOS 只能於主執行緒呼叫，
//! rdev 在監聽 callback 裡呼叫它導致整個 App 被 dispatch assertion 殺掉（見 docs/spike-01.md）。

pub mod accessibility;
pub mod double_press;

use std::cell::RefCell;
use std::thread;
use std::time::{Duration, Instant};

use core_foundation::runloop::CFRunLoop;
use core_graphics::event::{
    CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
    CGEventType, CallbackResult, EventField,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use double_press::{DoublePressDetector, DOUBLE_PRESS_WINDOW_MS};

/// 雙擊觸發後發給前端的事件名。
pub const CAPTURED_EVENT: &str = "sumi://captured";

/// kVK_ANSI_C：實體 C 鍵位的 keycode（與輸入法/語言無關）。
const KEYCODE_C: i64 = 8;

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
    // callback 是 Fn（非 FnMut），且只會在本執行緒的 run loop 被呼叫 → RefCell 即可。
    let detector = RefCell::new(DoublePressDetector::new(Duration::from_millis(
        DOUBLE_PRESS_WINDOW_MS,
    )));

    let result = CGEventTap::with_enabled(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        // ListenOnly：被動監聽，絕不攔截/修改事件，不影響正常複製行為。
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown, CGEventType::KeyUp],
        |_proxy, etype, event| {
            match etype {
                CGEventType::KeyDown
                    if event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                        == KEYCODE_C =>
                {
                    let cmd_held = event
                        .get_flags()
                        .contains(CGEventFlags::CGEventFlagCommand);
                    let autorepeat = event
                        .get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT)
                        != 0;
                    if cmd_held
                        && !autorepeat
                        && detector.borrow_mut().on_press(Instant::now())
                    {
                        log::info!("double Cmd+C detected");
                        on_trigger(&app);
                    }
                }
                CGEventType::KeyUp
                    if event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
                        == KEYCODE_C =>
                {
                    detector.borrow_mut().on_release();
                }
                // 系統因 callback 過慢或使用者操作而停用 tap 時會收到這兩種事件；
                // 先記 log，重新啟用列為 P0 待辦。
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    log::warn!("event tap disabled by system ({etype:?})");
                }
                _ => {}
            }
            CallbackResult::Keep
        },
        // tap 掛上本執行緒 run loop 後在此阻塞處理事件。
        || CFRunLoop::run_current(),
    );

    if result.is_err() {
        // 建 tap 失敗（多半是權限問題）時優雅降級：App 照常活著，只是雙擊不會動。
        log::error!("failed to create CGEventTap; double Cmd+C trigger disabled");
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
