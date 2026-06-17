# Sumi — macOS 翻譯工具

macOS 選單列常駐翻譯工具：複製文字 → 雙擊 `⌘C`，譯文浮窗（Glance）出現在游標附近、不打斷工作；需要逐字查詞、編輯重翻時，展開成可編輯的雙語工作台（Workbench）。平常隱形、沒有 Dock 圖示，只在右上角選單列留一個 icon。

## 目前功能

- **Glance（快翻浮窗）**：雙擊 `⌘C` → 過濾（機密/檔案/純 URL 跳過）→ 自動偵測來源語言 → MT 翻譯 → non-activating 浮窗，不搶前景焦點。
- **語言配對**：設「我的語言 ⇄ 對照語言」（預設 繁中 ⇄ English），依偵測到的來源**自動決定翻譯方向**，免反轉鈕；也可改回「固定目標語言」。
- **Workbench（可編輯工作台）**：Glance 按「展開」或 `⌘↩` 進入；原文可編輯、debounce 即時重翻；**點單字查英漢字典**。
- **英漢字典（ECDICT，本地離線）**：點字顯示音標／詞性／**繁體中文（台灣用詞）**釋義；支援**詞形還原**（`wakes`/`woke` → `wake`）；同一視窗內**查過快取秒回**；查無才退回單一 AI 字義（Gemini，明確標示）。
- **空白 ⌘CC → 空白 Workbench**：沒有新複製時雙擊 `⌘C`，直接開一個空白工作台讓你自己打字翻譯。
- **選單列常駐**：隱藏 Dock 圖示、不進 ⌘Tab；選單列 icon → 設定 / 版本 / 結束。
- **隱私**：字典查詢全本地；疑似機密內容永不送出；API key 只存 macOS Keychain。

## 文件

- 決策記錄：[docs/decisions.md](docs/decisions.md)
- 缺陷清單：[docs/issues.md](docs/issues.md) · 點子池：[docs/backlog.md](docs/backlog.md)
- Spike 報告：[docs/spike-01.md](docs/spike-01.md)（觸發/權限/剪貼簿）· [docs/spike-system-dictionary.md](docs/spike-system-dictionary.md)（為何不走 macOS 系統辭典、改用 ECDICT）
- 產品/設計：[docs/PRD.md](docs/PRD.md) · [docs/ui-spec.md](docs/ui-spec.md)

## 開發環境需求

