//! 視窗管理層：Glance = non-activating NSPanel；Workbench = 一般視窗（會拿 focus）。

pub mod glance;
pub mod workbench;

use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;

/// 設成 accessory（選單列常駐）：不顯示 Dock 圖示、不進 ⌘Tab，純背景。必須在主執行緒呼叫。
pub fn set_accessory_activation_policy() {
    let Some(mtm) = MainThreadMarker::new() else {
        log::error!("set_accessory_activation_policy must run on the main thread");
        return;
    };
    NSApplication::sharedApplication(mtm).setActivationPolicy(NSApplicationActivationPolicy::Accessory);
}

/// 把 App 帶到前景。accessory 模式下，要讓 Workbench / 設定視窗拿到鍵盤焦點就得先 activate
/// （Glance 刻意不呼叫此函式，維持 non-activating、不搶焦點）。必須在主執行緒呼叫。
pub fn activate_app() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    #[allow(deprecated)]
    NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
}
