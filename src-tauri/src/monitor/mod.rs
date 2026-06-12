//! 全域監聽層：雙擊 ⌘C 偵測、Esc / 點擊浮窗外關閉。OS 層邏輯只放這裡。
//!
//! 鍵盤監聽用手寫 CGEventTap（core-graphics），只讀 keycode 與 modifier flags，
//! 刻意不做 keycode→字元轉換：TIS/TSM API 在新版 macOS 只能於主執行緒呼叫，
//! rdev 在監聽 callback 裡呼叫它導致整個 App 被 dispatch assertion 殺掉（見 docs/spike-01.md）。

pub mod accessibility;
pub mod double_press;
pub mod filter;
pub mod pasteboard;

use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use core_foundation::runloop::CFRunLoop;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, CallbackResult, EventField,
};
use tauri::{AppHandle, Manager};

use crate::pipeline;
use crate::settings::SettingsState;
use crate::windows::glance;
use double_press::DoublePressDetector;

/// kVK_ANSI_C：實體 C 鍵位的 keycode（與輸入法/語言無關）。
const KEYCODE_C: i64 = 8;
/// kVK_Escape。浮窗永不取得鍵盤焦點，Esc 只能在全域監聽層處理。
const KEYCODE_ESCAPE: i64 = 53;

/// 啟動全域監聽。等到 Accessibility 權限就緒才建立 event tap，
/// 因此授權後不需重啟 App。
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
        log::info!("Accessibility permission OK, starting global event listener");
        run_listener(app);
    });
}

fn run_listener(app: AppHandle) {
    // callback 是 Fn（非 FnMut），且只會在本執行緒的 run loop 被呼叫 → RefCell 即可。
    let detector = RefCell::new(DoublePressDetector::new());

    let result = CGEventTap::with_enabled(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        // ListenOnly：被動監聽，絕不攔截/修改事件，不影響正常複製行為。
        CGEventTapOptions::ListenOnly,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::LeftMouseDown,
            CGEventType::RightMouseDown,
            CGEventType::OtherMouseDown,
        ],
        |_proxy, etype, event| {
            match etype {
                CGEventType::KeyDown => on_key_down(&app, &detector, event),
                CGEventType::KeyUp => {
                    if keycode(event) == KEYCODE_C {
                        detector.borrow_mut().on_release();
                    }
                }
                CGEventType::LeftMouseDown
                | CGEventType::RightMouseDown
                | CGEventType::OtherMouseDown => on_mouse_down(&app, event),
                // 系統因 callback 過慢或使用者操作而停用 tap 時會收到這兩種事件；
                // 先記 log，重新啟用列為待辦。
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

fn keycode(event: &CGEvent) -> i64 {
    event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE)
}

fn on_key_down(app: &AppHandle, detector: &RefCell<DoublePressDetector>, event: &CGEvent) {
    let code = keycode(event);
    if code == KEYCODE_C {
        let cmd_held = event.get_flags().contains(CGEventFlags::CGEventFlagCommand);
        let autorepeat =
            event.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;
        if !cmd_held || autorepeat {
            return;
        }
        let window_ms = app
            .state::<SettingsState>()
            .double_press_ms
            .load(Ordering::Relaxed);
        if detector
            .borrow_mut()
            .on_press(Instant::now(), Duration::from_millis(window_ms))
        {
            log::info!("double Cmd+C detected");
            pipeline::trigger(app);
        }
    } else if code == KEYCODE_ESCAPE && glance::is_visible(app) {
        glance::hide(app);
    }
}

fn on_mouse_down(app: &AppHandle, event: &CGEvent) {
    if !glance::is_visible(app) {
        return;
    }
    let location = event.location();
    if glance::contains_point(app, location.x, location.y) {
        // 點在浮窗內：視為活動，重置閒置計時。
        glance::touch_idle(app);
    } else {
        // 點別處：等同失焦，關閉。
        glance::hide(app);
    }
}
