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
