//! Accessibility（輔助使用）權限檢查與系統設定導引。

use std::process::Command;

use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// 回傳目前 process 是否已被授予 Accessibility 權限。
    fn AXIsProcessTrusted() -> bool;
    /// 同上，但可帶選項；`prompt=true` 會跳系統原生授權框並把 App 列入清單。
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// 跳系統原生授權框（macOS 一個 session 通常只顯示一次）。
/// 必須在主執行緒呼叫；呼叫時機應在 App 自己的說明之後。
pub fn request_trust_with_prompt() -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let options = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            CFBoolean::true_value().as_CFType(),
        )]);
        AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
    }
}

/// 手動後路：直接打開「系統設定 → 隱私權與安全性 → 輔助使用」（給關掉原生框的人）。
pub fn open_settings_pane() {
    let result = Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
    if let Err(e) = result {
        log::error!("failed to open Accessibility settings pane: {e}");
    }
}
