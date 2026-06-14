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
