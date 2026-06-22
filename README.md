# Sumi（墨）

> 複製任何文字 → 立刻看懂 → 想深入就展開細修。
> 專為 macOS 開發，不打斷工作流的雙語工作台。

**Sumi** 是一個常駐在 macOS 選單列的翻譯工具。在任何 App 反白文字、快速按兩下 `⌘C`，譯文浮窗就出現在游標旁邊——**不搶你的焦點、不打斷你正在打的字**。瞄一眼就懂的時候它快閃即關；需要逐字查詞、改原文重翻的時候，一鍵展開成可編輯的雙語工作台。

平常它是隱形的：沒有 Dock 圖示、不進 `⌘Tab`，只在右上角選單列留一枚墨色小印。

<!-- TODO: 放一張 Glance 浮窗的截圖或 GIF（雙擊 ⌘C → 譯文淡入） -->
<!-- 截圖佔位：Glance demo -->

---

每天看英文、日文，你的日常是不是這樣？

- 讀個 Log、刷個技術文件、看個 Jira，遇到不懂的詞，每次都要 ⌘C、切換到瀏覽器、貼進 Google 翻譯、看完再切回原本的 App。一天重複幾十次，專注力早就被切碎了。
- 市面上的翻譯外掛很多（Easydict、Pot…），但通常「看完就沒了」。如果你要把客戶的破英文改對、或者要把自己的中文寫法調成道地的英文 Email，那些唯讀的彈窗根本不夠用。

Sumi（墨） 就是為了解決這個硬傷而生的。

---

## 主要功能

### 1. Glance（速查模式）—— 瞄一眼就懂，快閃即關

- 雙擊 ⌘C 即時觸發：第一次 ⌘C 照常複製，300ms 內按第二次才叫出浮窗，絕不跟正常複製打架。
- 不打斷手頭工作：浮窗是 macOS 的 non-activating panel，它浮現時，你的游標依然在原本的編輯器裡，你可以一邊看著譯文，一邊繼續打你的 Code 或 Email。
- 極速淡出：按 Esc、點浮窗外面、或者放著不管 6 秒，它就會自己優雅地淡出。

### 2. Workbench（工作台模式）—— 邊讀邊改的雙語校對桌

- 在 Glance 浮窗按 ⌘↩（或點展開），就會拉開成雙欄工作台。
- 會自動重翻的原文框：原文是可以編輯的！你改動原文，停頓 400ms 就會自動幫你重翻。非常適合用來反覆微調你想寫的英文 Email 或 PR 敘述。
- 點字查字典：點擊譯文或原文裡的任何英文單字，立刻跳出本地字典卡。
- 空白畫布：如果沒有複製任何東西，直接雙擊 ⌘C，會直接開一個乾淨的 Workbench 讓你當成臨時的翻譯沙盒。

### 那些為工程師與重度使用者設計的小細節

- 智慧語言配對，免手動切換
  設定好「我的語言 ⇄ 對照語言」（例如 繁中 ⇄ English），Sumi 會自己偵測來源判定翻譯方向。讀英文直接出繁中；寫中文直接變英文，再也不用去點那個反轉按鈕。
- 全本地、零外送的英漢字典（ECDICT）
  點字查詢使用的是打包在本機的 SQLite 字典，支援詞形還原（wakes、woke 都查得到 wake），全程離線、秒回、絕不外流隱私。只有字典真的查不到的字，才會標示「AI 推測」並走 Gemini 補位。
- 日式水墨美學
  墨色字、紙白底、一枚朱印。Loading 的時候不是無聊的藍色轉圈圈，而是一筆朱色的筆鋒揮毫。

<!-- TODO: 放一張 Workbench 的截圖（左原文／右譯文／字典卡） -->
<!-- 截圖佔位：Workbench demo -->

---

## 技術選型

