//! 過濾層純邏輯。原則（已拍板）：機密寧可錯殺，其餘寧可放行。
//! log / 程式碼照翻；只有「整段看起來就是一把鑰匙」才跳過。

/// 超過此字元數只翻前段並標記截斷。
pub const MAX_TRANSLATE_CHARS: usize = 2000;

#[derive(Debug, PartialEq)]
pub enum Classification {
    /// 空白內容 → no-op。
    Empty,
    /// 疑似機密 → 永不送出，浮窗顯示「已略過」。
    Secret,
    /// 整段就是一個 URL 或檔案路徑 → 靜默不翻。
    UrlOrPath,
    /// 正常送翻。
    Text { text: String, truncated: bool },
}

pub fn classify(raw: &str) -> Classification {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Classification::Empty;
    }
    // URL/路徑比機密更具體，先判（反正兩者都不送出，順序不影響安全）。
    if is_pure_url(trimmed) || is_file_path(trimmed) {
        return Classification::UrlOrPath;
    }
    if looks_like_secret(trimmed) {
        return Classification::Secret;
    }

    let char_count = trimmed.chars().count();
    if char_count > MAX_TRANSLATE_CHARS {
        Classification::Text {
            text: trimmed.chars().take(MAX_TRANSLATE_CHARS).collect(),
            truncated: true,
        }
    } else {
        Classification::Text {
            text: trimmed.to_string(),
            truncated: false,
        }
    }
}

// ── 機密偵測 ──────────────────────────────────────────────────────────────

fn looks_like_secret(s: &str) -> bool {
    // 多行內容只在含金鑰區塊標頭時才殺（log/程式碼是核心情境，不能誤殺）。
    if s.contains("PRIVATE KEY-----") || s.contains("-----BEGIN PGP") {
        return true;
    }
    if s.lines().count() > 1 {
        return false;
    }
    // 單行 KEY=value / KEY: value，左邊名稱像機密欄位。
    if let Some(value) = secret_assignment_value(s) {
        if !value.is_empty() {
            return true;
        }
    }
    // 以下規則只適用「整段就是單一 token」。
    if s.split_whitespace().count() != 1 {
        return false;
    }
    let token = s;
    has_secret_prefix(token)
        || is_jwt(token)
        || is_long_hex(token)
        || is_long_base64(token)
        || is_password_like(token)
}

/// `API_KEY=xxx` / `password: xxx` 之類的單行賦值。
fn secret_assignment_value(line: &str) -> Option<&str> {
    let (name, value) = line.split_once(['=', ':'])?;
    let name = name.trim().to_lowercase();
    const MARKERS: [&str; 8] = [
        "password", "passwd", "pwd", "secret", "token", "api_key", "apikey", "access_key",
    ];
    if MARKERS.iter().any(|m| name.ends_with(m)) && !name.contains(' ') {
        Some(value.trim())
    } else {
        None
    }
}

