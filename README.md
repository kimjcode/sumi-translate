# Sumi — macOS 翻譯工具

macOS 常駐翻譯工具：複製文字 → 雙擊 `⌘C` 跳出翻譯浮窗（Glance），可展開成可編輯的雙語工作台（Workbench）。

> 目前進度：**Spike 01** — 驗證「雙擊 ⌘C 偵測 + Accessibility 權限流程 + 讀剪貼簿」。
> 尚無任何翻譯功能。詳見 [docs/spike-01.md](docs/spike-01.md)。

## 開發環境需求

- macOS（本專案 macOS only）
- [Rust](https://rustup.rs/)（stable，經 rustup 安裝）
- Node.js 18+ 與 npm
- Xcode Command Line Tools

## 啟動

```bash
npm install
npm run tauri dev
```

## Accessibility（輔助使用）權限 — 開發時必讀

雙擊 `⌘C` 偵測使用 CGEventTap 等級的全域鍵盤監聽，macOS 要求授予「輔助使用」權限，**且權限是授予「負責的 process」**：

- **開發模式（`npm run tauri dev`）**：權限要授予*啟動指令的那個 App*——通常是你的終端機（Terminal / iTerm / VS Code 等）。把它加進「系統設定 → 隱私權與安全性 → 輔助使用」並開啟。
- **打包後的 App（`npm run tauri build`）**：權限授予 `Sumi.app` 本身。若清單中沒有，按「+」手動加入。

App 啟動時會偵測權限狀態：未授權會顯示引導畫面（含一鍵開啟系統設定的按鈕），後端每秒輪詢，授權完成後自動開始監聽，**不需重啟**（若實測發現監聽未生效，重啟 App 一次即可；見 spike 報告）。

> 注意：若你曾在權限清單中看過同名項目但監聽無效，先把舊項目移除再重新加入（macOS TCC 以 binary 路徑/簽章識別，重編譯後可能失效）。

## 使用方式（Spike 階段）

1. 啟動後視窗顯示「已就緒」。
2. 在任何 App 反白文字，於 300ms 內按兩次 `⌘C`。
3. Sumi 視窗跳出並顯示剛複製的文字。`Esc` 隱藏視窗。

只按一次 `⌘C` 不會觸發；剪貼簿是圖片或空值時不會跳窗。

## 測試

```bash
cd src-tauri && cargo test --lib   # 雙擊判定等純邏輯單元測試
```

OS 層監聽（CGEventTap）依專案慣例採手動整合測試。

## 隱私紅線（已落實於程式碼）

- 任何 log 都不含剪貼簿內容（只記字元數）。
- 無任何 API key / secret；`.env` 與金鑰檔已列入 `.gitignore`。
