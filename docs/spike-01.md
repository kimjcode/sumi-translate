# Spike 01 報告 — 觸發 + 權限 + 剪貼簿

**日期：** 2026-06-12（同日修訂：rdev 實測 crash，已換手寫 CGEventTap）
**範圍：** 雙擊 `⌘C` 偵測 → Accessibility 權限引導 → 讀剪貼簿純文字 → 顯示。無翻譯、無 provider、無 Glance/Workbench 樣式。
**結論先講：** spike 達成它的目的——第一版用 rdev，**實機一按鍵整個 App 就被殺掉**；已換成手寫 CGEventTap（core-graphics），從根本上避開 crash 成因。程式可編譯可啟動、純邏輯 6/6 單元測試通過，端到端雙擊觸發待人工驗收。

---

## 事故記錄：rdev 一按鍵就 crash（spike 最重要的發現）

實測現象：授權後在任何 App 按下 ⌘（任何鍵都一樣），App 立即閃退。crash report（`~/Library/Logs/DiagnosticReports/sumi-*.ips`）堆疊：

```
EXC_BREAKPOINT (SIGTRAP)
dispatch_assert_queue_fail            ← libdispatch
← TSMGetInputSourceProperty           ← HIToolbox
← rdev::macos::keyboard::Keyboard::string_from_code
← rdev::macos::common::convert
← rdev::macos::listen::raw_callback
```

成因：rdev 在**每個**鍵盤事件的 callback 裡都會呼叫 TIS/TSM API 把 keycode 轉成字元（填 `event.name`），而這組 API 在新版 macOS（本機 26.3）被 libdispatch assertion **強制只能在主執行緒呼叫**。rdev 的 `listen` 設計上就是跑在背景執行緒 → 第一個鍵盤事件進來就 SIGTRAP，整個 process 被殺。這不是用法錯誤，是 rdev 0.5 在新 macOS 上的結構性問題。

**處置（已核准）**：移除 rdev，改用 `core-graphics` 手寫 CGEventTap。關鍵差異：我們只讀 keycode 整數欄位與 modifier flags，**完全不做 keycode→字元轉換、不碰 TIS API**——偵測 ⌘C 本來就不需要知道字元。`monitor/` 介面當初已隔離，`DoublePressDetector` 與全部單元測試原封不動沿用，只換 OS 接線層。

## 實作摘要

```
src-tauri/src/
  lib.rs                    Tauri 入口：logger、commands、啟動 monitor
  monitor/
    mod.rs                  手寫 CGEventTap（listen-only）、雙擊判定接線、剪貼簿讀取/過濾、事件發送
    double_press.rs         雙擊判定純邏輯（可單測，DOUBLE_PRESS_WINDOW_MS = 300 可調常數）
    accessibility.rs        AXIsProcessTrusted FFI + 開啟系統設定面板
src/
  App.tsx                   三態 UI：權限 onboarding / 待命 / 顯示擷取文字
```

流程：App 啟動 → 後端執行緒每秒輪詢 `AXIsProcessTrusted()` → 權限就緒才建 CGEventTap（session、**ListenOnly**，只訂閱 KeyDown/KeyUp）→ callback 檢查 keycode==kVK_ANSI_C 且 ⌘ flag 在、且非 autorepeat → `DoublePressDetector` 判定「間隔 ≤ 300ms 且兩次之間有放開」→ `arboard` 讀剪貼簿 → 圖片/空值/純空白則 no-op → emit `sumi://captured` → 顯示視窗。前端同步輪詢權限狀態，授權後自動從 onboarding 切到待命畫面。

## 四個問題的回答

### 1. 雙擊偵測穩不穩？有沒有誤觸 / 漏觸？

- rdev 版：**不穩到極點——一按鍵就 crash**（見上）。已換手寫 CGEventTap。
- 判定邏輯（`DoublePressDetector`）以 6 個單元測試鎖住：單擊不觸發；窗內第二擊觸發；超窗不觸發（成為新窗第一擊）；**按住不放的 key-repeat 不誤觸**（CGEventTap 版有雙保險：事件的 AUTOREPEAT 欄位 + 偵測器要求兩次按下間有 release）；觸發後重置（連按三下只觸發一次）。
- 已知限制（記入 P0 待辦）：
  - keycode 比對用 `kVK_ANSI_C`（實體鍵位），**非 QWERTY 實體配置（如 Dvorak）下 ⌘C 的 keycode 不同**會漏觸。正解是啟動時在主執行緒查一次鍵盤配置，不能在 callback 裡查。
  - tap 被系統停用（TapDisabledByTimeout/ByUserInput）目前只記 log 不自動重啟。

### 2. 從第二個 ⌘C 到視窗顯示的延遲？

換 CGEventTap 後尚未實測（需你授權後人工量測）。理論路徑：tap callback（只比對兩個整數欄位）→ 同步讀剪貼簿 → Tauri emit → React render → `window.show()`，全程無網路，預期遠低於 100ms。粗估方式：觸發 log 有 timestamp，對照視窗出現體感即可。

潛在 race：第二次 ⌘C 也會讓前景 App 重寫剪貼簿，我們立即讀取——但第一次 ⌘C 已寫入相同內容，讀到的內容正確。若實測偶發讀到舊內容，解法是延遲 30–50ms 再讀。

