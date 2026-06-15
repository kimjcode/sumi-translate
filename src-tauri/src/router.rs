//! 語言/路由層：依設定（固定目標 / 語言配對）解析目標語言並翻譯。
//! 配對用「方案 A」：先翻成「我的語言」，偵測到來源就是我的語言時才回頭翻成對照語言。
//! 紅線：不在此 log 內容。

use serde::{Deserialize, Serialize};

use crate::providers::{self, Provider, ProviderError};
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LangMode {
    /// 永遠翻成固定的 `target_lang`（舊行為）。
    Fixed,
    /// 「我的語言 ⇄ 對照語言」雙向；其餘外語 fallback 到我的語言。
    Pairing,
}

impl Default for LangMode {
    fn default() -> Self {
        LangMode::Pairing
    }
}

/// 路由後的結果：含實際翻成的目標語言（給 UI 顯示用）。
#[derive(Clone)]
pub struct Routed {
    pub text: String,
    pub detected_source: Option<String>,
    pub target_lang: String,
}

/// 取語言碼的「基底語言」：去掉地區、轉小寫。`zh-TW`→`zh`、`EN-US`→`en`、`ZH`→`zh`。
fn base_lang(code: &str) -> String {
    code.split(['-', '_']).next().unwrap_or(code).to_lowercase()
}

/// 配對模式核心決策（純邏輯，可單測）：
/// 已先翻成「我的語言」並拿到偵測來源，是否該改翻成「對照語言」？
/// 規則：偵測到的來源 = 我的語言（基底相同）時 → true（要改翻對照語言）。
/// 其餘（對照語言 / 其他外語 / 偵測不到）→ false（維持我的語言，即 fallback）。
pub fn should_use_counterpart(
    detected: Option<&str>,
    my_lang: &str,
    counterpart: &str,
) -> bool {
    let my = base_lang(my_lang);
    let other = base_lang(counterpart);
    // 我的語言與對照語言基底相同（如 繁中⇄簡中）無法靠偵測分辨方向 → 不改翻，維持固定。
    if my == other {
        return false;
    }
    match detected {
        Some(code) => base_lang(code) == my,
        None => false, // 偵測不到 → fallback 到我的語言
    }
}

/// 依設定解析目標並翻譯。Glance 與 Workbench 共用此入口。
pub async fn translate_routed(
    settings: &Settings,
    provider: Provider,
    api_key: &str,
    client: &reqwest::Client,
    text: &str,
) -> Result<Routed, ProviderError> {
    match settings.lang_mode {
        LangMode::Fixed => {
            let t =
                providers::translate(provider, client, api_key, text, &settings.target_lang).await?;
            Ok(Routed {
                text: t.text,
                detected_source: t.detected_source,
                target_lang: settings.target_lang.clone(),
            })
        }
        LangMode::Pairing => {
            let my = &settings.my_lang;
            let counterpart = &settings.counterpart_lang;
            // 先翻成「我的語言」。
            let first = providers::translate(provider, client, api_key, text, my).await?;
            if should_use_counterpart(first.detected_source.as_deref(), my, counterpart) {
                // 來源就是我的語言 → 改翻成對照語言。
                let second =
                    providers::translate(provider, client, api_key, text, counterpart).await?;
                Ok(Routed {
                    text: second.text,
                    detected_source: first.detected_source,
                    target_lang: counterpart.clone(),
                })
            } else {
                Ok(Routed {
                    text: first.text,
                    detected_source: first.detected_source,
                    target_lang: my.clone(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 以 我的語言=zh-TW、對照語言=en 為例（任務驗收的範例）。
    const MY: &str = "zh-TW";
    const CP: &str = "en";

    #[test]
    fn source_is_counterpart_keeps_my_lang() {
        // 英文來源（已翻成繁中）→ 不改翻，維持繁中。
        assert!(!should_use_counterpart(Some("en"), MY, CP));
    }

    #[test]
    fn source_is_my_lang_switches_to_counterpart() {
        // 中文來源 → 改翻成英文。偵測碼可能是 zh / zh-CN / zh-TW，基底皆 zh。
        assert!(should_use_counterpart(Some("zh"), MY, CP));
        assert!(should_use_counterpart(Some("zh-CN"), MY, CP));
        assert!(should_use_counterpart(Some("zh-TW"), MY, CP));
        assert!(should_use_counterpart(Some("ZH"), MY, CP)); // DeepL 大寫
    }

    #[test]
    fn other_language_falls_back_to_my_lang() {
        // 日文 / 西語等 → 維持繁中（fallback）。
        assert!(!should_use_counterpart(Some("ja"), MY, CP));
        assert!(!should_use_counterpart(Some("es"), MY, CP));
    }

    #[test]
    fn undetected_falls_back_to_my_lang() {
        assert!(!should_use_counterpart(None, MY, CP));
    }

    #[test]
    fn reversed_pairing_is_symmetric() {
        // 我的語言=en、對照語言=zh-TW（英語母語者）。
        assert!(should_use_counterpart(Some("en"), "en", "zh-TW")); // 英文來源→改翻中文
        assert!(!should_use_counterpart(Some("zh"), "en", "zh-TW")); // 中文來源→維持英文
    }

    #[test]
    fn same_base_pairing_never_switches() {
        // 繁中⇄簡中：基底都 zh，無法靠偵測分辨 → 不改翻。
        assert!(!should_use_counterpart(Some("zh"), "zh-TW", "zh-CN"));
    }
}
