# 申請 Google Cloud Translation API key

Sumi 的快翻路徑預設走 Google Cloud Translation。以下是取得 key 的最短路徑；細節以 Google 官方文件為準（連結在文末）。Cloud Translation 是付費 API，需要在 GCP 專案上綁定帳單帳戶（無永久免費層；新帳戶有 $300 試用額度）。

## 步驟

1. **建立／選擇 GCP 專案**
   到 [Google Cloud Console](https://console.cloud.google.com/) 建立一個新專案，或選一個現有專案。

2. **啟用 Cloud Translation API**
   在 [API Library](https://console.cloud.google.com/apis/library/translate.googleapis.com) 搜尋 *Cloud Translation API* 並按「啟用」（首次啟用會要求綁定帳單帳戶）。

3. **建立 API key**
   到 [憑證頁](https://console.cloud.google.com/apis/credentials) →「建立憑證」→「API 金鑰」。

4. **★ 限制這把 key（重要安全步驟，務必做）**
   不受限的 key 一旦外洩，任何人都能拿去盜刷你綁定的帳單。透過 Console 建立金鑰時，現在會**要求你至少設一項限制**才能建立——請選最嚴的：
   - **API 限制**：「限制金鑰」→ 只勾 **Cloud Translation API**。這樣即使 key 外洩，也只能呼叫翻譯、無法動用你專案裡其他服務。（建好後仍可隨時回憑證頁點該 key 調整。）
   - **用量上限／預算警示**：到 [Billing → Budgets & alerts](https://console.cloud.google.com/billing/budgets) 設一個每月預算與警示門檻，盜刷時能及早發現。也可在 [配額頁](https://console.cloud.google.com/iam-admin/quotas) 調低 Translation 的每日用量上限作為硬上限。
   > Sumi 本身把 key 存在 macOS Keychain、不進 log、不進檔案（見 README〈隱私〉），但「key 在 Google 端的權限範圍」只能在 GCP 設定，請務必做這一步。

5. **貼進 Sumi**
   開 Sumi → 設定 → 翻譯引擎 → 貼上這把 key。

## 替代選項：DeepL

不想用 Google 的話，Sumi 也支援 [DeepL API](https://www.deepl.com/pro-api)（設定頁可切換）。注意 DeepL **免費層**（key 以 `:fx` 結尾）可能用送出的文字改善服務；**付費 Pro** 才預設不訓練。

## 官方文件

- [Cloud Translation 快速入門](https://docs.cloud.google.com/translate/docs/setup)
- [使用 API 金鑰](https://docs.cloud.google.com/docs/authentication/api-keys)
- [為 API 金鑰設定限制](https://docs.cloud.google.com/docs/authentication/api-keys#securing)
