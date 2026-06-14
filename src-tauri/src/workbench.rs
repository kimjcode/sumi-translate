//! Workbench 服務層：展開、重翻（沿用過濾層 + MT）、字典查詢、Gemini 文法串流。
//! 紅線：不 log 原文/剪貼簿內容；機密內容不送出；key 只在 Keychain。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::monitor::filter;
use crate::providers::{self, dictionary::DictionaryEntry};
use crate::settings::SettingsState;
use crate::windows::{glance, workbench as wb_window};

pub const INPUT_EVENT: &str = "workbench://input";
pub const LLM_TOKEN_EVENT: &str = "workbench://llm-token";
pub const LLM_DONE_EVENT: &str = "workbench://llm-done";
pub const LLM_ERROR_EVENT: &str = "workbench://llm-error";

#[derive(Clone, Serialize)]
pub struct WorkbenchInput {
    pub original: String,
    pub translated: String,
    pub target_lang: String,
}

pub struct WorkbenchState {
    input: Mutex<Option<WorkbenchInput>>,
    llm_seq: AtomicU64,
    dict_client: reqwest::Client,
    llm_client: reqwest::Client,
}

impl WorkbenchState {
    pub fn new() -> Self {
        Self {
            input: Mutex::new(None),
            llm_seq: AtomicU64::new(0),
            dict_client: providers::http_client(),
            // 串流不能套整體 timeout（會在串到一半時被切），只留 connect timeout。
            llm_client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build LLM HTTP client"),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WbTranslation {
    Ok {
        translated: String,
        detected_source: Option<String>,
        truncated: bool,
    },
    /// 疑似機密 → 不送出（沿用 P0 過濾層紅線）。
    Secret,
    Empty,
    Error {
        message: String,
    },
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LlmEvent {
    Token { seq: u64, delta: String },
    Done { seq: u64 },
    Error { seq: u64, message: String },
}

// ── Commands ───────────────────────────────────────────────────────────────

/// Glance「展開」→ 帶入內容、關 Glance、開 Workbench。
#[tauri::command]
pub fn open_workbench(
    app: AppHandle,
    state: tauri::State<'_, WorkbenchState>,
    original: String,
    translated: String,
    target_lang: String,
) {
    let input = WorkbenchInput {
        original,
        translated,
        target_lang,
    };
    *state.input.lock().expect("workbench input poisoned") = Some(input.clone());
    glance::hide(&app);
    wb_window::show(&app);
    // 視窗在啟動時就掛載、之後只 show/hide（React 不重新掛載），所以每次展開都推事件，
    // 讓前端更新內容並清掉殘留的單字卡。
    let _ = app.emit_to(wb_window::WORKBENCH_LABEL, INPUT_EVENT, input);
}

/// Workbench 前端掛載時讀取帶入的原文 / 譯文。
#[tauri::command]
pub fn get_workbench_input(state: tauri::State<'_, WorkbenchState>) -> Option<WorkbenchInput> {
    state.input.lock().expect("workbench input poisoned").clone()
}

#[tauri::command]
pub fn close_workbench(app: AppHandle) {
    wb_window::hide(&app);
}

/// 編輯原文後重翻：沿用 P0 過濾層（機密內容仍不送出），走既有 MT provider。
#[tauri::command]
pub async fn workbench_translate(
    app: AppHandle,
    text: String,
) -> WbTranslation {
    match filter::classify(&text) {
        filter::Classification::Empty => WbTranslation::Empty,
        filter::Classification::Secret => WbTranslation::Secret,
        // Workbench 是使用者主動編輯，URL/路徑也照翻（仍經機密過濾，紅線不變）。
        filter::Classification::UrlOrPath => {
            run_mt(&app, text.trim().to_string(), false).await
        }
        filter::Classification::Text { text, truncated } => run_mt(&app, text, truncated).await,
    }
}

async fn run_mt(app: &AppHandle, text: String, truncated: bool) -> WbTranslation {
    let settings = app.state::<SettingsState>();
    let snapshot = settings.snapshot();
    let provider = snapshot.provider;
    let Some(api_key) = settings.api_key(provider) else {
        return WbTranslation::Error {
            message: providers::ProviderError::MissingKey.user_message(provider),
        };
    };
    let client = app.state::<WorkbenchState>().dict_client.clone();
    match providers::translate(provider, &client, &api_key, &text, &snapshot.target_lang).await {
        Ok(t) => {
            log::info!(
                "workbench re-translated {} chars via {}",
                text.chars().count(),
                provider.display_name()
            );
            WbTranslation::Ok {
                translated: t.text,
                detected_source: t.detected_source,
                truncated,
            }
        }
        Err(e) => WbTranslation::Error {
            message: e.user_message(provider),
        },
    }
}

/// 點字 → 真字典（第一段，非 LLM）。查無此字回 null。
#[tauri::command]
pub async fn dictionary_lookup(
    app: AppHandle,
    word: String,
) -> Result<Option<DictionaryEntry>, String> {
    let client = app.state::<WorkbenchState>().dict_client.clone();
    providers::dictionary::lookup(&client, &word)
        .await
        .map_err(|e| e.user_message_named("字典"))
}

/// 點字 → Gemini 文法/語境（第二段，LLM，真串流）。回傳 request id；
/// token 透過 `workbench://llm-*` 事件串給前端。
#[tauri::command]
pub fn gemini_explain(
    app: AppHandle,
    word: String,
    sentence: String,
    target_lang: String,
) -> u64 {
    let state = app.state::<WorkbenchState>();
    let seq = state.llm_seq.fetch_add(1, Ordering::SeqCst) + 1;

    let Some(api_key) = app.state::<SettingsState>().llm_api_key() else {
        let _ = app.emit_to(
            wb_window::WORKBENCH_LABEL,
            LLM_ERROR_EVENT,
            LlmEvent::Error {
                seq,
                message: "尚未設定 Gemini API key — 到設定視窗貼上即可".into(),
            },
        );
        return seq;
    };

    let client = state.llm_client.clone();
    let prompt = build_explain_prompt(&word, &sentence, &target_lang);
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let wb = app2.state::<WorkbenchState>();
        let result = providers::llm::stream_generate(&client, &api_key, &prompt, |delta| {
            // 被新的查詢取代就停止這條串流。
            if wb.llm_seq.load(Ordering::SeqCst) != seq {
                return false;
            }
            let _ = app2.emit_to(
                wb_window::WORKBENCH_LABEL,
                LLM_TOKEN_EVENT,
                LlmEvent::Token {
                    seq,
                    delta: delta.to_string(),
                },
            );
            true
        })
        .await;

        if app2.state::<WorkbenchState>().llm_seq.load(Ordering::SeqCst) != seq {
            return; // 已被取代，不發 done/error
        }
        match result {
            Ok(()) => {
                let _ = app2.emit_to(wb_window::WORKBENCH_LABEL, LLM_DONE_EVENT, LlmEvent::Done { seq });
            }
            Err(e) => {
                // 診斷：API 錯誤訊息（如 404 列出可用 model），非使用者內容。
                log::warn!("gemini stream failed: {e:?}");
                let _ = app2.emit_to(
                    wb_window::WORKBENCH_LABEL,
                    LLM_ERROR_EVENT,
                    LlmEvent::Error {
                        seq,
                        message: e.user_message_named("Gemini"),
                    },
                );
            }
        }
    });

    seq
}

fn lang_name(code: &str) -> &str {
    match code {
        "zh-TW" => "繁體中文",
        "zh-CN" => "简体中文",
        "en" => "English",
        "ja" => "日本語",
        "ko" => "한국어",
        _ => "繁體中文",
    }
}

fn build_explain_prompt(word: &str, sentence: &str, target_lang: &str) -> String {
    let lang = lang_name(target_lang);
    format!(
        "你是英文教學助理。請用{lang}針對句子中的指定單字，簡潔說明：\n\
         1. 它在這個句子裡的詞性與用法\n\
         2. 語境／語感（這裡為什麼用它、有什麼細微差別）\n\
         3. 一個更自然或更精確的改寫建議（如果有；沒有就說目前已恰當）\n\n\
         句子：{sentence}\n\
         指定單字：{word}\n\n\
         用簡短分點，聚焦文法與語境，不要重複字典定義。"
    )
}
