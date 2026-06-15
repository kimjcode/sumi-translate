# 決策記錄（decisions）

> 已拍板的產品/技術決策。改這裡之前先取得共識；PRD/CLAUDE.md 沒寫清楚的，以本檔為準。

## D6 — 語言配對模式（2026-06-15）

讓目標語言依偵測到的來源自動決定，免反轉鈕、一個設定通吃雙向。邏輯放在語言/路由層（`router.rs`），Glance 與 Workbench 共用。

- **兩種模式（互斥）**：`固定目標語言`（舊行為保留）、`語言配對`（新增）。
- **預設**：**語言配對**，`我的語言 = 繁中(zh-TW)`、`對照語言 = English(en)`（主要使用者＝台灣工程師）。既有 settings.json 缺欄位時，serde default 自動補成此預設。
- **命名**：UI 用「我的語言 ⇄ 對照語言」，**不要**用「翻譯語言/目標語言」這類會混淆來源/目標的詞。
- **路由規則**（我的語言 A、對照語言 B）：來源=B → A；來源=A → B；其他外語 → A；偵測不到 → A。**只有 B 享雙向，其餘外語一律 fallback 到 A。**
- **偵測方式＝方案 A**：先翻成 A，若偵測出來源就是 A（基底語言相同）才回頭翻成 B。Google/DeepL 共用、用 provider 內建偵測、最常見情境（讀外語）只 1 次呼叫；只有「寫我的語言→對照語言」方向會多 1 次 MT 呼叫。**不需新 crate。** 基底語言比對（zh-TW/zh-CN/zh 視為同語言）；A、B 基底相同（如 繁中⇄簡中）無法靠偵測分辨方向 → 不切換。
- **Workbench 顯示**：配對模式下 toolbar 顯示解析後的目標（唯讀「→ 繁中/EN」），不給固定下拉；固定模式維持可選下拉。
- 快取鍵改用「路由簽章」（模式 + 語言）而非單一 target，避免不同方向互相污染。

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

### D4 補充 — App 形態與「搶焦點」的釐清（2026-06-13）

- **Glance 唯讀是設計，不是 bug**：浮窗不能在裡面打字；「在浮窗內編輯原文＋即時重翻」是 Workbench（P1）的事。曾誤把「無法在浮窗內打字」當成搶焦點 bug。
- 真正要驗的是「浮窗出現時，底下原本的 App 能不能繼續打字」——non-activating panel 已滿足，**毋需把 App 設成 accessory**。
- 一度為了解這個（不存在的）問題把 App 改成 accessory（背景代理），結果：① 沒 Dock 圖示 ② 設定視窗關掉後要重啟才能再開（還沒做選單列圖示）。**P0 決定維持一般 App**（有 Dock 圖示、設定隨時可開）。
- accessory／選單列常駐形態延後到有選單列圖示時一起做（呼應 PRD「常駐工具」終態）。
- 教訓：表象（閃退、狂跳密碼）當時其實是 ResizeObserver 凍結視窗高度 + Keychain 每次讀都跳密碼兩個獨立 bug，不是 accessory 造成；別被表象帶著改架構。

## D5 — Workbench（P1）：字典來源、Gemini 串流、focus（2026-06-14）

- **字典資料源（第一段「真字典」）**：選 **Free Dictionary API（dictionaryapi.dev）**。免 key、提供音標／詞性／英文釋義；查無此字回 404 → 視為 `None`（正常情況非錯誤）。英漢釋義（有道／mdict）延後。中文語境交給第二段 Gemini。
- **Gemini（LLM）**：model `gemini-2.0-flash`（常數，易換版）。key 存 Keychain，account `gemini_api_key`，與 MT 的 key 分開管理（不塞進 MT 的 `Provider` enum）。
- **真 token 串流**：用 reqwest 的 `chunk()` 逐塊讀 SSE（`?alt=sse`），以 **bytes 緩衝、只解碼整行**避免在 chunk 邊界切斷 CJK 多位元組字元。token 經 `workbench://llm-*` 事件串給前端，套朱色筆鋒游標。**不需新 crate**（`chunk()` 不在 `stream` feature 後面）。
- **focus 行為**：Workbench 是**一般視窗**（`show()` + `set_focus()` 拿鍵盤焦點），與 Glance 的 non-activating panel 相反。沿用 P0「維持一般 App」的結論，不動 accessory。
- **過濾層沿用**：重翻路徑一樣過 `filter::classify`；機密內容仍 `Secret` 不送出（紅線）。Workbench 是主動編輯，URL/路徑照翻（仍經機密過濾）。
- **字典 ≠ LLM 的落實**：字典卡兩段，上段純字典資料（不經 LLM）、下段標明 Gemini，視覺與資料來源都分開。
- **P1 全程未新增任何 crate**：字典 + Gemini + 串流全用既有 reqwest + Tauri events。

## 歷史決策（spike-01）

- 全域監聽：手寫 CGEventTap（core-graphics），**不用 rdev**（macOS 26 上一按鍵就 crash，見 docs/spike-01.md）。
- 雙擊判定需兩次按下之間有 release + 排除 autorepeat。
- `time` crate 釘 0.3.47（0.3.48 與 rustc 1.96 coherence 衝突）。
