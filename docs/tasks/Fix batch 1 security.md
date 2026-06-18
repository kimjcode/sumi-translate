# 修正第一批：資安紅線 + 高價值穩定性（貼給 Claude Code · 新 session）

> 依稽核報告 `docs/audit-20260618.md`。從 master 開分支 `feature/security-fixes`。
> 開始前讀 `CLAUDE.md`、`docs/audit-20260618.md`、`docs/decisions.md`、`docs/issues.md`。
> 這批是「紅線 + 真實會發生 + 修法明確」的集合，一條分支處理完一起驗收。

---

## 🔴 必修（踩到 CLAUDE.md 紅線）

### H1 — Google MT 的 API key 可能被寫進 log
- **位置**：`providers/mt.rs:61`（key 放 URL query：`?key={api_key}`）+ `pipeline.rs:271-273`（把 `ProviderError::Network(msg)` 丟進 `log::warn!`）。
- **問題**：Google 把 key 放 query string；reqwest 的連線層錯誤（逾時/斷網/DNS/TLS）的 Display 會帶上完整 URL（含 `key=AIza…`），這個字串被 log 出來 → key 進 log。斷網是日常事件，遲早會中。違反紅線「任何 log 不可含 key、零容忍」。DeepL/Gemini 走 header 不受影響，只有 Google MT 中招。
- **修法（兩者都做，b 為主）**：
  - (b) 把 Google key 改放 **header `X-Goog-Api-Key: <key>`**（Google Translation v2 支援），URL 永不含 key。
  - (a) log 端對 `Network` 錯誤不要直接輸出原始字串，或先 redact（遮掉任何 `key=...`），根治「未來任何 reqwest error 進 log」的風險。

### H2 — 字典 fallback（`gemini_define`）繞過機密過濾，且送整個原文框
- **位置**：`workbench.rs:242-316`（`gemini_define` 直接 `stream_generate`，沒有 `filter::classify`）+ `Workbench.tsx:196,237`（送的是 `ta.value` 整個原文框，不是該字所在句）。
- **問題**：點到 ECDICT 查無的字時，後端不過濾就把該字 + **整個原文框**送 Gemini。情境：在 blank Workbench（D8）或編輯框貼進含密碼/token 的設定/log，點任何生字 → 含機密的全文外送 Gemini。重翻路徑（`workbench_translate`）有過濾，但字典路徑漏了。違反紅線「機密內容不送外部（含字典 fallback）」。
- **修法**：
  - `gemini_define` 進入點先 `filter::classify`（或 `looks_like_secret`），命中 `Secret` → 回「已略過可能的機密內容」、不送出。
  - 只送**該字所在句**，而非整個 textarea（降低外送量，符合隱私 §9）。

## 🟡 一併修（同批、低風險、高回報）

### M1 — event tap 被系統停用後無聲失效
- **位置**：`monitor/mod.rs:84-86`（`TapDisabledByTimeout | TapDisabledByUserInput` 只 `log::warn!`，未重啟）。
- **問題**：callback 太慢或使用者操作會讓系統停用 tap，目前不 re-enable、也不通知 → 雙擊 ⌘C 靜默死掉直到重啟 App。違反 PRD §7.3「監聽層異常可優雅降級」。
- **修法**：收到 disabled 事件呼叫 `CGEventTap` re-enable（保留 tap 參考）或重建 tap；無法恢復時用 tray/通知提示「雙擊已停用，請重啟」。

### M2 — `workbench_translate` 無請求序號 → 連續編輯可能顯示舊譯文
- **位置**：`workbench.rs:155-200`（`run_mt`）對照 `pipeline.rs:190-199`（Glance 已有 `request_seq`）。
- **問題**：原文 debounce 後每次獨立 await，網路慢時較早送出的較晚回，會用過時譯文蓋掉新的。
- **修法**：比照 Glance 用 `AtomicU64` 序號，回來時非最新就丟棄（或前端記 latest text 回填前比對）。把既有解法套到這條路。

