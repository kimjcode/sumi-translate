# Issues — 缺陷紀錄

修 bug 時維護這份清單。與 `decisions.md` 分工：
- **issues.md** = 壞了什麼、怎麼修的（缺陷）。
- **decisions.md** = 為什麼這樣設計（決策）。

**格式：每筆四欄，精簡可掃，不要寫成冗長 debug 日記。**
**流程：先寫症狀與懷疑 → 隔離變因確認根因 → 再動手。**（別被表象帶著去改架構。）

---

## P0 / Glance

### 1. 浮窗只顯示英文、沒有譯文
- **症狀**：浮窗正常出現，但只顯示原文、翻譯一直沒回來。
- **根因**：reqwest 0.13 的 `system-proxy` feature 在 macOS 上讓請求靜默卡死。
- **修法**：移除 system-proxy，改直連。
- **狀態**：Fixed

### 2. keyring 不可用
- **症狀**：讀取 Keychain 的 key 時 crate 行為異常。
- **根因**：keyring v4 已變成純範例 crate。
- **修法**：改用 keyring v3。
- **狀態**：Fixed

### 3. 過濾層誤殺正常內容
- **症狀**：檔案路徑 `/Users/.../x.pdf`、連字號詞 `Pre-trained` 被當成機密內容跳過。
- **根因**：通用密碼規則太寬鬆。
- **修法**：URL/路徑先判；密碼樣式須同時含數字＋符號＋字母。
- **狀態**：Fixed

### 4. 閃退 ＋ 狂跳密碼（兩個獨立 bug，非單一表象）
- **症狀**：浮窗閃退、且不斷跳 Keychain 密碼，一度懷疑是「搶焦點 / accessory」。
- **根因**：其實是兩個無關的 bug——(a) ResizeObserver 凍結視窗高度；(b) Keychain 對未簽章的 dev binary 每次讀都跳密碼。與 focus/accessory 無關。
- **修法**：(a) 修 ResizeObserver 凍高度；(b) dev 限定問題、簽章後消失，README 已註記。
- **狀態**：Fixed
- **教訓**：先隔離變因再動架構——被表象帶去改 accessory，繞了一圈才找到真兇。
## P1 / Workbench

### 5. 展開只有第一次成功，第二次失效
- **症狀**：Glance 按「展開」第一次正常開 Workbench，關掉後再展開就沒反應。
- **根因**：用原生紅色關閉鈕關 Workbench 時，Tauri 預設**銷毀**視窗；第二次 `show()` 找不到視窗。
- **修法**：攔截 `CloseRequested` → `prevent_close()` + 隱藏，視窗保留供下次再 show。
- **狀態**：Fixed

### 6. 展開不帶入當下內容、殘留上一次單字卡
- **症狀**：開 Workbench 第一次空白、第二次顯示上一次內容；上次點的單字卡會殘留。
- **根因**：Workbench 視窗啟動時掛載一次、之後只 show/hide，React 不重新掛載，`getWorkbenchInput()` 只在啟動跑一次。
- **修法**：`open_workbench` 每次發 `workbench://input` 事件推當下內容；前端收到就更新內容並清掉殘留單字卡。
- **狀態**：Fixed

### 7. 字典卡 Gemini 回 HTTP 404
- **症狀**：文法/語境段顯示「Gemini 回了錯誤（HTTP 404）」。
- **根因**：model `gemini-2.0-flash` 在現行 API/該 key 上找不到（2.0 應已退役）。
- **修法**：改 `gemini-2.5-flash`；並把 API 錯誤訊息浮上 UI（404 body 會列可用 model），便於再調整。
- **狀態**：Fixed（待實機確認 2.5-flash 可用）

### 8. 句尾無標點時，點左半邊空白會展開最後一個字
- **症狀**：原文句尾沒有標點時，點原文欄空白處會誤開最後一個字的字卡。
- **根因**：textarea 點空白／行尾時游標吸附到文字結尾；句尾是字母就被 wordAtCaret 抓成最後一個字（有標點時結尾非字母才剛好倖免）。
- **修法**：純點擊時要求游標右側是字母（真的點在字上）；點空白/行尾/空格不查。
- **狀態**：Fixed

