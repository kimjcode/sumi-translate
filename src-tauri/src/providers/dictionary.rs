//! 真字典資料源：ECDICT 英漢（本地 SQLite，已簡轉繁/台灣用詞）。
//! 全本地、免 key、離線、不送任何東西出去（隱私）。只回事實性資料，不經 LLM（字典 ≠ LLM）。
//! SQLite 由 `npm run build:dict` 產生（見 scripts/build-dict.py），未進 git。

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

/// 送給前端的字典條目（沿用既有結構，前端不變）。
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

/// 查詢結果：含還原後的原形（lemma），給 session 快取當鍵用。
#[derive(Clone, Debug, Serialize)]
pub struct DictLookup {
    pub entry: Option<DictionaryEntry>,
    /// 還原後的原形（小寫）。查無時 = 小寫原字。
    pub lemma: String,
}

/// 唯讀開啟 ECDICT SQLite。
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
}

/// 查單字：先直接查 → 查無則用 `lemma` 表還原成原形再查（wakes/woke → wake）。
/// 一律回傳還原後的原形（lemma）；entry 查無回 `None`（交由上層走 Gemini fallback）。
pub fn lookup(conn: &Connection, word: &str) -> DictLookup {
    let key = word.trim().to_lowercase();
    if key.is_empty() {
        return DictLookup { entry: None, lemma: key };
    }
    // 1. 直接查（變化型若本身也是收錄字，這裡先命中）。
    if let Some(entry) = query_entry(conn, &key) {
        return DictLookup { entry: Some(entry), lemma: key };
    }
    // 2. 詞形還原：變化型 → 原形，用原形再查。
    if let Some(lemma) = query_lemma(conn, &key) {
        let lemma = lemma.to_lowercase();
        let entry = query_entry(conn, &lemma);
        return DictLookup { entry, lemma };
    }
    // 3. 查無：lemma = 原字（給快取鍵用）。
    DictLookup { entry: None, lemma: key }
}

fn query_entry(conn: &Connection, word_lower: &str) -> Option<DictionaryEntry> {
    let row = conn
        .query_row(
            "SELECT word, phonetic, translation FROM ecdict WHERE word_lower = ?1 LIMIT 1",
            [word_lower],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .ok()??;
    let (headword, phonetic, translation) = row;
    Some(DictionaryEntry {
        word: headword,
        phonetic: format_phonetic(&phonetic),
        meanings: parse_meanings(&translation),
    })
}

fn query_lemma(conn: &Connection, form: &str) -> Option<String> {
    conn.query_row(
        "SELECT word FROM lemma WHERE form = ?1 LIMIT 1",
        [form],
        |r| r.get::<_, String>(0),
    )
    .optional()
    .ok()?
}

fn format_phonetic(raw: &str) -> Option<String> {
    let p = raw.trim();
    if p.is_empty() {
        None
    } else {
        Some(format!("/{p}/"))
    }
}

/// 把 ECDICT 的 `translation` 欄拆成「詞性 + 釋義」。每行一個義項，行首若有詞性標記
/// （`n.`、`vt.`、`[計]`…）就抽出來。
fn parse_meanings(translation: &str) -> Vec<DictMeaning> {
    translation
        .split('\n')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|line| {
            let (pos, def) = split_pos(line);
            DictMeaning {
                part_of_speech: pos,
                definitions: vec![def],
            }
        })
        .collect()
}

/// 抽出行首詞性標記：`n.`/`vt.`/`adj.` 之類，或 `[計]`/`[醫]` 學科標記。
fn split_pos(line: &str) -> (String, String) {
    let bytes = line.as_bytes();
    // 學科標記 [..]
    if line.starts_with('[') {
        if let Some(end) = line.find(']') {
            let pos = line[..=end].to_string();
            let rest = line[end + 1..].trim_start().to_string();
            return (pos, rest);
        }
    }
    // 詞性縮寫：連續英文字母後接 '.'
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    if i > 0 && i < bytes.len() && bytes[i] == b'.' {
        let pos = line[..=i].to_string();
        let rest = line[i + 1..].trim_start().to_string();
        return (pos, rest);
    }
    (String::new(), line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_phonetic_with_slashes() {
        assert_eq!(format_phonetic("tɔr'nedo").as_deref(), Some("/tɔr'nedo/"));
        assert_eq!(format_phonetic("  ").as_deref(), None);
    }

    #[test]
    fn splits_pos_abbreviation() {
        assert_eq!(split_pos("n. 龍捲風"), ("n.".into(), "龍捲風".into()));
        assert_eq!(split_pos("vt. 探出"), ("vt.".into(), "探出".into()));
    }

    #[test]
    fn splits_subject_tag() {
        assert_eq!(split_pos("[計] 軟體"), ("[計]".into(), "軟體".into()));
    }

    #[test]
    fn no_pos_keeps_whole_line() {
        assert_eq!(split_pos("龍捲風"), (String::new(), "龍捲風".into()));
    }

    #[test]
    fn parses_multiline_translation() {
        let t = "n. 記憶, 回憶\nn. 記憶體\n[計] 儲存器";
        let m = parse_meanings(t);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].part_of_speech, "n.");
        assert_eq!(m[2].part_of_speech, "[計]");
        assert_eq!(m[2].definitions[0], "儲存器");
    }
}
