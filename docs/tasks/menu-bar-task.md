# 選單列常駐 + 隱藏 Dock（貼給 Claude Code 的任務）

> 從 master 開分支 `feature/menu-bar`。
> 開始前讀 `CLAUDE.md`、`docs/decisions.md`、`docs/issues.md`（**特別是 #4 那次被 accessory 表象帶歪的教訓**）。

## 目標

把 Sumi 變成 **macOS menu bar app（選單列常駐程式）**：平常隱形、沒有 Dock 圖示、沒有主視窗，右上角選單列一個 icon，點開有選單。符合「平常隱形、雙擊 ⌘C 才出現」的常駐工具定位。

## 要做的事

1. **隱藏 Dock 圖示**：app 設為 macOS accessory 模式（`NSApplicationActivationPolicy.accessory`）——Dock 不顯示、⌘Tab 不出現，純背景常駐。
2. **加選單列 tray icon**：用 Tauri 內建 tray 支援。icon + 一個選單，至少含：
   - 設定（開啟設定視窗）
   - 關於 / 版本
   - 結束 Sumi
   - （可選）一個狀態列，顯示目前是否已取得輔助使用權限
3. icon 採用單色 template image，能隨 macOS 深/淺色選單列自動適應（不要彩色寫死）。

## ⚠️ 最大風險：accessory 模式可能波及 focus 行為（先隔離再做）

P0 的 issue #4 教訓：曾被 accessory 表象帶去亂改架構、繞遠路。這次是**真的**要改 activation policy，所以**改之前先確認、改之後逐項驗**這兩個既有行為有沒有被影響：

- **Glance 浮窗**：仍是 non-activating、不搶前景 App 的鍵盤焦點？
- **Workbench**：展開後仍能正常拿到鍵盤焦點、可編輯？

accessory 模式會改變 app 跟系統的 activation 關係，這兩個視窗的 focus 邏輯是辛苦調對的，必須確認沒被破壞。**若發現 accessory 與某個 focus 行為衝突，先停下來回報，不要硬改視窗架構去硬湊。**

## 技術約束

- 沿用既有視窗/權限邏輯，不重寫。
- 新增 crate 前先列出、說明、等我確認（tray 應該用 Tauri 內建、未必需要新 crate）。
- 紅線照舊：不 log 內容、key 只進 Keychain、secret 一律 gitignore。修 bug 記 `docs/issues.md`。

## 驗收標準（每條可手動驗證）

1. 啟動後 **Dock 沒有 Sumi 圖示**、⌘Tab 切換器裡也沒有。
2. 選單列右上角有 Sumi 的 icon，深/淺色選單列下都看得清楚。
3. 點 icon 跳出選單；「設定」開得了設定視窗、「結束」能正常退出。
4. **雙擊 ⌘C 翻譯、Glance 浮窗照常出現且不搶焦點**（底下 App 仍可打字）。
5. **Workbench 展開後仍正常拿 focus、可編輯**。
6. 權限流程（AXIsProcessTrustedWithOptions）仍正常；首次啟動引導不受影響。
7. repo 無 secret、無內容被 log。

## 交付

- menu bar 常駐版（隱藏 Dock + tray 選單）。
- `docs/decisions.md` 記下：改用 accessory 模式、tray 選單項目。
- `docs/issues.md` 記下過程中若有 focus 相關的坑。
- 簡短回報：accessory 對 Glance / Workbench focus 有沒有影響、怎麼處理的。

## 完成後

停下來給我看結果。特別回報 focus 那兩條有沒有被波及——這是這張卡唯一的真風險。