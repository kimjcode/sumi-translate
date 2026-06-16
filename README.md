# Sumi — macOS 翻譯工具

macOS 常駐翻譯工具：複製文字 → 雙擊 `⌘C`，譯文浮窗（Glance）出現在游標附近、不打斷工作。之後可展開成可編輯的雙語工作台（Workbench，開發中）。

> 目前進度：**P0 / Glance 模式** — 雙擊觸發 + 過濾 + 自動偵測來源語言 + MT 翻譯 + non-activating 浮窗。
> 決策記錄見 [docs/decisions.md](docs/decisions.md)，spike 報告見 [docs/spike-01.md](docs/spike-01.md)。

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

## 首次設定（兩步）

1. **授予「輔助使用」權限**：依 App 內引導操作（見下節注意事項）。
2. **貼上翻譯 API key**：設定視窗 → 翻譯引擎 → 貼上 Google Cloud Translation API key（或切到 DeepL 用 DeepL key）。key 只會存進 **macOS Keychain**，不落地任何檔案。

## Accessibility（輔助使用）權限 — 開發時必讀

雙擊 `⌘C` 偵測使用 CGEventTap 等級的全域鍵盤監聽，macOS 要求授予「輔助使用」權限，**且權限是授予「負責的 process」**：

- **開發模式（`npm run tauri dev`）**：權限要授予*啟動指令的那個 App*——通常是你的終端機（Terminal / iTerm / VS Code 等）。
- **打包後的 App（`npm run tauri build`）**：權限授予 `Sumi.app` 本身。

App 內的「啟用權限」按鈕會跳系統原生授權框並把 App 列入清單；授權後自動開始監聽，**不需重啟**（若 10 秒內沒反應，重啟一次即可）。

> 注意：若你曾在權限清單中看過同名項目但監聽無效，先把舊項目移除再重新加入（macOS TCC 以 binary 路徑/簽章識別，重編譯後可能失效）。

### Keychain 密碼提示（dev 專屬）

開發模式下，每次啟動後第一次翻譯會跳一兩次 Keychain 密碼框。原因：`npm run tauri dev` 產生的是未簽章、且每次重編都不同的 binary，Keychain 不認得它，讀取已存的 API key 時就會要你授權（keyring 底層查找＋讀取各跳一次，故可能兩次）。程式只在每次啟動的第一次翻譯讀 Keychain，之後走記憶體快取、不再跳。
**打包成簽章＋公證過的 `.app` 後此提示消失**（簽章身分固定，按一次「永遠允許」即可）。

## 使用方式

1. 在任何 App 反白文字，快速按兩次 `⌘C`（預設 300ms 內，可調）。
2. 譯文浮窗出現在游標附近：原文（弱化）＋ 譯文（主角）。浮窗**不搶 focus**，底下的 App 可以繼續打字。
3. 關閉：`Esc`、點浮窗外任意處，或閒置約 6 秒自動淡出（可調）。

不會觸發的情況（設計如此）：單擊 `⌘C`、複製檔案、複製圖片、整段是純網址或檔案路徑、空剪貼簿。
**疑似機密內容**（密碼、API key、token、JWT 等樣式）永不送出，浮窗會顯示「已略過可能的機密內容」。

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
