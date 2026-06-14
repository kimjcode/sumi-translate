//! LLM provider：Gemini（預設）。只用於文法 / 語境 / 改寫——翻譯/字典不走這裡。
//! 真 token 串流：用 reqwest 的 `chunk()` 逐塊讀 SSE，逐 token 回呼。

use serde::Deserialize;

use super::ProviderError;

/// 預設模型。集中為常數，日後換版只改這裡。
pub const GEMINI_MODEL: &str = "gemini-2.0-flash";

fn endpoint(model: &str) -> String {
    format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"
    )
}

#[derive(Deserialize)]
struct StreamChunk {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<Content>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    #[serde(default)]
    text: Option<String>,
}

/// 從一行 SSE `data:` 取出本塊的文字增量（可能跨多個 part）。
fn extract_delta(json: &str) -> Option<String> {
    let chunk: StreamChunk = serde_json::from_str(json).ok()?;
    let mut out = String::new();
    for cand in chunk.candidates {
        if let Some(content) = cand.content {
            for part in content.parts {
                if let Some(t) = part.text {
                    out.push_str(&t);
                }
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// 串流產生內容。`on_token` 每收到一段增量就被呼叫；回傳 `false` 可提早中止（取消）。
/// 紅線：prompt / 輸出內容都不在這裡 log。
pub async fn stream_generate(
    client: &reqwest::Client,
    api_key: &str,
    prompt: &str,
    mut on_token: impl FnMut(&str) -> bool,
) -> Result<(), ProviderError> {
    let body = serde_json::json!({
        "contents": [{ "parts": [{ "text": prompt }] }]
    });

    let mut resp = client
        .post(endpoint(GEMINI_MODEL))
        .header("x-goog-api-key", api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v["error"]["message"].as_str().map(str::to_string))
            .unwrap_or_default();
        return Err(ProviderError::Api {
            status: status.as_u16(),
            message,
        });
    }

    // 以 bytes 緩衝：只解碼整行（newline 結尾），避免在 chunk 邊界切斷 CJK 多位元組字元。
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let chunk = resp
            .chunk()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;
        let Some(bytes) = chunk else { break };
        buffer.extend_from_slice(&bytes);

        while let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buffer.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line[..line.len() - 1]);
            let line = line.trim();
            if let Some(json) = line.strip_prefix("data:") {
                let json = json.trim();
                if json == "[DONE]" {
                    return Ok(());
                }
                if let Some(delta) = extract_delta(json) {
                    if !on_token(&delta) {
                        return Ok(()); // 被取消
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_delta() {
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"你好"}],"role":"model"}}]}"#;
        assert_eq!(extract_delta(json).as_deref(), Some("你好"));
    }

    #[test]
    fn concatenates_multiple_parts() {
        let json = r#"{"candidates":[{"content":{"parts":[{"text":"a"},{"text":"b"}]}}]}"#;
        assert_eq!(extract_delta(json).as_deref(), Some("ab"));
    }

    #[test]
    fn empty_chunk_yields_none() {
        let json = r#"{"candidates":[{"content":{"parts":[]}}]}"#;
        assert_eq!(extract_delta(json), None);
        // 沒有 candidates（如只有 usageMetadata 的尾塊）也回 None。
        assert_eq!(extract_delta(r#"{"usageMetadata":{}}"#), None);
    }
}
