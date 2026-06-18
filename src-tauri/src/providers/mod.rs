//! 服務抽象層：MT / LLM(Gemini) / Dictionary 統一介面。
//! 外部 API 呼叫只能在這個模組（CLAUDE.md 邊界規則）。

pub mod dictionary;
pub mod llm;
pub mod mt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Google,
    Deepl,
}

impl Provider {
    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Google => "Google",
            Provider::Deepl => "DeepL",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Translation {
    pub text: String,
    /// provider 偵測到的來源語言碼（小寫，如 "en"）。
    pub detected_source: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)] // 內部欄位供 Debug 輸出與測試使用
pub enum ProviderError {
    MissingKey,
    Network(String),
    Api { status: u16, message: String },
    Parse(String),
}

impl ProviderError {
    /// 給使用者看的訊息：說「怎麼了、怎麼解」（ui-spec 文案底線）。
    pub fn user_message(&self, provider: Provider) -> String {
        self.user_message_named(provider.display_name())
    }

    /// 同上，但服務名稱自帶（給 Gemini / Dictionary 等非 MT 服務用）。
    pub fn user_message_named(&self, name: &str) -> String {
        match self {
            ProviderError::MissingKey => {
                format!("尚未設定 {name} API key — 到 Sumi 設定視窗貼上即可")
            }
            ProviderError::Network(_) => {
                format!("連不上 {name} — 檢查網路後再雙擊一次")
            }
            ProviderError::Api { status: 401 | 403, .. } => {
                format!("{name} 拒絕了這把 API key — 到設定確認 key 是否有效")
            }
            ProviderError::Api { status: 429, .. } => {
                format!("{name} 額度暫時用完 — 稍等再試")
            }
            ProviderError::Api { status, message } => {
                if message.is_empty() {
                    format!("{name} 回了錯誤（HTTP {status}）— 稍後再試")
                } else {
                    // 帶上服務端訊息（如 404 會列出可用 model），方便診斷。
                    format!("{name} 回了錯誤（HTTP {status}）：{message}")
                }
            }
            ProviderError::Parse(_) => {
                format!("看不懂 {name} 的回應 — 稍後再試，持續發生請回報")
            }
        }
    }
}

/// MT provider 統一 trait。新 provider 實作此 trait 後加進 `translate` 的分派即可。
pub trait MtTranslator {
    #[allow(dead_code)] // trait 契約的一部分，分派目前走 enum
    fn provider(&self) -> Provider;

    async fn translate(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        text: &str,
        target_lang: &str,
    ) -> Result<Translation, ProviderError>;
}

/// 依設定分派到對應 adapter（async trait 不可做 trait object，以 enum 分派）。
pub async fn translate(
    provider: Provider,
    client: &reqwest::Client,
    api_key: &str,
    text: &str,
    target_lang: &str,
) -> Result<Translation, ProviderError> {
    match provider {
        Provider::Google => {
            mt::GoogleTranslator
                .translate(client, api_key, text, target_lang)
                .await
        }
        Provider::Deepl => {
            mt::DeepLTranslator
                .translate(client, api_key, text, target_lang)
                .await
        }
    }
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build HTTP client")
}

/// 遮掉錯誤字串中可能夾帶的 API key（紅線縱深防禦）。
///
/// reqwest 連線層錯誤的 `Display` 會帶上完整請求 URL；若 key 放在 query string
/// （`?key=AIza…`），這個字串一旦進 log 就洩漏整把金鑰。Google MT 的 key 已改放 header
/// （URL 不再含 key），此函式是「未來任何拿 reqwest error 字串去 log」的根因防線：
/// 在 `ProviderError::Network` 建構時就把 `key=<value>` 一律遮成 `key=REDACTED`。
pub fn redact_secrets(input: &str) -> String {
    // ASCII 小寫化後做大小寫無關搜尋；位元組長度不變，索引與原字串對齊。
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut idx = 0;
    while idx < input.len() {
        match lower[idx..].find("key=") {
            Some(rel) => {
                let val_start = idx + rel + 4; // 跳過 "key="
                out.push_str(&input[idx..val_start]);
                // 吃掉 value：URL-safe key 字元（英數與 - _ . ~）。
                let mut end = val_start;
                for (off, c) in input[val_start..].char_indices() {
                    if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                        end = val_start + off + c.len_utf8();
                    } else {
                        break;
                    }
                }
                if end > val_start {
                    out.push_str("REDACTED");
                }
                idx = end;
            }
            None => {
                out.push_str(&input[idx..]);
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_key_in_reqwest_url_error() {
        let err = "error sending request for url (https://translation.googleapis.com/language/translate/v2?key=AIzaSyD-9tSrke72PouQMnMX-a7eZSW0jkFMBWY)";
        let redacted = redact_secrets(err);
        assert!(!redacted.contains("AIzaSyD"), "key must not survive: {redacted}");
        assert!(redacted.contains("key=REDACTED"));
        assert!(redacted.contains("translate/v2")); // 其餘 URL 保留供診斷
    }

    #[test]
    fn redacts_multiple_and_is_case_insensitive() {
        let s = redact_secrets("a?Key=abc123&b=2&key=xyz_789");
        assert!(!s.contains("abc123") && !s.contains("xyz_789"));
        assert_eq!(s, "a?Key=REDACTED&b=2&key=REDACTED");
    }

    #[test]
    fn leaves_key_free_strings_untouched() {
        let s = "connection timed out after 10s";
        assert_eq!(redact_secrets(s), s);
        // 「key=」後面沒有值（空字串）不亂插 REDACTED。
        assert_eq!(redact_secrets("key="), "key=");
    }
}
