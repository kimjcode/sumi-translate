# Spike 報告 — macOS 系統辭典英漢查詢

**日期：** 2026-06-15
**分支：** `spike/system-dictionary`
**驗證物：** `src-tauri/examples/dict_spike.rs`（`cargo run --example dict_spike`），無新增 crate（用既有 `core-foundation` + extern 連 CoreServices）。
**範圍：** 只驗證能不能乾淨拿到結構化英漢釋義，**未接進 Workbench**。

---

## 結論先講

**資料極好，但「乾淨的公開 API」拿不到結構化資料；要拿到好東西得直接讀 `.dictionary` 詞庫檔，而那條路有實質摩擦（要自寫 Apple 辭典格式 parser + 動態定位一個路徑含 hash、且取決於使用者語言設定的系統資產）。**

→ 偏向任務的選項 (b)：**建議改評估「自帶一份自由授權的英漢資料集」**（下方給了比泛稱 mdict 更具體的候選），而不是賭系統辭典。理由見 §4。

---

## Q1：拿不拿得到中文釋義？

**拿得到——而且是繁中。** 這台機器的系統資產裡有 Apple 官方的 **「Traditional Chinese - English.dictionary」**（繁中-英文），實際 entry（直接解 `Body.data` 取得）：

- `A` → 「英語字母中第一個字母，小寫為 a」
- `a`（冠詞）→ 「一」「任一」，附例句中譯「他已在洛杉磯找到一份工作。」

所以中文釋義、詞性、音標、例句中譯都有。

**但 `DCSCopyTextDefinition`（公開 API）在這台機器回 NULL**——因為使用者從沒在「辭典」app 設定啟用清單（`com.apple.DictionaryServices` 只有 `DCSPreferenceVersion=7`，沒有 `DCSActiveDictionaries`），而該 API **只查「已啟用」的辭典集**。

## Q2：回傳能不能結構化？

**取決於走哪條路，差很多：**

| 路徑 | 拿到的東西 | 結構化 |
|---|---|---|
| `DCSCopyTextDefinition`（公開 API） | **已排版的純文字**（換行分隔），且無法指定用哪本辭典 | ✗ 要靠純文字 heuristic 硬拆，且內容/語言看使用者啟用了什麼 |
| 直接讀 `.dictionary` 的 `Body.data` | **乾淨的語意 XML** | ✓ 很好拆 |

直接讀檔拿到的 entry markup（節錄）：

```xml
<d:entry d:title="A" lang="zh-cmn-Hant-TW">
  <span class="hw">A</span>                          <!-- 詞 -->
  <span class="pr">美 <span class="ph t_US">е</span></span>   <!-- 美式音標 -->
  <span class="pr">英 <span class="ph t_UK">еi</span></span>  <!-- 英式音標 -->
  <span d:pos="1" class="pos">n.</span>              <!-- 詞性 -->
  <span class="sn">1</span>                          <!-- 義項編號 -->
  <span d:def="1" class="trans">英語字母中第一個字母…</span>  <!-- 中文釋義 -->
  <span class="ex">The first letter…</span>          <!-- 英文例句 -->
  <span class="trans">英語字母表中的第一個字母是A。</span>      <!-- 例句中譯 -->
</d:entry>
```

有 `d:title / d:pos / d:def / d:prn` 命名空間屬性 **加上** `hw / ph / pos / trans / sn / ex` 的 CSS class——拆成「音標 / 詞性 / 中文釋義 / 例句」非常直接，比英英 Free Dictionary 還乾淨、且是中文。**但這份結構只有走「直接讀檔」才拿得到；公開 API 只給純文字。**

## Q3：依賴使用者裝詞庫嗎？

- 詞庫**不是**使用者手動裝的，是 macOS 的**系統資產**（`/System/Library/AssetsV2/com_apple_MobileAsset_DictionaryServices_dictionary3macOS/<hash>.asset/AssetData/…`），**隨需下載**。這台已經有（推測因為系統語言含繁中）。
- 這台同時有：`Traditional Chinese - English`、`Traditional Chinese - English Idioms`、`Traditional Chinese`、`Traditional Chinese Common Words`、`Apple Dictionary`。
- **公開 API 查無的行為**：回 NULL（不是錯誤）。要它有東西，使用者得先去「辭典」app 勾選啟用——而且**還是無法指定**要回哪一本（多本啟用時回第一個命中者）。
- 兩個隱憂：① 路徑含 hash，會隨 macOS 更新/重下載改變，得**動態搜尋**定位；② 資產存在與否**綁使用者的語言設定**，不是每台 Mac 都有、可能要觸發下載。

---

## 4. 評估與建議

### 走系統辭典的兩條路，都有硬傷

- **走公開 API `DCSCopyTextDefinition`**：拿到純文字、不能選辭典、要使用者先啟用、跨機器不確定。**結構化與可靠度都不夠**，否決。
- **走直接讀 `.dictionary` 檔**：資料結構超好，但要：
  1. 自寫 **Apple 辭典格式 parser**（`Body.data` 是 0x40 header + 一串 zlib chunk，每個 chunk 解出多個「4-byte 長度 + `<d:entry>` XML」；查特定字還要解 `KeyText.index`）。是個小型 parser 工程。
  2. **動態定位** 路徑含 hash 的系統資產（搜 `AssetsV2`）。
  3. 仰賴使用者的語言設定讓該資產存在（不保證）。
  4. 讀系統檔（Sumi 目前非沙盒可行；未來若沙盒化會破）。

### 更穩的替代：自帶一份「自由授權」英漢資料集

與其賭系統辭典，不如**自己掌握資料**。比泛稱 mdict 更具體的候選：

- **ECDICT**（開源英漢詞典，~77 萬詞，CSV/SQLite，授權寬鬆可再散布）：英→中釋義 + 音標 + 詞性標記，**永遠在、離線、隱私友善、跨機器一致、不依賴系統資產、不必自寫 Apple 格式 parser**。
- 對比多數 mdict 的 `.mdx`（多為**版權**英漢詞典，不可隨 app 散布）——ECDICT 的授權正是為了避開這點。

**取捨**：系統辭典資料品質（含例句中譯、US/UK 音標）略勝；但 ECDICT 在「可靠、可控、可散布、實作成本」全面勝出，且品質對 Workbench 上段（音標/詞性/中文釋義）已足夠。

### 我的建議

**不要走系統辭典**（API 不堪用、直接讀檔是 parser 工程且資產不保證存在）。**改評估自帶 ECDICT 之類的自由授權英漢資料集**取代現有英英；深度語境照舊交給第二段 Gemini。

> 若你仍想用系統辭典的高品質資料，可行但成本高：要把「Apple 辭典格式 parser + 資產動態定位 + 找不到時 fallback」當成一個獨立任務做，且接受跨機器/沙盒的不確定性。

---

## 附：實際指令與樣本（可重現）

```bash
# 公開 API（這台回 NULL，因無啟用辭典）
cargo run --example dict_spike

# 盤點系統辭典資產
find /System/Library/AssetsV2 -name "*.dictionary" -maxdepth 6 2>/dev/null

# 直接解 Body.data 看 entry 結構（見本報告 Q2 樣本）
```