- **框架**：[Tauri](https://tauri.app/)（Rust 後端 + React 前端）。比 Electron 應用輕量許多，適合常駐。
- **平台**：macOS only，不做跨平台抽象層。
- **翻譯引擎**：快翻走 MT（Google Cloud Translation / DeepL）；深度理解與 AI 字義走 Gemini。
- **字典**：[ECDICT](https://github.com/skywind3000/ECDICT) 英漢資料，離線打包成 SQLite。

更完整的「做什麼／為什麼」見 [docs/PRD.md](docs/PRD.md)，已拍板的技術決策見 [docs/decisions.md](docs/decisions.md)。

---

## 安裝與編譯

> ⚠️ 目前**不提供簽章成品**，需要自己 build。整個流程是標準的 Tauri 開發環境。

### 前置需求

- macOS
- [Rust](https://rustup.rs/)（stable，經 rustup 安裝）
- Node.js 18+ 與 npm
- Xcode Command Line Tools
- Python 3 + OpenCC（**只用於建置字典**，不是 App 執行期依賴）：`pip3 install opencc`

### 編譯與啟動

```bash
npm install
npm run build:dict     # ★ 首次必跑！產生 Workbench 英漢字典
npm run tauri dev      # 開發模式啟動
```

> **`npm run build:dict` 第一次一定要跑。** 它會下載 ECDICT、用 OpenCC 簡轉繁，產生 `src-tauri/resources/ecdict.sqlite`（約 1 分鐘，產物不進 git）。**沒跑過的話 Workbench 點字字典會整段查無**（只能退回 Gemini 補充）。換 ECDICT 版本只改 `scripts/build-dict.py` 頂部的 pin。

打包成 `.app`：

```bash
npm run tauri build
```

---

## 首次設定

首次啟動會自動跳出設定視窗做引導（之後平常隱形，從選單列墨印 →「設定…」再打開）：

### 1. 授予「輔助使用」權限

雙擊 `⌘C` 偵測需要 CGEventTap 等級的全域鍵盤監聽，macOS 要求授予**輔助使用（Accessibility）**權限。App 內的「啟用權限」按鈕會跳系統原生授權框並把 App 列入清單，授權後**自動開始監聽、不需重啟**（若 10 秒內沒反應，重啟一次即可）。

> Sumi **不記錄你打的字**，只偵測觸發鍵。

**權限是授予「負責的 process」**，這點很重要：

- **開發模式（`npm run tauri dev`）**：權限要給*啟動指令的那個 App*——通常是你的終端機（Terminal / iTerm / VS Code 等）。
- **打包後的 App**：權限給 `Sumi.app` 本身。

> **未簽章版的已知摩擦**：正式版目前是 ad-hoc 簽章（[刻意暫不做 Developer ID 簽章](docs/decisions.md)，以保留「陌生人第一次安裝」的測試視角）。cdhash 每次重打包都會變，所以「輔助使用」授權**重打包後會失效**（清單顯示開著卻沒作用）。解法：到輔助使用清單把「Sumi」**整條移除（−）再重新授權**（切換開關沒用）。授權頁有「重新檢查」鈕與疑難排解引導。

### 2. 貼上 API key

設定 → 翻譯引擎 → 貼上 **Google Cloud Translation API key**（或切 DeepL）。深度功能可另貼 **Gemini** key（給字典查無時的 AI 字義用；字典本身免 key）。

**key 只存 macOS Keychain，不落地任何檔案。**

> 開發模式下，每次啟動後第一次翻譯可能跳一兩次 Keychain 密碼框——因為 `tauri dev` 的 binary 未簽章、每次重編都不同，Keychain 不認得它。打包成簽章成品後此提示消失。

### 3. 語言

預設「語言配對 繁中 ⇄ English」，可改對照語言或切「固定目標語言」。

---

## 隱私

Sumi 的使用者常會複製 log、內部 Jira，也可能不小心複製到密碼。每次翻譯都把這些丟給第三方 API 是真實風險，所以 Sumi 把隱私當紅線在寫程式。以下這些**已落實於程式碼**：

- **API key 只存 macOS Keychain**——不回傳前端、不進 log、不進任何檔案。
- **疑似機密內容永不送出**：密碼、API key、token、JWT、PEM/PGP 區塊等樣式會被攔下，浮窗顯示「已略過可能的機密內容」，原文不送、不進 log。
- **任何 log 都不含剪貼簿內容**——只記字元數與耗時。
- **字典查詢全本地、零外送**；只有字典查無的字才走 Gemini（需網路，且明確標示）。
- **不永久儲存剪貼簿內容**（本地歷史為選用、預設關閉）。

> 取捨透明：為了能正常翻譯 log／程式碼，**多行內容**除了 PEM/PGP 區塊外採「寧可放行」策略（若整份多行 `.env` 含密碼那行也會一起送 MT）。這是刻意的設計取捨，理由與未來收緊方式記在 [docs/decisions.md](docs/decisions.md)（D3、D11）。

未來路線：支援本地模型（Ollama 等），讓敏感內容全程不出機器。

---

## 開發與測試

```bash
cd src-tauri && cargo test --lib   # 過濾規則、雙擊判定、provider 解析等純邏輯單元測試
```

OS 層監聽（CGEventTap）與浮窗行為依專案慣例採**手動整合測試**（全域鍵盤事件難以穩定地寫單元測試）。

專案文件：

- 產品需求 [docs/PRD.md](docs/PRD.md) · UI 規格 [docs/ui-spec.md](docs/ui-spec.md)
- 設計決策 [docs/decisions.md](docs/decisions.md) · 缺陷清單 [docs/issues.md](docs/issues.md) · 點子池 [docs/backlog.md](docs/backlog.md)
- Spike 報告：[docs/spike-01.md](docs/spike-01.md)（觸發／權限／剪貼簿）· [docs/spike-system-dictionary.md](docs/spike-system-dictionary.md)（為何改用 ECDICT 而非系統辭典）

---

## License

[MIT](LICENSE)。

字典資料來自 [ECDICT](https://github.com/skywind3000/ECDICT)：Sumi 在建置時（`npm run build:dict`）下載 ECDICT 並轉為本地 SQLite，**字典資料不隨原始碼散布**（產物已列入 `.gitignore`），目前也不提供編譯成品。ECDICT 採 MIT 授權（Copyright 2025 Linwei），可自由使用。