- macOS（本專案 macOS only）
- [Rust](https://rustup.rs/)（stable，經 rustup 安裝）
- Node.js 18+ 與 npm
- Xcode Command Line Tools
- Python 3 + OpenCC（建置字典用）：`pip3 install opencc`

## 啟動

```bash
npm install
npm run build:dict   # ★ 產生 Workbench 英漢字典（首次必跑；下載 ECDICT + 簡轉繁，約 1 分鐘）
npm run tauri dev
```

> `npm run build:dict` 會產生 `src-tauri/resources/ecdict.sqlite`（不進 git）。**`npm run tauri dev` / `build` 前需先跑過一次**，否則 Workbench 點字字典上段會查無（退回 Gemini 補充）。換 ECDICT 版本只改 `scripts/build-dict.py` 頂部的 pin。

## 首次設定

首次啟動會自動顯示設定視窗做引導（之後平常隱形，從選單列 icon →「設定…」再打開）：

1. **授予「輔助使用」權限**：依 App 內引導操作（見下節注意事項）。
2. **貼上翻譯 API key**：設定 → 翻譯引擎 → Google Cloud Translation API key（或切 DeepL）。深度功能可另貼 **Gemini** key（字典查無時的 AI 字義用；字典本身免 key）。key 只存 **macOS Keychain**，不落地任何檔案。
3. **語言**：預設「語言配對 繁中 ⇄ English」，可改對照語言或切「固定目標語言」。

## Accessibility（輔助使用）權限 — 開發時必讀

雙擊 `⌘C` 偵測使用 CGEventTap 等級的全域鍵盤監聽，macOS 要求授予「輔助使用」權限，**且權限是授予「負責的 process」**：

- **開發模式（`npm run tauri dev`）**：權限要授予*啟動指令的那個 App*——通常是你的終端機（Terminal / iTerm / VS Code 等）。
- **打包後的 App（`npm run tauri build`）**：權限授予 `Sumi.app` 本身。

App 內的「啟用權限」按鈕會跳系統原生授權框並把 App 列入清單；授權後自動開始監聽，**不需重啟**（若 10 秒內沒反應，重啟一次即可）。

> 注意：若你曾在權限清單中看過同名項目但監聽無效，先把舊項目移除再重新加入（macOS TCC 以 binary 路徑/簽章識別，重編譯後可能失效）。

> **正式版（`tauri build`）尚未做 Developer ID 簽章**，目前是 ad-hoc 簽章——cdhash 每次打包都變，所以「輔助使用」授權**重打包後會失效**（清單顯示開著卻沒用）。解法：到輔助使用清單把「Sumi」**整條移除（−）再重新授權**（切換開關沒用）。授權頁有「重新檢查」鈕與疑難排解引導。根治需 Apple Developer ID 簽章＋公證。見 docs/issues.md #13。

### Keychain 密碼提示（dev 專屬）

開發模式下，每次啟動後第一次翻譯會跳一兩次 Keychain 密碼框。原因：`npm run tauri dev` 產生的是未簽章、且每次重編都不同的 binary，Keychain 不認得它，讀取已存的 API key 時就會要你授權（keyring 底層查找＋讀取各跳一次，故可能兩次）。程式只在每次啟動的第一次翻譯讀 Keychain，之後走記憶體快取、不再跳。
**打包成簽章＋公證過的 `.app` 後此提示消失**（簽章身分固定，按一次「永遠允許」即可）。

## 使用方式

**快翻（Glance）**
1. 在任何 App 反白文字，快速按兩次 `⌘C`（預設 300ms 內，可調）。
2. 譯文浮窗出現在游標附近：原文（弱化）＋ 譯文（主角）。浮窗**不搶 focus**，底下的 App 可以繼續打字。
3. 關閉：`Esc`、點浮窗外任意處，或閒置約 6 秒自動淡出（可調）。

不會觸發的情況（設計如此）：單擊 `⌘C`、複製檔案、複製圖片、整段是純網址或檔案路徑。
**疑似機密內容**（密碼、API key、token、JWT 等樣式）永不送出，浮窗會顯示「已略過可能的機密內容」。

**展開到 Workbench**
- 在 Glance 按「展開」或 `⌘↩` → 開啟可編輯工作台，帶入原文/譯文。
- 原文可編輯，停頓約 400ms 自動重翻；**點任一英文單字**跳出英漢字典卡（音標/詞性/繁中釋義，查無才用 AI 字義）。
- `Esc` 關閉。

**空白 ⌘CC（主動輸入）**
- 沒有複製新東西時雙擊 `⌘C` → 直接開**空白 Workbench**，自己打字翻譯（不會帶出上次剪貼簿內容）。

**選單列**
- 平常無 Dock 圖示；右上角選單列 icon → 設定… / 版本 / 結束 Sumi。

## 測試

```bash
cd src-tauri && cargo test --lib   # 過濾規則、雙擊判定、provider 解析等純邏輯單元測試
```

OS 層監聽（CGEventTap）與浮窗行為依專案慣例採手動整合測試。

## 隱私紅線（已落實於程式碼）

- API key 只存 macOS Keychain；不回傳前端、不進 log、不進檔案。
- 疑似密碼/token/key 的剪貼簿內容**不送任何外部 API**。
- 任何 log 都不含剪貼簿內容（只記字元數與耗時）。
- `.env` 與金鑰檔已列入 `.gitignore`。
