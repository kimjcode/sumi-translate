# 決策記錄（decisions）

> 已拍板的產品/技術決策。改這裡之前先取得共識；PRD/CLAUDE.md 沒寫清楚的，以本檔為準。

## D10 — 暫不做 Developer ID 簽章（保留陌生環境第一體驗視角）（2026-06-17）

正式版（`tauri build`）目前維持 **ad-hoc 簽章**，**刻意暫不接 Developer ID 簽章 + 公證**。

- **已知代價（接受）**：cdhash 每次重打包都變 → 輔助使用授權重打包後失效（[issues #13](issues.md)）、dev 的 Keychain 反覆跳密碼。
- **理由**：**保留「陌生環境第一次使用」的測試視角**——未簽章版每次重打包都像新使用者第一次安裝，能持續在真實的「卡住」情境下驗證 onboarding（權限引導、重新檢查、疑難排解）夠不夠順。簽章後這個摩擦會消失，反而看不到第一體驗的痛點。
- **緩解**：onboarding 已加「重新檢查」鈕 + 「整條移除再重授」疑難排解（issues #13）。
- **何時做**：要對外發布／給非開發者使用時，再接 Developer ID 簽章 + 公證（一勞永逸，授權跨打包穩定保留）。

## D9 — 選單列常駐 + 隱藏 Dock（accessory）（2026-06-17）

把 Sumi 變成 macOS 選單列常駐程式：平常隱形、無 Dock 圖示、無主視窗，右上角選單列一個 icon。這是 D4 補充裡延後的「accessory／選單列終態」，現在連同 tray 一起做。

- **accessory 模式**：啟動時 `NSApplicationActivationPolicy::Accessory`——不進 Dock、不進 ⌘Tab。
- **focus 處理（D4/#4 教訓：先隔離再驗）**：accessory 下視窗預設拿不到鍵盤焦點，所以：
  - **Glance**：維持 `orderFrontRegardless`、**不** activate App → non-activating、不搶前景焦點（accessory 反而更乾淨）。
  - **Workbench / 設定視窗**：show 前先 `activateIgnoringOtherApps(true)` → 才拿得到鍵盤焦點可編輯。
  - 結論：兩條 focus 行為靠「Glance 不 activate、其餘 activate」區分，未改視窗架構。
- **tray 選單**（Tauri 內建 `tray-icon` + `image-png` feature，非新 crate）：`設定…`（開設定視窗）／停用的版本列 `Sumi vX.Y.Z`（兼關於）／`結束 Sumi`。icon 用單色 template image（`icon_as_template(true)`），隨深/淺色選單列自動配色。
- **主視窗（設定）改 `visible:false`**：平常不彈。首次啟動若**未取得輔助使用權限**才自動顯示做 onboarding；已授權則維持隱形，靠選單列「設定」叫出。沿用「關閉=隱藏不銷毀」+ Dock reopen 保險。
- tray icon 原始檔 `src-tauri/icons/tray.png`（單色 template，44px）。

## D8 — ⌘CC 的完整行為樹：有新複製→Glance / 無新複製→空白 Workbench（2026-06-17）

讓「沒有複製任何新東西時按 ⌘CC」有明確用途：開一個全空 Workbench 自己打字翻譯（順便消除「空白按 ⌘CC 帶出上次剪貼簿內容」的困惑）。

- **行為樹**（⌘CC 觸發時）：
  - **有新複製**（剪貼簿 `changeCount` 比這次手勢開始前升高）→ 照舊翻譯這次複製的內容（Glance）。
  - **無新複製**（`changeCount` 沒變）→ 開**全空** Workbench（原文欄空、游標就緒），不帶任何上次剪貼簿內容。
- **判斷用 `changeCount`、不綁時間**：第一次 ⌘C 按下時記下複製「前」的 `changeCount` 當基準（在 event tap 抓）；雙擊成立後在主執行緒讀當下 `changeCount` 比對（此時第一次複製已落地，較可靠）。十分鐘前複製過、現在空白按 ⌘CC（沒選取、changeCount 沒升高）仍正確判為「無新複製」。
- **空白路徑直接開 Workbench、跳過 Glance**（自己打字需要可編輯、會拿 focus 的視窗）；之後沿用既有 debounce 重翻管線與語言配對設定，與一般 Workbench 完全相同。
- **不改動「有新複製 → Glance」這條既有路徑。**
- **Guardrail（重要）**：**⌘CC 的分流規則到此為止——只有「有新複製 / 無新複製」兩條，別再無限增加分支。** 多一條分流就多一條使用者要記的規則，會讓「直覺操作」退化成「要背規則」。新需求請找別的入口（選單列、設定、Workbench 內動作），不要再往 ⌘CC 疊。

## D7 — Workbench 字典卡：ECDICT 英漢 + 簡化為「只留字典」（2026-06-16）

字典來源從英英（Free Dictionary API）換成英漢（ECDICT 本地 SQLite）。背景見 `docs/spike-system-dictionary.md`（為何不走系統辭典）。

**字典卡簡化（2026-06-16 修訂）**：實際使用後決定**移除下段「逐字 Gemini 文法/語境」整段**，字典卡只剩字典。
- 理由：點單字時字典就夠；逐字 Gemini 語意價值低，且原「上段+下段」在 ECDICT 查無時會對同一字**發兩個 Gemini 請求**，是 503/變慢/內容自相矛盾的根因。
- **命中字典 → 純本地真字典**（音標/詞性/中文），完全不打 Gemini。
- **查無 → 只發一個 Gemini 請求**取「AI 字義」，明確標示「AI 字義 · Gemini（字典未收錄，AI 推測）」，不做假字典框。同時涵蓋罕見技術詞（bootloader）與非英文拉丁詞（西語 sentir）。
- **語言路由**：字典區塊由「ECDICT 命中」決定，**不再用「拉丁字母=英文」猜語言**（避免西/法語誤判）。
- **錯誤處理**：查無只剩單一請求；其 503/429/網路錯誤**自動短退避重試 ≤2 次**（且僅在尚未串出 token 時），仍失敗才給友善繁中訊息，**不露原始英文錯誤**。
- **「整段語法糾正」**（句/段級，非逐字）移至 backlog 之後規劃，不在此卡。

- **資料源／授權**：ECDICT 1.0.28（`skywind3000/ECDICT`），**MIT 授權（Copyright 2025 Linwei），可隨 app 散布**。
- **打包形式**：完整版做成 SQLite 當 app resource 打包（`bundle.resources`），rusqlite 索引查詢、不全載入記憶體。**不做首次下載**（離線精神）。
- **繁中（台灣）轉換**：ECDICT 原始是簡體。**在資料準備階段離線用 OpenCC `s2twp` 轉繁中+台灣用詞**（軟體/記憶體/陣列/滑鼠/非同步…），出貨的 SQLite 已是繁中。app 執行期**不帶 OpenCC**。
  - 已知限制（先接受）：少數冷僻技術術語的兩岸差異 `s2twp` 不一定全中，可能殘留大陸譯法；之後靠 P2「使用者字典/術語庫」覆蓋，本任務不處理。
- **體積取捨**：完整版 3.4M 條（含片語）317MB 過大。**只收「單一英文單字」且有語料頻率/考試詞表/Collins-Oxford 標記者** → 5.8 萬詞、約 **8MB**（含詞形還原表）。對齊前端點單字查詢，多字片語永不會被點到故濾除。長尾罕見詞交給 Gemini fallback。
- **詞形還原（D）**：ECDICT 收原形。用 ECDICT 自帶的 `exchange` 欄（記過去式/分詞/複數/三單/比較級…）反建一張 `lemma(form→原形)` 表，打包在同一個 SQLite。查詢順序：**直接查 → 查無則用 lemma 表還原成原形再查**（wakes/woke → wake）。變化型若本身也是收錄字，direct 先命中不受影響。還原後仍查無 → Gemini fallback。約 5.5 萬筆對照，幾乎零額外成本。
- **查無 fallback**：見上方「字典卡簡化」——單一 AI 字義請求（`workbench://def-*` 事件通道），不開天窗。
- **Session 快取（E）**：lazy + in-memory（前端 Map），**鍵 = 還原後原形 + 語言方向**（故 wakes/waking 命中同一筆）。快取「字典結果 或 單一 AI 字義」；同一 Workbench 內重複點同字秒回、查無時不再打 Gemini。**不預載**（只為真實點擊付費，呼應隱私）。**applyInput（關閉再開）時清空** → 天然的「重新整理」。查無需待 AI 字義結束才算可命中（避免回放半成品）。
  - 查詢層順序：`點字 → 詞形還原 → 用原形查快取 → 命中即回；未命中才查 ECDICT；查無才發單一 AI 字義 → 完成存回快取`。
- **新增 crate**：`rusqlite`（`bundled`，內建 SQLite，不依賴系統庫）。OpenCC 只是建置工具（`pip3 install opencc`），非 app 依賴、非 Rust crate。
- **大檔處理**：產物 `src-tauri/resources/ecdict.sqlite` **不進 git**（`.gitignore`），由 `npm run build:dict`（`scripts/build-dict.py`）下載 pin 住的 ECDICT + 轉繁 + 產生。下載來源/版本 pin 在腳本內。**build / tauri dev 前需先跑 `npm run build:dict`**（README 與 CLAUDE.md 指令區已註明）。
- **隱私**：字典查詢全本地、零外送；只有查無時的單一 AI 字義才走 Gemini（需網路）。

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