### 3. 權限流程體感如何？有沒有 macOS 限制？

- event tap **在偵測到權限就緒之後才建立**（後端每秒輪詢），理論上授權後免重啟。實測：使用者印象是「授權後馬上能用」，且 rdev 版的 crash 發生在 tap callback 內——代表授權後 tap 確實收到事件，**免重啟路徑基本成立**；但因 crash 打斷觀察、後續測試時權限已給過，「剛授權 → 立即可用」沒有被乾淨重測。**P0 onboarding 採保守文案**：寫「授權後自動啟用」，補防呆「若 10 秒內沒反應，請重啟 Sumi」。
- **權限歸屬是「負責的 process」**：dev 模式要授權給啟動 `npm run tauri dev` 的終端機/IDE，不是 `sumi` binary；打包後才是 `Sumi.app`。開發者最容易踩雷的點，已寫進 README。
- 本 spike 用 `AXIsProcessTrusted()`（不帶 prompt）+ App 內按鈕開系統設定。App 可能不會自動出現在清單，需按「+」手動加入；P0 可改用 `AXIsProcessTrustedWithOptions(prompt=true)` 讓系統自動列入（需 CFDictionary，現在 `core-foundation` 已在依賴中，成本低）。
- 重編譯後 TCC 可能認不得舊授權（以路徑/簽章識別），需移除重加。
- 實測 log 確認：無權限時 App 正常啟動並停在 `waiting for Accessibility permission`，不崩潰、不靜默失敗。

### 4. 最後選了哪些 crate、為什麼？

| Crate | 版本 | 理由 |
|---|---|---|
| `tauri` / `tauri-build` | 2.x | 技術選型已鎖定 |
| `serde` / `serde_json` | 1 | Tauri 必要依賴、事件 payload 序列化 |
| `core-graphics` | 0.25 | **取代 rdev**。servo 維護的 CGEventTap 安全封裝，本來就是 rdev 的底層依賴；自己控制 callback 內容，不碰 TIS API |
| `core-foundation` | 0.10 | CFRunLoop（event tap 的 run loop），同為 servo 維護 |
| `arboard` | 3 | 純 Rust 剪貼簿讀取；對圖片/空值回 `Err`，正好用於過濾 |
| `log` + `env_logger` | 0.4 / 0.11 | 遵守「不用 println! 當正式 log」慣例 |
| ~~`rdev`~~ | ~~0.5~~ | **已移除**：在 macOS 26 上一按鍵就 SIGTRAP（見事故記錄） |

Accessibility 檢查不加 crate：直接 `extern "C"` 連結 ApplicationServices 的 `AXIsProcessTrusted`。

**相依性事故記錄**：`time` 0.3.48（tauri-utils → plist 的傳遞依賴）與 rustc 1.96 產生 E0119 coherence 衝突，導致編譯失敗。已在 `Cargo.lock` 釘到 0.3.47。**升級依賴時不要把 time 拉回 0.3.48**，等上游修復版再解除。

## 驗收狀態

| # | 驗收項 | 狀態 |
|---|---|---|
| 1 | 雙擊 ⌘C → 視窗顯示複製文字 | ✅ **人工驗證通過**（2026-06-12，中/英文皆原樣顯示） |
| 2 | 單擊不觸發、不影響正常複製 | 邏輯有單測；tap 為 ListenOnly 不攔截事件；實測未回報異常 |
| 3 | 圖片/空剪貼簿優雅 no-op | 完成（`get_text()` Err / 空白皆不開窗） |
| 4 | 無權限時走引導流程 | 完成並經 log 驗證（onboarding + 一鍵開設定 + 自動輪詢） |
| 5 | 時間窗為可調常數 | 完成（`DOUBLE_PRESS_WINDOW_MS = 300`，`monitor/double_press.rs`） |
| 6 | 無 secret、log 不含剪貼簿內容 | 完成（log 只記字元數；`.gitignore` 含 `.env`/金鑰） |

### 人工驗收時觀察到的行為（spike 符合預期，P0 過濾層處理）

1. **Finder 複製檔案 → 顯示檔名**：Finder 對檔案按 ⌘C 時剪貼簿同時含 file URL 與純文字檔名，spike 只讀純文字所以顯示檔名。P0 過濾規則：剪貼簿含 file-url 型別（`public.file-url` / `NSFilenamesPboardType`）即視為「複製檔案」整個跳過。注意 `arboard` 讀不到型別資訊，屆時需經 NSPasteboard 檢查型別（可能需新增 objc2 系 crate，依紅線先核准）。
2. **複製網址 → 原樣顯示**：spike 標準本來就是原封不動顯示。P0 在語言/路由層加規則：純 URL 不送翻譯（無意義、省 API 成本）。

## 給下一步的建議

1. ~~重新人工驗收~~ → 已通過：按鍵不再 crash、雙擊觸發正常、中英文皆正確顯示。
2. P0 開工前的小決策：改用 `AXIsProcessTrustedWithOptions(prompt=true)` 改善 onboarding（依賴已就緒，免新 crate）。
3. P0 過濾層待辦（語言/路由層，與機密過濾同處）：檔案複製跳過、純 URL 跳過、去重。
4. P0 監聽層待辦：tap 被系統停用時自動重啟；非 QWERTY 實體配置的 keycode 對應。
