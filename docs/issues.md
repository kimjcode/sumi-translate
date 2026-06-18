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

## 資安修正第一批（feature/security-fixes · 對應 audit-20260618.md）

### 14. Google MT 的 API key 可能被寫進 log（H1 · 紅線）
- **症狀**：Google 路徑發生連線層錯誤（逾時／斷網／DNS／TLS）時，reqwest 錯誤 Display 會帶上完整請求 URL；key 放在 query string（`?key=AIza…`）→ 整把金鑰隨 `log::warn!` 進 log。
- **根因**：Google Translation v2 把 key 放 URL query（`mt.rs:61`），且 `ProviderError::Network` 直接吃 `e.to_string()`，再被 pipeline 的 warn log 輸出。違反紅線「任何 log 不可含 key」。
- **修法**：(b 為主) key 改放 header `X-Goog-Api-Key`，URL 永不含 key；(a) 新增 `providers::redact_secrets`，在所有 `ProviderError::Network` 建構點把 `key=<value>` 遮成 `key=REDACTED`（mt.rs Google/DeepL、llm.rs send/chunk），根治「未來任何 reqwest error 進 log」。
- **狀態**：Fixed（手動驗證：拔網路翻一次，log 不含 key）

### 15. 字典 fallback 繞過機密過濾、且送整個原文框（H2 · 紅線）
- **症狀**：在 Workbench 貼含密碼/token 的設定/log，點任一生字 → 後端不過濾就把該字 + **整個原文框**送 Gemini。
- **根因**：`gemini_define` 入口沒有 `filter::classify`（重翻路徑有、字典路徑漏了）；前端 `sentence = ta.value` 送整段。
- **修法**：`gemini_define` 進入點先 `filter::classify(&sentence)`，命中 `Secret` → 回「已略過可能的機密內容」、不送出；前端改送「該字所在句」（`sentenceAtCaret`，以句界＋換行切，讓機密那行被獨立後過濾才命中）。
- **狀態**：Fixed（手動驗證：含密碼樣式點字不送、顯示已略過；正常文字只送該句）

### 16. event tap 被系統停用後無聲失效（M1）
- **症狀**：callback 太慢或使用者操作讓系統停用 tap 後，雙擊 ⌘C 靜默死掉直到重啟 App。
- **根因**：`TapDisabledByTimeout | TapDisabledByUserInput` 只 `log::warn!`，未 re-enable。`with_enabled` 不把 tap 交給 callback，拿不到 mach port 重啟。
- **修法**：改手動建 tap（`new_unchecked`）保留 `CFMachPort`；收到 disabled 事件就 `CGEventTapEnable(port, true)` 就地重啟（自綁公開 C 函式，不新增 crate）。
- **狀態**：Fixed（這兩種 disable 重啟可靠；未另做 tray 通知，若日後 re-enable 仍失效再補降級提示）
- **註（曾誤判）**：「輸入 Gemini/MT key 密碼後 Workbench 視窗跳掉」一度被懷疑是此 re-enable 造成，後查證**未簽章正式版（無本批改動）同樣會跳**，且「跳 ⟺ 密碼框出現、key 快取後不跳」→ 是 Keychain 密碼框（ad-hoc 簽章 binary）的既有焦點行為，與 M1 無關，同 #13／D10 成因。

### 17. workbench_translate 無請求序號 → 連續編輯可能顯示舊譯文（M2）
- **症狀**：原文 debounce 後每次獨立 await，網路慢時較早送出、較晚回的請求會用過時譯文蓋掉新的。
- **根因**：`run_mt` 無序號/取消（對照 Glance 的 `request_seq`）。
- **修法**：`WorkbenchState` 加 `mt_seq: AtomicU64`；`workbench_translate` 領序號、await 回來非最新就回新增的 `WbTranslation::Stale`，前端忽略不回填。
- **狀態**：Fixed

### 18. Gemini 串流無 idle timeout → 串到一半 stall 會無限等（M3）
- **症狀**：首 token 後連線 stall，AI 字義永遠停在「串流中」，不 done 不 error。
- **根因**：`llm_client` 只設 `connect_timeout`，刻意不設整體 timeout（對），但也沒有任何 read/idle timeout。
- **修法**：`llm_client` 加 `read_timeout(20s)`（reqwest 0.13 內建，每次讀重置）；保留整體不限時，stall 逾時轉 `Network` 錯誤。
- **狀態**：Fixed

## 已知取捨與排查線索（非缺陷 · 對應 audit L2/L3）

> 這節記「已知、目前可接受、不打算改」的點，避免日後被當新 bug 重查。
> 與上方缺陷分開：這裡沒有「要修」的動作，只留線索。

### L2. 過濾層「寧可錯殺」的已知誤判樣本（符合 D3，不改程式）
- **位置**：`monitor/filter.rs:107-130`（`is_long_hex` / `is_password_like`）。
- **現象**：以下正常內容會被判為 `Secret` 而**靜默不翻**（浮窗顯示「已略過可能的機密內容」）：
  - 純 40 碼 SHA-1（如 git commit hash）→ 命中 `is_long_hex`（≥32 碼 hex）。
  - 含連字號的 UUID（如 `550e8400-e29b-41d4-a716-446655440000`）→ 同時含數字＋符號＋字母，命中 `is_password_like` 通用密碼樣式。
- **取捨**：這是 [D3](decisions.md)「機密寧可錯殺、其餘寧可放行」的**刻意取捨**，不是 bug。誤殺的代價是「該段不翻」，遠小於漏放真機密。
- **狀態**：Accepted（不改）。若日後有人回報「commit hash／UUID 不翻」，先對照本筆，別當新 bug 重查根因。

### L3. `change_count()` 在 event-tap 背景執行緒讀 NSPasteboard（低風險）
- **位置**：`monitor/pasteboard.rs:18-20`（讀 `changeCount`）＋ `monitor/mod.rs:128`（在 event-tap callback 執行緒呼叫）。
- **現象**：`changeCount()` 於 CGEventTap 的背景執行緒讀取 `NSPasteboard.generalPasteboard`。NSPasteboard **並非保證 thread-safe**；clipboard manager 慣例上會背景輪詢 `changeCount`，故目前評估低風險、可接受，未改。
- **狀態**：Accepted（不改）。**排查線索**：若日後出現詭異的 `changeCount` 行為（D8 的「有/無新複製」分流誤判、讀到舊值、偶發 crash），把「背景執行緒讀 NSPasteboard」列為第一個懷疑點，考慮改到主執行緒讀或加同步。
