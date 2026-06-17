//! 全域監聽層：雙擊 ⌘C 偵測、Esc / 點擊浮窗外關閉。OS 層邏輯只放這裡。
//!
//! 鍵盤監聽用手寫 CGEventTap（core-graphics），只讀 keycode 與 modifier flags，
//! 刻意不做 keycode→字元轉換：TIS/TSM API 在新版 macOS 只能於主執行緒呼叫，
//! rdev 在監聽 callback 裡呼叫它導致整個 App 被 dispatch assertion 殺掉（見 docs/spike-01.md）。

pub mod accessibility;
pub mod double_press;
pub mod filter;
pub mod pasteboard;

use std::cell::{Cell, RefCell};
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
use double_press::{DoublePressDetector, Press};

/// kVK_ANSI_C：實體 C 鍵位的 keycode（與輸入法/語言無關）。
const KEYCODE_C: i64 = 8;
/// kVK_Escape。浮窗永不取得鍵盤焦點，Esc 只能在全域監聽層處理。
const KEYCODE_ESCAPE: i64 = 53;
/// kVK_Return。⌘↩ 展開到 Workbench——同 Esc，浮窗收不到鍵盤事件，只能在這裡攔。
const KEYCODE_RETURN: i64 = 36;

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
    // callback 是 Fn（非 FnMut），且只會在本執行緒的 run loop 被呼叫 → RefCell/Cell 即可。
    let detector = RefCell::new(DoublePressDetector::new());
    // 第一次 ⌘C 按下時的 changeCount 基準（複製發生前的值）。
    let baseline_cc = Cell::new(0isize);

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
                CGEventType::KeyDown => on_key_down(&app, &detector, &baseline_cc, event),
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

fn on_key_down(
    app: &AppHandle,
    detector: &RefCell<DoublePressDetector>,
    baseline_cc: &Cell<isize>,
    event: &CGEvent,
) {
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
        match detector
            .borrow_mut()
            .on_press(Instant::now(), Duration::from_millis(window_ms))
        {
            // 第一次按下：記下複製發生「前」的 changeCount 當基準。
            Press::Started => baseline_cc.set(pasteboard::change_count()),
            // 雙擊成立：交給主執行緒依「這次有沒有新複製」分流。
            Press::Fired => {
                log::info!("double Cmd+C detected");
                dispatch_double_press(app, baseline_cc.get());
            }
            Press::Ignored => {}
        }
    } else if code == KEYCODE_ESCAPE && glance::is_visible(app) {
        glance::hide(app);
    } else if code == KEYCODE_RETURN
        && event.get_flags().contains(CGEventFlags::CGEventFlagCommand)
        && glance::is_visible(app)
    {
        // ⌘↩：展開到 Workbench。只在 Glance 顯示時生效，不影響系統其他 ⌘↩。
        log::info!("Cmd+Return on Glance → expand to Workbench");
        crate::workbench::expand_from_glance(app);
    }
}

/// 雙擊成立後在主執行緒分流：有新複製 → 翻譯（Glance）；無新複製 → 開空白 Workbench。
/// 在主執行緒讀 changeCount（此時第一次 ⌘C 的複製已落地，比 tap 當下更可靠）。
fn dispatch_double_press(app: &AppHandle, baseline: isize) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if pasteboard::change_count() > baseline {
            log::info!("→ new copy, translate (Glance)");
            pipeline::handle_on_main(app2);
        } else {
            log::info!("→ no new copy, open blank Workbench");
            crate::workbench::open_blank(&app2);
        }
    });
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
