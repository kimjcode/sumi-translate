//! Glance 浮窗：真 NSPanel（nonactivatingPanel），不搶 focus。
//!
//! 做法：用 Tauri 建一般視窗後，把底層 NSWindow 的 class 換成自訂 NSPanel 子類
//! （覆寫 canBecomeKeyWindow=false），再補上 nonactivatingPanel style mask。
//! 顯示用 orderFrontRegardless、隱藏用 orderOut，全程不經過會搶焦點的路徑。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use objc2::runtime::AnyObject;
use objc2::{define_class, ClassType, MainThreadOnly};
use objc2_app_kit::{NSPanel, NSWindowCollectionBehavior, NSWindowStyleMask};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::settings::SettingsState;

pub const GLANCE_LABEL: &str = "glance";
/// 隱藏前先讓前端淡出（ui-spec：120ms）。
const FADE_OUT_MS: u64 = 130;
pub const WILL_HIDE_EVENT: &str = "glance://will-hide";

define_class!(
    // SAFETY：NSPanel 無子類化限制；本型別無 ivars、不實作 Drop。
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "SumiGlancePanel"]
    pub struct GlancePanel;

    impl GlancePanel {
        // 永不成為 key window：點擊浮窗也不奪走前景 App 的鍵盤焦點。
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

#[derive(Default)]
pub struct GlanceState {
    pub visible: AtomicBool,
    /// 每次 show / 活動 +1；閒置計時器以此判斷自己是否已過期。
    pub idle_generation: AtomicU64,
}

/// 啟動時建立浮窗（隱藏）並轉成 NSPanel。必須在主執行緒（Tauri setup）呼叫。
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, GLANCE_LABEL, WebviewUrl::App("index.html".into()))
        .title("Sumi Glance")
        .inner_size(340.0, 136.0)
        .visible(false)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .focused(false)
        .accept_first_mouse(true)
        .build()?;
    convert_to_panel(&window)
}

fn convert_to_panel(window: &WebviewWindow) -> tauri::Result<()> {
    let ptr = window.ns_window()? as *mut AnyObject;
    unsafe {
        let class = GlancePanel::class();
        objc2::ffi::object_setClass(ptr.cast(), (class as *const objc2::runtime::AnyClass).cast());
        let panel = &*(ptr as *const NSPanel);
        panel.setStyleMask(panel.styleMask() | NSWindowStyleMask::NonactivatingPanel);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary,
        );
    }
    Ok(())
}

fn with_panel(window: &WebviewWindow, f: impl FnOnce(&NSPanel)) {
    if let Ok(ptr) = window.ns_window() {
        // SAFETY：init 時已將該 NSWindow 換成 NSPanel 子類；只在主執行緒呼叫。
        let panel = unsafe { &*(ptr as *const NSPanel) };
        f(panel);
    }
}

pub fn is_visible(app: &AppHandle) -> bool {
    app.state::<GlanceState>().visible.load(Ordering::SeqCst)
}

/// 在游標附近顯示浮窗（不搶 focus）。任意執行緒可呼叫。
pub fn show_at_cursor(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = app2.get_webview_window(GLANCE_LABEL) else {
            return;
        };
        position_near_cursor(&app2, &window);
        with_panel(&window, |panel| panel.orderFrontRegardless());
        app2.state::<GlanceState>().visible.store(true, Ordering::SeqCst);
    });
    touch_idle(app);
}

/// 隱藏浮窗：先通知前端淡出，120ms 後 orderOut。任意執行緒可呼叫。
pub fn hide(app: &AppHandle) {
    let state = app.state::<GlanceState>();
    if !state.visible.swap(false, Ordering::SeqCst) {
        return;
    }
    let _ = app.emit_to(GLANCE_LABEL, WILL_HIDE_EVENT, ());
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(FADE_OUT_MS));
        let app3 = app2.clone();
        let _ = app2.run_on_main_thread(move || {
            // 淡出期間若又觸發了 show，放棄這次隱藏。
            if app3.state::<GlanceState>().visible.load(Ordering::SeqCst) {
                return;
            }
            if let Some(window) = app3.get_webview_window(GLANCE_LABEL) {
                with_panel(&window, |panel| panel.orderOut(None));
            }
        });
    });
}

/// 重置閒置計時：每次 show、收到結果、滑鼠在浮窗上活動時呼叫。
pub fn touch_idle(app: &AppHandle) {
    let state = app.state::<GlanceState>();
    let generation = state.idle_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let idle_ms = app
        .state::<SettingsState>()
        .snapshot()
        .idle_close_ms;
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(idle_ms));
        let state = app2.state::<GlanceState>();
        if state.idle_generation.load(Ordering::SeqCst) == generation
            && state.visible.load(Ordering::SeqCst)
        {
            hide(&app2);
        }
    });
}

/// 全域座標（點，top-left 原點，= CGEvent location 的座標系）是否落在浮窗內。
pub fn contains_point(app: &AppHandle, x_pts: f64, y_pts: f64) -> bool {
    let Some(window) = app.get_webview_window(GLANCE_LABEL) else {
        return false;
    };
    let (Ok(pos), Ok(size), Ok(scale)) = (
        window.outer_position(),
        window.outer_size(),
        window.scale_factor(),
    ) else {
        return false;
    };
    let x = x_pts * scale;
    let y = y_pts * scale;
    x >= pos.x as f64
        && x <= pos.x as f64 + size.width as f64
        && y >= pos.y as f64
        && y <= pos.y as f64 + size.height as f64
}

/// 把浮窗放在游標右下方，並夾在游標所在螢幕的工作區內；下方放不下就改放游標上方。
fn position_near_cursor(app: &AppHandle, window: &WebviewWindow) {
    let Ok(cursor) = app.cursor_position() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let scale = window.scale_factor().unwrap_or(2.0);
    let offset_x = 12.0 * scale;
    let offset_y = 18.0 * scale;

    let mut x = cursor.x + offset_x;
    let mut y = cursor.y + offset_y;

    if let Ok(Some(monitor)) = app.monitor_from_point(cursor.x, cursor.y) {
        let area_pos = monitor.position();
        let area_size = monitor.size();
        let max_x = area_pos.x as f64 + area_size.width as f64 - size.width as f64 - 8.0;
        let max_y = area_pos.y as f64 + area_size.height as f64 - size.height as f64 - 8.0;
        if y > max_y {
            // 下方放不下 → 放游標上方。
            y = cursor.y - size.height as f64 - offset_y;
        }
        x = x.clamp(area_pos.x as f64 + 8.0, max_x.max(area_pos.x as f64 + 8.0));
        y = y.max(area_pos.y as f64 + 8.0);
    }

    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
}
