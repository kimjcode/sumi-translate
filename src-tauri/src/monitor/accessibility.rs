//! Accessibility（輔助使用）權限檢查與系統設定導引。

use std::process::Command;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// 回傳目前 process 是否已被授予 Accessibility 權限。
    fn AXIsProcessTrusted() -> bool;
}

pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// 直接打開「系統設定 → 隱私權與安全性 → 輔助使用」。
pub fn open_settings_pane() {
    let result = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
    if let Err(e) = result {
        log::error!("failed to open Accessibility settings pane: {e}");
    }
}