### M3 — Gemini 串流無 idle timeout → 串到一半 stall 會無限等
- **位置**：`workbench.rs:51-54`（`llm_client` 只設 `connect_timeout`，刻意不設整體 timeout）。
- **問題**：不設整體 timeout 是對的（避免腰斬長回應），但無 idle/read timeout；首 token 後連線 stall → AI 字義永遠停在「串流中」，不 done 不 error。
- **修法**：對「兩次 chunk 之間」加 idle timeout（在 `chunk().await` 外包一層 timeout，逾時視為 `Network` 錯誤），保留整體不限時。

## 🟢 順手做（低風險、避免誤導/補縱深）

### L4 — 文件與現況脫節（純文字、零風險、會誤導人故順手修）
- `docs/PRD.md` 標題仍「ClipTranslate AI」→ 改 Sumi；§4.7 字典仍寫「有道/Cambridge API/mdict」→ 已被 D7（本地 ECDICT）取代，改成現況。
- `SettingsView.tsx:316` 提示「字典查詢免 key（公開字典 API）」→ 字典是本地 ECDICT，非公開 API，改正。

### L1 — 設一個最小 CSP（縱深防禦）
- `tauri.conf.json` 目前 `security.csp: null`。目前安全（只載本地、無 `innerHTML`/`eval`），但建議設最小 CSP（如 `default-src 'self'`），未來若引入 markdown/HTML 渲染才不踩雷。確認設了不會擋掉現有本地資源載入。

---

## 技術約束 / 紅線

- 從 master 開 `feature/security-fixes`，**不要直接在 master 改**（這批改的是 key/過濾敏感區）。
- 沿用既有 filter / provider / pipeline 架構，不另寫一套。
- 新增 crate 前先列出、說明、等我確認（理論上不需要）。
- 修 H1/H2 後特別自查：有沒有任何**新**路徑會把 key 或剪貼簿/原文內容寫進 log。
- 每個修掉的問題記進 `docs/issues.md`（症狀/根因/修法/狀態），對應稽核編號（H1/H2/M1…）。

## 驗收標準（每條可手動驗證）

1. **H1**：Google MT 路徑斷網/逾時時，log 裡**不含** key（URL 改 header 後 URL 本身就不含 key；或 redact 後遮掉）。手動拔網路翻一次、看 log 確認。
2. **H2**：在 Workbench 貼一段含密碼樣式的文字、點其中一個生字 → **不送 Gemini**，顯示「已略過可能的機密內容」；正常文字點字 → 只送該字所在句（非整段）。
3. **M1**：event tap 被停用後能自動 re-enable（或無法恢復時有提示），雙擊不再無聲死掉。
4. **M2**：連續快速編輯原文，畫面最終顯示的是**最新**輸入對應的譯文，不會被慢回的舊請求蓋掉。
5. **M3**：Gemini 串流中途 stall，會在合理時間內轉為錯誤訊息，不會永遠卡「串流中」。
6. **L4**：PRD 無「ClipTranslate AI」、字典敘述為本地 ECDICT；設定頁提示不再寫「公開字典 API」。
7. **L1**：設了最小 CSP，且 app 功能正常（本地資源照常載入、翻譯/字典/設定都能用）。
8. 既有功能（Glance/Workbench/語言配對/字典）全部不受影響；`cargo test --lib` 仍全綠。
9. repo 無 secret、無 key/內容被 log。

## 交付

- 修好的分支 `feature/security-fixes`。
- `docs/issues.md` 補上 H1/H2/M1/M2/M3 各一筆。
- `docs/decisions.md` 若有相關決策（如 Google 改 header）記一筆。
- 簡短回報：每條怎麼修的、H1/H2 的紅線確認結果、有沒有踩到既有功能。

## 完成後

停下來給我看結果與回報，**先不要合併**——我會請你自審 grep + 驗收後再合併進 master。