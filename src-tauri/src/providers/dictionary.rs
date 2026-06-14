//! 真字典資料源：Free Dictionary API（dictionaryapi.dev），免 key、英文。
//! 只回事實性資料（音標 / 詞性 / 釋義），不經 LLM，避免幻覺（CLAUDE.md：字典 ≠ LLM）。

use serde::{Deserialize, Serialize};

use super::ProviderError;

const ENDPOINT: &str = "https://api.dictionaryapi.dev/api/v2/entries/en";
/// 每個詞性最多顯示幾條釋義，保持卡片精簡。
const MAX_DEFS_PER_MEANING: usize = 3;

/// 送給前端的字典條目。
#[derive(Clone, Debug, Serialize)]
pub struct DictionaryEntry {
    pub word: String,
    pub phonetic: Option<String>,
    pub meanings: Vec<DictMeaning>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DictMeaning {
    pub part_of_speech: String,
    pub definitions: Vec<String>,
}

// ── API 回應 ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ApiEntry {
    word: String,
    phonetic: Option<String>,
    #[serde(default)]
    phonetics: Vec<ApiPhonetic>,
    #[serde(default)]
    meanings: Vec<ApiMeaning>,
}

#[derive(Deserialize)]
struct ApiPhonetic {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct ApiMeaning {
    #[serde(rename = "partOfSpeech")]
    part_of_speech: String,
    #[serde(default)]
    definitions: Vec<ApiDefinition>,
}

#[derive(Deserialize)]
struct ApiDefinition {
    definition: String,
}

fn parse(body: &str) -> Option<DictionaryEntry> {
    // 404 時 API 回的是物件（`{"title":...}`）而非陣列，反序列化失敗即視為查無此字。
    let entries: Vec<ApiEntry> = serde_json::from_str(body).ok()?;
    let first = entries.into_iter().next()?;

    let phonetic = first
        .phonetic
        .filter(|p| !p.trim().is_empty())
        .or_else(|| {
            first
                .phonetics
                .iter()
                .find_map(|p| p.text.clone().filter(|t| !t.trim().is_empty()))
        });

    let meanings = first
        .meanings
        .into_iter()
        .map(|m| DictMeaning {
            part_of_speech: m.part_of_speech,
            definitions: m
                .definitions
                .into_iter()
                .map(|d| d.definition)
                .filter(|d| !d.trim().is_empty())
                .take(MAX_DEFS_PER_MEANING)
                .collect(),
        })
        .filter(|m| !m.definitions.is_empty())
        .collect();

    Some(DictionaryEntry {
        word: first.word,
        phonetic,
        meanings,
    })
}

/// 查單字。查無此字回 `Ok(None)`（正常情況，不是錯誤）。
pub async fn lookup(
    client: &reqwest::Client,
    word: &str,
) -> Result<Option<DictionaryEntry>, ProviderError> {
    let word = word.trim();
    if word.is_empty() {
        return Ok(None);
    }
    let url = format!("{ENDPOINT}/{}", urlencoding(word));
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;

    let status = resp.status();
    // 404 = 查無此字（API 設計如此），回 None 不報錯。
    if status.as_u16() == 404 {
        return Ok(None);
    }
    let body = resp
        .text()
        .await
        .map_err(|e| ProviderError::Network(e.to_string()))?;
    if !status.is_success() {
        return Err(ProviderError::Api {
            status: status.as_u16(),
            message: String::new(),
        });
    }
    Ok(parse(&body))
}

/// 最小 URL path 編碼：字典查詢只會是單字，處理空白與少數符號即可。
fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            c if c.is_ascii_alphanumeric() || matches!(c, '-' | '\'' | '.') => c.to_string(),
            c => {
                let mut buf = [0u8; 4];
                c.encode_utf8(&mut buf)
                    .bytes()
                    .map(|b| format!("%{b:02X}"))
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dictionary_entry() {
        let body = r#"[{"word":"translate","phonetic":"/trænzleɪt/","phonetics":[],
            "meanings":[{"partOfSpeech":"verb","definitions":[
                {"definition":"to turn into another language"},
                {"definition":"to change from one form to another"}]}]}]"#;
        let entry = parse(body).unwrap();
        assert_eq!(entry.word, "translate");
        assert_eq!(entry.phonetic.as_deref(), Some("/trænzleɪt/"));
        assert_eq!(entry.meanings.len(), 1);
        assert_eq!(entry.meanings[0].part_of_speech, "verb");
        assert_eq!(entry.meanings[0].definitions.len(), 2);
    }

    #[test]
    fn falls_back_to_phonetics_array() {
        let body = r#"[{"word":"x","phonetics":[{"text":""},{"text":"/eks/"}],
            "meanings":[{"partOfSpeech":"noun","definitions":[{"definition":"the letter x"}]}]}]"#;
        let entry = parse(body).unwrap();
        assert_eq!(entry.phonetic.as_deref(), Some("/eks/"));
    }

    #[test]
    fn caps_definitions_per_meaning() {
        let body = r#"[{"word":"run","meanings":[{"partOfSpeech":"verb","definitions":[
            {"definition":"a"},{"definition":"b"},{"definition":"c"},{"definition":"d"}]}]}]"#;
        let entry = parse(body).unwrap();
        assert_eq!(entry.meanings[0].definitions.len(), MAX_DEFS_PER_MEANING);
    }

    #[test]
    fn not_found_object_returns_none() {
        let body = r#"{"title":"No Definitions Found","message":"Sorry pal"}"#;
        assert!(parse(body).is_none());
    }

    #[test]
    fn url_encodes_non_ascii() {
        assert_eq!(urlencoding("café"), "caf%C3%A9");
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("don't"), "don't");
    }
}
