//! NSPasteboard 型別檢查：偵測「複製檔案」（Finder ⌘C 會同時放 file URL 與純文字檔名）。

use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL};
use objc2_foundation::NSArray;

/// 剪貼簿目前內容是否包含檔案 URL（= 使用者複製的是檔案，不是文字）。
/// 只能在主執行緒呼叫（NSPasteboard 未保證執行緒安全）。
pub fn has_file_url() -> bool {
    unsafe {
        let pasteboard = NSPasteboard::generalPasteboard();
        let types = NSArray::from_slice(&[NSPasteboardTypeFileURL]);
        pasteboard.availableTypeFromArray(&types).is_some()
    }
}

/// 剪貼簿的 changeCount——每次被寫入就 +1。用來判斷「這次 ⌘C 有沒有產生新複製」。
/// 只讀整數、不取內容（紅線）；clipboard manager 慣例可於背景執行緒讀取。
pub fn change_count() -> isize {
    NSPasteboard::generalPasteboard().changeCount()
}