/// 已知服務的 key 前綴。
fn has_secret_prefix(token: &str) -> bool {
    const PREFIXES: [&str; 13] = [
        "sk-", "sk_live_", "sk_test_", "ghp_", "gho_", "ghs_", "ghr_", "github_pat_",
        "glpat-", "xoxb-", "xoxp-", "AIza", "ya29.",
    ];
    if PREFIXES.iter().any(|p| token.starts_with(p)) && token.len() >= 12 {
        return true;
    }
    // AWS access key id：AKIA + 16 碼大寫英數。
    token.len() == 20
        && token.starts_with("AKIA")
        && token[4..].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn is_jwt(token: &str) -> bool {
    token.starts_with("eyJ") && token.matches('.').count() == 2 && token.len() > 20
}

fn is_long_hex(token: &str) -> bool {
    token.len() >= 32 && token.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_long_base64(token: &str) -> bool {
    token.len() >= 40
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
}

/// 通用密碼樣式：單一 token、同時含「數字 + 符號」且大小寫至少其一。
/// 寧可錯殺：`P@ssw0rd!23` 會被擋；連字號普通詞（`Pre-trained`）與一般單字不會。
fn is_password_like(token: &str) -> bool {
    let len = token.chars().count();
    if !(8..=128).contains(&len) {
        return false;
    }
    let has_upper = token.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = token.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = token.chars().any(|c| c.is_ascii_digit());
    let has_punct = token.chars().any(|c| c.is_ascii_punctuation());
    has_punct && has_digit && (has_upper || has_lower)
}

// ── URL / 路徑 ────────────────────────────────────────────────────────────

fn is_pure_url(s: &str) -> bool {
    if s.split_whitespace().count() != 1 {
        return false;
    }
    ["http://", "https://", "ftp://", "www."]
        .iter()
        .any(|p| s.starts_with(p))
}

fn is_file_path(s: &str) -> bool {
    if s.lines().count() > 1 {
        return false;
    }
    s.starts_with("file://")
        || s.starts_with("~/")
        || (s.starts_with('/') && s.len() > 1 && !s.starts_with("/ "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_secret(s: &str) {
        assert_eq!(classify(s), Classification::Secret, "should be secret: {s:?}");
    }

    fn assert_translates(s: &str) {
        assert!(
            matches!(classify(s), Classification::Text { .. }),
            "should translate: {s:?}"
        );
    }

    // 機密：永不送出
    #[test]
    fn blocks_known_key_prefixes() {
        assert_secret("sk-proj-abc123def456ghi789");
        assert_secret("ghp_16C7e42F292c6912E7710c838347Ae178B4a");
        assert_secret("AKIAIOSFODNN7EXAMPLE");
        assert_secret("AIzaSyD-9tSrke72PouQMnMX-a7eZSW0jkFMBWY");
        assert_secret("xoxb-123456789-abcdefghij");
    }

    #[test]
    fn blocks_jwt() {
        assert_secret("eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U");
    }

    #[test]
    fn blocks_long_hex_and_base64() {
        assert_secret("d41d8cd98f00b204e9800998ecf8427ed41d8cd9");
        assert_secret("dGhpcyBpcyBhIHNlY3JldA==aaaabbbbccccddddeeee".replace(' ', "").as_str());
    }

    #[test]
    fn blocks_password_like_token() {
        assert_secret("P@ssw0rd!23");
        assert_secret("Tr0ub4dor&3");
    }

    #[test]
    fn blocks_secret_assignment() {
        assert_secret("DB_PASSWORD=hunter2hunter2");
        assert_secret("api_key: abc123def456");
    }

    #[test]
    fn blocks_private_key_block() {
        assert_secret("-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA\n-----END RSA PRIVATE KEY-----");
    }

    // 寧可放行：log 與程式碼是核心情境
    #[test]
    fn allows_log_lines() {
        assert_translates("2026-06-12 17:03:11 ERROR failed to connect to db: timeout after 30s");
        assert_translates("[WARN] retrying request (attempt 3/5) — upstream returned 503");
    }

    #[test]
    fn allows_code() {
        assert_translates("let result = client.translate(text).await?;");
        assert_translates("if (user.isAdmin && !ctx.dryRun) { applyChanges(); }");
    }

    #[test]
    fn allows_normal_words_with_digits() {
        assert_translates("Champions2024");
        assert_translates("The iPhone15 launch event starts at 10am.");
    }

    #[test]
    fn allows_hyphenated_words() {
        assert_translates("Pre-trained");
        assert_translates("state-of-the-art");
        assert_translates("sumi-translate");
    }

    #[test]
    fn allows_plain_sentences() {
        assert_translates("Hello, world. This is a normal sentence.");
        assert_translates("這是一段中文。");
    }

    // URL / 路徑：靜默不翻
    #[test]
    fn skips_pure_urls() {
        assert_eq!(classify("https://example.com/a?b=c"), Classification::UrlOrPath);
        assert_eq!(classify("www.example.com"), Classification::UrlOrPath);
    }

    #[test]
    fn skips_file_paths() {
        assert_eq!(classify("/Users/kim/Documents/report.pdf"), Classification::UrlOrPath);
        assert_eq!(classify("~/Desktop/notes.txt"), Classification::UrlOrPath);
        assert_eq!(classify("file:///tmp/a.log"), Classification::UrlOrPath);
    }

    #[test]
    fn sentence_containing_url_still_translates() {
        assert_translates("Check https://example.com for details.");
    }

    // 空值與截斷
    #[test]
    fn empty_input_is_empty() {
        assert_eq!(classify("   \n  "), Classification::Empty);
    }

    #[test]
    fn truncates_long_text() {
        let long: String = "字".repeat(MAX_TRANSLATE_CHARS + 500);
        match classify(&long) {
            Classification::Text { text, truncated } => {
                assert!(truncated);
                assert_eq!(text.chars().count(), MAX_TRANSLATE_CHARS);
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn short_text_not_truncated() {
        match classify("short text") {
            Classification::Text { truncated, .. } => assert!(!truncated),
            other => panic!("expected Text, got {other:?}"),
        }
    }
}
