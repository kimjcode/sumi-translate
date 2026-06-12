//! MT adapters：Google Cloud Translation v2（預設）與 DeepL。
//! 兩者皆一次回傳整段譯文（無 token 串流），來源語言用 provider 內建 auto-detect。

use serde::Deserialize;

use super::{MtTranslator, Provider, ProviderError, Translation};

// ── Google ────────────────────────────────────────────────────────────────

pub struct GoogleTranslator;

const GOOGLE_ENDPOINT: &str = "https://translation.googleapis.com/language/translate/v2";

#[derive(Deserialize)]
struct GoogleResponse {
    data: GoogleData,
}

#[derive(Deserialize)]
struct GoogleData {
    translations: Vec<GoogleItem>,
}

#[derive(Deserialize)]
struct GoogleItem {
    #[serde(rename = "translatedText")]
    translated_text: String,
    #[serde(rename = "detectedSourceLanguage")]
    detected_source_language: Option<String>,
}

fn parse_google(body: &str) -> Result<Translation, ProviderError> {
    let resp: GoogleResponse =
        serde_json::from_str(body).map_err(|e| ProviderError::Parse(e.to_string()))?;
    let item = resp
        .data
        .translations
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Parse("empty translations array".into()))?;
    Ok(Translation {
        text: item.translated_text,
        detected_source: item.detected_source_language.map(|s| s.to_lowercase()),
    })
}

impl MtTranslator for GoogleTranslator {
    fn provider(&self) -> Provider {
        Provider::Google
    }

    async fn translate(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        text: &str,
        target_lang: &str,
    ) -> Result<Translation, ProviderError> {
        // Google API key 僅含 URL-safe 字元（英數、-、_），可直接放 query string。
        let resp = client
            .post(format!("{GOOGLE_ENDPOINT}?key={api_key}"))
            .json(&serde_json::json!({
                "q": text,
                "target": target_lang,
                "format": "text",
            }))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if !status.is_success() {
            return Err(api_error(status.as_u16(), &body));
        }
        parse_google(&body)
    }
}

// ── DeepL ─────────────────────────────────────────────────────────────────

pub struct DeepLTranslator;

/// DeepL 免費層的 key 以 `:fx` 結尾，走 api-free 端點。
fn deepl_endpoint(api_key: &str) -> &'static str {
    if api_key.ends_with(":fx") {
        "https://api-free.deepl.com/v2/translate"
    } else {
        "https://api.deepl.com/v2/translate"
    }
}

/// 把通用語言碼映射成 DeepL 的 target_lang 格式。
fn deepl_target_lang(target: &str) -> String {
    match target {
        "zh-TW" => "ZH-HANT".into(),
        "zh-CN" => "ZH-HANS".into(),
        "en" => "EN-US".into(),
        other => other.to_uppercase(),
    }
}

#[derive(Deserialize)]
struct DeepLResponse {
    translations: Vec<DeepLItem>,
}

#[derive(Deserialize)]
struct DeepLItem {
    text: String,
    detected_source_language: Option<String>,
}

fn parse_deepl(body: &str) -> Result<Translation, ProviderError> {
    let resp: DeepLResponse =
        serde_json::from_str(body).map_err(|e| ProviderError::Parse(e.to_string()))?;
    let item = resp
        .translations
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Parse("empty translations array".into()))?;
    Ok(Translation {
        text: item.text,
        detected_source: item.detected_source_language.map(|s| s.to_lowercase()),
    })
}

impl MtTranslator for DeepLTranslator {
    fn provider(&self) -> Provider {
        Provider::Deepl
    }

    async fn translate(
        &self,
        client: &reqwest::Client,
        api_key: &str,
        text: &str,
        target_lang: &str,
    ) -> Result<Translation, ProviderError> {
        let resp = client
            .post(deepl_endpoint(api_key))
            .header("Authorization", format!("DeepL-Auth-Key {api_key}"))
            .json(&serde_json::json!({
                "text": [text],
                "target_lang": deepl_target_lang(target_lang),
            }))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        if !status.is_success() {
            return Err(api_error(status.as_u16(), &body));
        }
        parse_deepl(&body)
    }
}

// ── 共用 ──────────────────────────────────────────────────────────────────

/// 從失敗回應中盡量撈出訊息欄位；撈不到就放狀態碼。注意：訊息可能含請求摘要，不進 log。
fn api_error(status: u16, body: &str) -> ProviderError {
    #[derive(Deserialize)]
    struct ErrBody {
        error: Option<ErrInner>,
        message: Option<String>,
    }
    #[derive(Deserialize)]
    struct ErrInner {
        message: Option<String>,
    }
    let message = serde_json::from_str::<ErrBody>(body)
        .ok()
        .and_then(|e| e.error.and_then(|i| i.message).or(e.message))
        .unwrap_or_default();
    ProviderError::Api { status, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_response() {
        let body = r#"{"data":{"translations":[{"translatedText":"哈囉，世界","detectedSourceLanguage":"en"}]}}"#;
        let t = parse_google(body).unwrap();
        assert_eq!(t.text, "哈囉，世界");
        assert_eq!(t.detected_source.as_deref(), Some("en"));
    }

    #[test]
    fn parses_google_response_without_detection() {
        let body = r#"{"data":{"translations":[{"translatedText":"hi"}]}}"#;
        let t = parse_google(body).unwrap();
        assert_eq!(t.detected_source, None);
    }

    #[test]
    fn parses_deepl_response() {
        let body =
            r#"{"translations":[{"detected_source_language":"EN","text":"哈囉，世界"}]}"#;
        let t = parse_deepl(body).unwrap();
        assert_eq!(t.text, "哈囉，世界");
        assert_eq!(t.detected_source.as_deref(), Some("en"));
    }

    #[test]
    fn rejects_malformed_response() {
        assert!(matches!(
            parse_google(r#"{"data":{"translations":[]}}"#),
            Err(ProviderError::Parse(_))
        ));
        assert!(matches!(parse_deepl("not json"), Err(ProviderError::Parse(_))));
    }

    #[test]
    fn deepl_free_key_uses_free_endpoint() {
        assert_eq!(
            deepl_endpoint("abcd-1234:fx"),
            "https://api-free.deepl.com/v2/translate"
        );
        assert_eq!(deepl_endpoint("abcd-1234"), "https://api.deepl.com/v2/translate");
    }

    #[test]
    fn maps_target_langs_for_deepl() {
        assert_eq!(deepl_target_lang("zh-TW"), "ZH-HANT");
        assert_eq!(deepl_target_lang("zh-CN"), "ZH-HANS");
        assert_eq!(deepl_target_lang("en"), "EN-US");
        assert_eq!(deepl_target_lang("ja"), "JA");
    }

    #[test]
    fn extracts_api_error_message() {
        let e = api_error(403, r#"{"error":{"message":"API key not valid"}}"#);
        match e {
            ProviderError::Api { status, message } => {
                assert_eq!(status, 403);
                assert_eq!(message, "API key not valid");
            }
            _ => panic!("wrong variant"),
        }
    }
}
