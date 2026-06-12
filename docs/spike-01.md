# Spike 01 報告 — 觸發 + 權限 + 剪貼簿

**日期：** 2026-06-12
**範圍：** 雙擊 `⌘C` 偵測 → Accessibility 權限引導 → 讀剪貼簿純文字 → 顯示。無翻譯、無 provider、無 Glance/Workbench 樣式。
**結論先講：** 程式碼路徑全部打通、可編譯可啟動、純邏輯 6/6 單元測試通過；但**端到端雙擊觸發尚待人工驗證**（需要在系統設定授予 Accessibility 權限，此步驟無法自動化）。

---

## 實作摘要

```
src-tauri/src/
  lib.rs                    Tauri 入口：logger、commands、啟動 monitor
  monitor/
    mod.rs                  rdev 全域監聽執行緒、⌘ 狀態追蹤、剪貼簿讀取/過濾、事件發送
    double_press.rs         雙擊判定純邏輯（可單測，DOUBLE_PRESS_WINDOW_MS = 300 可調常數）
    accessibility.rs        AXIsProcessTrusted FFI + 開啟系統設定面板
src/
  App.tsx                   三態 UI：權限 onboarding / 待命 / 顯示擷取文字
```

流程：App 啟動 → 後端執行緒每秒輪詢 `AXIsProcessTrusted()` → 權限就緒才呼叫 `rdev::listen`（建立 CGEventTap）→ 偵測「⌘ 按住 + C 兩次按下（間隔 ≤ 300ms，且兩次之間必須有放開）」→ `arboard` 讀剪貼簿 → 圖片/空值/純空白則 no-op → emit `sumi://captured`（含文字與字元數）→ 顯示視窗。前端同步每秒輪詢權限狀態，授權後自動從 onboarding 切到待命畫面。

## 四個問題的回答

### 1. 雙擊偵測穩不穩？有沒有誤觸 / 漏觸？

判定邏輯（`DoublePressDetector`）已用 6 個單元測試鎖住行為：

- 單擊不觸發；窗內第二擊觸發；超窗不觸發（超窗那一下成為新窗的第一擊）。
- **按住 ⌘C 不放的 OS key-repeat 不會誤觸**：要求兩次按下之間必須有 KeyRelease。
- 觸發後狀態重置，第三擊重新計窗（連按三下只觸發一次）。

**待人工驗證**（我無法替這個環境授權 Accessibility）：實機上 rdev 事件是否漏接、在輸入法切換/安全輸入模式（如密碼欄）下的行為。已知風險：rdev 維護緩慢（其依賴 `block` v0.1.6 已被 cargo 標記 future-incompatibility 警告）；若實測不穩，備案是改用 `core-graphics` 手寫 CGEventTap。

### 2. 從第二個 ⌘C 到視窗顯示的延遲？

**未實測**（同上，需權限後人工量測）。理論路徑：CGEventTap callback → 讀剪貼簿（同步、本機）→ Tauri emit → React setState → `window.show()`，全程無網路、無重編譯，預期遠低於 100ms。實測方式：觸發時 log 已帶 timestamp，肉眼對照視窗出現即可粗估；如需精確，下個迭代在前端收到事件時補一條 latency log。

注意一個潛在 race：第二次 ⌘C 也會讓前景 App 重寫剪貼簿，我們是立即讀取——但第一次 ⌘C 已寫入相同內容，所以讀到的內容正確。若實測發現偶發讀到舊內容，解法是延遲 30–50ms 再讀。

### 3. 權限流程體感如何？有沒有 macOS 限制？

設計上已避開「必須重啟」的常見原因：**event tap 是在偵測到權限就緒之後才建立**（後端每秒輪詢，未授權前不碰 CGEventTap），所以理論上授權後不需重啟。需人工確認實際行為；若 macOS 仍要求重啟，onboarding 文案要改。

已知的 macOS 限制（已寫進 README）：

