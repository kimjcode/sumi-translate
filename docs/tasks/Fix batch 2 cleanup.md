# 修正第二批：測試補強 + 記錄（貼給 Claude Code · 新 session）

> 依稽核報告 `docs/audit-20260618.md`。**第一批 `feature/security-fixes` 合併進 master 後**，再從 master 開分支 `feature/audit-cleanup`。
> 這批是「健康習慣 + 記錄」，不急、不該拖慢第一批。優先級低於第一批。
> 開始前讀 `CLAUDE.md`、`docs/audit-20260618.md`、`docs/issues.md`、`docs/decisions.md`。

---

## M5 — 補測試缺口（改程式 + 加測試）

- **位置**：`pipeline.rs` 的 `cache_get/cache_put`（LRU、`CACHE_CAP` 邊界）、`cache_key`、`routing_signature` 無單元測試；前端 `wordAtCaret`（`Workbench.tsx:312`）、def-seq 過濾、debounce 無測試。
- **要做**：
  - 替 `cache_*`（含 LRU 淘汰、容量邊界）與 `routing_signature`、`cache_key`（碰撞）補 Rust 單元測試。
  - 把 `wordAtCaret` 抽成純函式，加前端測試（Vitest）：涵蓋標點、連字號、選取 vs 點擊、游標在詞首/詞中/詞尾、空白處點擊等邊界。
- **理由**：filter/double_press/router/dictionary 都測得不錯，但這幾個「容易默默錯」的點沒守住（快取鍵碰撞、LRU 淘汰、選字邊界）。

## L2 — 記錄過濾層已知誤殺樣本（只記文件，不改程式）

- **位置**：`filter.rs:107-130`。
- **要做**：在 `docs/issues.md` 或 `docs/decisions.md` 補記已知會被「寧可錯殺」判為 `Secret` 而靜默不翻的樣本：純 40 碼 SHA-1、含連字號的 UUID 等。
- **理由**：符合 D3 精神（不是 bug），但記下來避免日後被當新 bug 重查。

## L3 — 記錄背景執行緒讀 NSPasteboard 的風險（只記文件）

- **位置**：`monitor/pasteboard.rs:18-20` + `monitor/mod.rs:128`。
- **要做**：在 `docs/issues.md` 留一筆：`change_count()` 在 event-tap 背景執行緒讀 NSPasteboard，NSPasteboard 非保證 thread-safe（目前低風險可接受）。當作日後若出現詭異 changeCount 的排查起點。
- **理由**：低風險、不需現在改，但值得留排查線索。

## M4 — 決策：多行夾帶機密整段送 MT（多半維持現狀，記一筆）

- **位置**：`filter.rs:48-72`（`looks_like_secret` 多行只殺 PEM/PGP，`KEY=value` 規則在多行時 return false）。
- **背景**：這是 D3「log/程式碼照翻、寧可放行」的已知取捨，不是 bug。第一批 H2 修好後，Workbench 字典路徑的機密已擋；M4 講的是 **MT 翻譯路徑**的多行情境（如貼整份 `.env` 去翻，含密碼那行會一起送）。
- **預設處理（維持現狀 + 記錄）**：在 `docs/decisions.md` 記一筆「M4 多行夾帶機密整段送 MT，為 D3 取捨下的已知、可接受風險；不收緊」。
- **若我另行指示要收緊**（這張卡預設**不做**，除非我說）：多行時逐行掃 `secret_assignment_value` 與已知前綴 token，命中就遮該行再送（而非整段放行或整段殺）。**先別做，等我決定。**

---

## 技術約束

- 在 master（第一批已合併後）開 `feature/audit-cleanup`。
- M5 是改程式 + 加測試；L2/L3/M4 主要是記文件。新增測試框架（Vitest）前先說一聲。
- 紅線照舊。

## 驗收標準

1. `cache_*`、`routing_signature`、`cache_key` 有單元測試，涵蓋 LRU 淘汰與容量邊界。
2. `wordAtCaret` 抽成純函式並有前端測試涵蓋邊界情況。
3. `docs/issues.md` 補上 L2（誤殺樣本）、L3（NSPasteboard 背景讀）兩筆記錄。
4. `docs/decisions.md` 補上 M4 的「已知可接受、不收緊」決策。
5. `cargo test --lib` 與前端測試皆綠。
6. 既有功能不受影響。

## 交付

- 修好的分支 `feature/audit-cleanup`。
- 更新的 issues.md / decisions.md。
- 簡短回報：新增了哪些測試、有沒有在補測試時意外發現 bug。

## 完成後

停下來給我看結果，先不要合併。