### 9. Glance 浮窗被 Dock／螢幕邊緣切掉
- **症狀**：游標靠近螢幕下緣（或邊緣）時，浮窗被 Dock 蓋住、顯示不完整；短譯文也一樣。
- **根因**：定位用 `monitor_from_point(cursor)` 取螢幕，但它底層用 CGDisplayBounds（logical 點），而 `cursor_position` 給的是 physical 像素；Retina（2x）上座標對不上 → 回 `None` → 整段夾邊界邏輯被跳過 → 浮窗毫無邊界保護。另外原本用 `monitor.size()`（全螢幕）而非可視工作區。
- **修法**：改自己用 `available_monitors()`（position/size 皆 physical）比對找游標所在螢幕；用其 `work_area()`（已扣 Dock/選單列）夾邊界，近底時往游標上方開。
- **狀態**：Fixed

### 10. Glance 上按 ⌘↩ 沒反應
- **症狀**：Glance 浮窗顯示「展開 ⌘↩」，但按 ⌘↩ 無作用（只能用滑鼠點「展開」鈕）。
- **根因**：Glance 是 non-activating panel、`canBecomeKeyWindow=false`，永遠拿不到鍵盤焦點，前端收不到 keydown（同 Esc 的處境）。當時也根本沒有任何 ⌘↩ handler。
- **修法**：跟 Esc 一樣在全域 event tap 攔截——`KEYCODE_RETURN` + ⌘ flag + Glance 顯示中才觸發，不註冊系統 global-shortcut（只在浮窗顯示時生效，不影響別處的 ⌘↩）。後端在顯示 Result 時記下可展開內容供 tap 取用。
- **狀態**：Fixed

### 11. 字典卡查無時同字發兩個 Gemini 請求 → 503／變慢／內容自相矛盾
- **症狀**：點 ECDICT 查無的字，字典卡上段（AI 字義）與下段（文法/語境）各打一次 Gemini，常見 503、變慢，且上下兩段對同一字說法不一致。
- **根因**：原設計「上段字典 + 下段逐字 Gemini 語意」，查無時上段 fallback 與下段語意是兩個獨立 Gemini 請求。
- **修法**：移除下段逐字 Gemini 整段，字典卡只留字典；查無只發單一 AI 字義請求，並對 503/429/網路錯誤短退避重試 ≤2 次（尚未串出 token 時）。
- **狀態**：Fixed

### 12. 設定視窗關掉後叫不回來（正式版）
- **症狀**：build 出來的正式版，第一次開設定頁、關掉後，再點 App icon 叫不出設定。
- **根因**：兩個都中——(a) main（設定）視窗沒有 close handler，Tauri 預設關閉鈕**整個銷毀**視窗（同 #5）；(b) 點 Dock icon（applicationShouldHandleReopen）沒綁「重新顯示設定」。銷毀後又沒有重開入口 → 叫不回來。
- **修法**：(a) main 視窗 `CloseRequested` → `prevent_close()` + 隱藏（關閉=隱藏不銷毀）；(b) 在 run loop 處理 `RunEvent::Reopen` → 顯示並聚焦 main 視窗。之後做選單列時也能從那裡叫出。
- **狀態**：Fixed

### 13. 正式版輔助使用權限「開著卻沒用」、卡在 onboarding
- **症狀**：`npm run tauri build` 的正式版，系統設定裡輔助使用顯示 Sumi 已開，但 App 仍停在授權頁、`AXIsProcessTrusted()` 回 false；切換開關／重啟無效。
- **根因**：release 是 **ad-hoc 簽章**（`TeamIdentifier not set`、`flags=adhoc,linker-signed`），TCC 對未正式簽章 app 綁 binary 的 **cdhash**；每次重打包 cdhash 變，系統設定那條「Sumi」綁到舊 cdhash，對不上現在的 binary。**只切換開關不會重綁**。
- **修法（B：未簽章現實的緩解，非根治）**：onboarding 加「重新檢查」鈕 + 回到視窗自動重查 + 疑難排解（教使用者**整條移除（−）再重授**，不是切換開關）。根治需 Apple Developer ID 簽章＋公證（cdhash 不再隨打包變動、授權跨打包穩定保留）；**目前刻意暫不做，理由見 [decisions.md D10](decisions.md)**。
- **狀態**：Mitigated（根治＝正式簽章；暫緩屬決策，見 D10）