- **權限歸屬是「負責的 process」**：dev 模式下要授權給啟動 `npm run tauri dev` 的終端機/IDE，不是 `sumi` binary 本身；打包後才是授權 `Sumi.app`。這對開發者體感最容易踩雷。
- 本 spike 用 `AXIsProcessTrusted()`（不帶 prompt），由 App 內按鈕開啟「系統設定 → 隱私權與安全性 → 輔助使用」。App 可能不會自動出現在清單，需使用者按「+」手動加入——P0 onboarding 可改用 `AXIsProcessTrustedWithOptions(prompt=true)` 讓系統自動列入並彈窗，但需要建 CFDictionary（多一點 FFI 或引入 `core-foundation` crate），列為 P0 待辦。
- 重編譯後 TCC 可能認不得舊授權（以路徑/簽章識別），需移除重加。

實測 log 確認：無權限時 App 正常啟動並停在 `waiting for Accessibility permission`，不會崩潰或靜默失敗。

### 4. 選了哪些 crate、為什麼？

| Crate | 版本 | 理由 |
|---|---|---|
| `tauri` / `tauri-build` | 2.x | 技術選型已鎖定 |
| `serde` / `serde_json` | 1 | Tauri 必要依賴、事件 payload 序列化 |
| `rdev` | 0.5 | CGEventTap 封裝，最快驗證雙擊偵測；已知維護風險（見問題 1），spike 目的之一就是評估它 |
| `arboard` | 3 | 純 Rust 剪貼簿讀取，比 Tauri plugin 少一層 IPC；對圖片/空值回 `Err`，正好用於過濾 |
| `log` + `env_logger` | 0.4 / 0.11 | 遵守「不用 println! 當正式 log」慣例 |

**沒有**為 Accessibility 檢查加 crate：直接 `extern "C"` 連結 ApplicationServices 的 `AXIsProcessTrusted`。

**相依性事故記錄**：`time` 0.3.48（tauri-utils → plist 的傳遞依賴）與 rustc 1.96 產生 E0119 coherence 衝突（其內部 `ModifierValue` trait 的 impl 打壞下游 blanket impl），導致 `cookie`、`tauri-utils` 編不過。已在 `Cargo.lock` 釘到 0.3.47 解決。**升級依賴時注意不要把 time 拉回 0.3.48**；等上游修復版（0.3.49+）出來再解除。

## 驗收狀態

| # | 驗收項 | 狀態 |
|---|---|---|
| 1 | 雙擊 ⌘C → 視窗顯示複製文字 | 程式碼完成，**待你授權後人工驗證** |
| 2 | 單擊不觸發、不影響正常複製 | 邏輯有單測；監聽為 listen-only 不攔截事件；待人工驗證 |
| 3 | 圖片/空剪貼簿優雅 no-op | 完成（`get_text()` Err / 空白皆不開窗），待人工驗證 |
| 4 | 無權限時走引導流程 | 完成並經 log 驗證（onboarding 畫面 + 一鍵開設定 + 自動輪詢） |
| 5 | 時間窗為可調常數 | 完成（`DOUBLE_PRESS_WINDOW_MS = 300`，`monitor/double_press.rs`） |
| 6 | 無 secret、log 不含剪貼簿內容 | 完成（log 只記字元數；`.gitignore` 含 `.env`/金鑰） |

## 給下一步的建議

1. 你先跑 `npm run tauri dev` 完成人工驗收（特別是授權後不重啟是否生效、雙擊體感）。
2. 若 rdev 實測穩定 → P0 沿用；不穩 → 換手寫 CGEventTap（介面已隔離在 `monitor/`，替換成本低）。
3. P0 開工前的小決策：是否改用 `AXIsProcessTrustedWithOptions(prompt=true)` 改善 onboarding（需引入 CFDictionary FFI 或 `core-foundation` crate，依紅線需先核准）。
