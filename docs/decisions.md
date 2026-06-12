# 決策記錄（decisions）

> 已拍板的產品/技術決策。改這裡之前先取得共識；PRD/CLAUDE.md 沒寫清楚的，以本檔為準。

## D1 — MT provider：Google 預設、DeepL 可切換（2026-06-12）

- `providers/` 以統一 trait（`MtTranslator`）抽象，Google Cloud Translation v2 與 DeepL 各一個 adapter。
- **Google 為預設**：付費 API 不用使用者內容訓練。
- 切換到 DeepL 時，設定頁**當下顯示提醒**：DeepL 免費層（key 以 `:fx` 結尾）可能用送出的文字改善服務，付費 Pro 才預設不訓練。
- 來源語言用 provider 內建 auto-detect，不另裝語言偵測庫。
- 目標語言預設繁中（台灣，`zh-TW`），可在設定改。DeepL 語言碼映射：`zh-TW→ZH-HANT`、`zh-CN→ZH-HANS`、`en→EN-US`。
- API key 一律存 macOS Keychain（service `com.kimj.sumi`），key 不回傳前端、不進 log、不進檔案。
- MT 是一次回傳整段，**不做假串流**；等待狀態用朱色筆鋒 loading。真串流等 LLM（P1）。

## D2 — 權限流程：先自己說明，再跳原生框（2026-06-12）

1. 首次啟動顯示 Sumi 自己的說明（為什麼需要、不記錄輸入內容）。
2. 使用者按「啟用權限」才呼叫 `AXIsProcessTrustedWithOptions(prompt=true)`——原生框一個 session 只跳一次，所以不在啟動時自動呼叫。
3. 另留「打開系統設定」手動按鈕（給關掉原生框的人）。
4. 前後端輪詢 trust 狀態，授權後自動開始監聽（event tap 在權限就緒後才建立，免重啟）。
5. 防呆文案：「若 10 秒內沒反應，重新啟動 Sumi 一次」。

## D3 — 過濾層規則：機密寧可錯殺，其餘寧可放行（2026-06-12）

判定順序（`monitor/filter.rs`，全部有單元測試）：

1. **複製檔案**（NSPasteboard 含 `public.file-url`）→ 靜默 no-op。
2. 空白 → no-op。
3. **整段是純 URL / 檔案路徑** → 靜默不翻（比機密先判，兩者都不送出所以順序不影響安全）。
4. **機密** → 永不送出，浮窗顯示「已略過可能的機密內容」（不顯示原文、不進 log）：
   - 含 `PRIVATE KEY-----` / `-----BEGIN PGP` 區塊（唯一的多行殺規則）。
   - 單行 `password/secret/token/api_key = value` 賦值。
   - 整段為單一 token 且符合：已知前綴（`sk-`、`ghp_`、`AKIA…`、`AIza`、`xoxb-` 等）／JWT／≥32 碼 hex／≥40 碼 base64／通用密碼樣式（含數字+符號+字母）。
5. **log / 程式碼照翻**（核心情境）；多行內容除規則 4-a 外一律放行。
6. 超過 2000 字 → 只翻前 2000 字並顯示「（已截斷）」。
7. 與上次相同內容 → 不重打 API（快取直接回放，浮窗照樣出現）。

## D4 — Glance 浮窗：真 NSPanel，不吃 git dependency（2026-06-12）

- 用 objc2-app-kit 自寫 glue：Tauri 視窗建立後以 `object_setClass` 換成自訂 NSPanel 子類（`canBecomeKeyWindow=false`）+ `nonactivatingPanel` style mask。**不用 tauri-nspanel**（僅 git dep，供應鏈風險）。
- 顯示走 `orderFrontRegardless`、隱藏走 `orderOut`，全程不搶 focus，點擊浮窗也不會啟用 App。
- 浮窗永遠拿不到鍵盤焦點 → `Esc`、點浮窗外關閉都由**全域 event tap** 處理（tap 同時訂閱滑鼠按下事件）。
- 關閉條件：`Esc`／點浮窗外／閒置（預設 6s，可調）。
- 透明圓角需要 Tauri `macos-private-api` feature（官方 feature flag，非第三方）。

## 歷史決策（spike-01）

- 全域監聽：手寫 CGEventTap（core-graphics），**不用 rdev**（macOS 26 上一按鍵就 crash，見 docs/spike-01.md）。
- 雙擊判定需兩次按下之間有 release + 排除 autorepeat。
- `time` crate 釘 0.3.47（0.3.48 與 rustc 1.96 coherence 衝突）。
