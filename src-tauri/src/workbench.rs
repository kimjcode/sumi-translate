//! Workbench 服務層：展開、重翻（沿用過濾層 + MT）、字典查詢、Gemini 文法串流。
//! 紅線：不 log 原文/剪貼簿內容；機密內容不送出；key 只在 Keychain。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use rusqlite::Connection;
use serde::Serialize;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager};

use crate::monitor::filter;
use crate::providers::{self, dictionary::DictLookup};
use crate::settings::SettingsState;
use crate::windows::{glance, workbench as wb_window};

pub const INPUT_EVENT: &str = "workbench://input";
pub const LLM_TOKEN_EVENT: &str = "workbench://llm-token";
pub const LLM_DONE_EVENT: &str = "workbench://llm-done";
pub const LLM_ERROR_EVENT: &str = "workbench://llm-error";
// 上段字典查無 → Gemini 短釋義補充（與下段文法分開的事件通道）。
pub const DEF_TOKEN_EVENT: &str = "workbench://def-token";
pub const DEF_DONE_EVENT: &str = "workbench://def-done";
pub const DEF_ERROR_EVENT: &str = "workbench://def-error";

#[derive(Clone, Serialize)]
pub struct WorkbenchInput {
    pub original: String,
    pub translated: String,
    pub target_lang: String,
}

pub struct WorkbenchState {
    input: Mutex<Option<WorkbenchInput>>,
    llm_seq: AtomicU64,
    def_seq: AtomicU64,
    dict_client: reqwest::Client,
    llm_client: reqwest::Client,
    /// ECDICT SQLite 連線（唯讀），首次查詢時 lazy 開啟。
    ecdict: Mutex<Option<Connection>>,
}

impl WorkbenchState {
    pub fn new() -> Self {
        Self {
            input: Mutex::new(None),
            llm_seq: AtomicU64::new(0),
            def_seq: AtomicU64::new(0),
            dict_client: providers::http_client(),
            // 串流不能套整體 timeout（會在串到一半時被切），只留 connect timeout。
            llm_client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("failed to build LLM HTTP client"),
            ecdict: Mutex::new(None),
        }
    }
}

/// 解析 ECDICT SQLite 路徑：先打包資源目錄，dev 時 fallback 到原始碼樹。
fn ecdict_path(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = app.path().resolve("resources/ecdict.sqlite", BaseDirectory::Resource) {
        if p.exists() {
            return Some(p);
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/ecdict.sqlite");
    dev.exists().then_some(dev)
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WbTranslation {
    Ok {
        translated: String,
        detected_source: Option<String>,
        truncated: bool,
        /// 解析後實際翻成的目標語言（配對模式下由路由決定）。
        target_lang: String,
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

/// 帶入內容、關 Glance、開 Workbench。前端按鈕與全域 ⌘↩ 共用此路徑。
fn open_with(app: &AppHandle, original: String, translated: String, target_lang: String) {
    let input = WorkbenchInput {
        original,
        translated,
        target_lang,
    };
    *app.state::<WorkbenchState>()
        .input
        .lock()
        .expect("workbench input poisoned") = Some(input.clone());
    glance::hide(app);
    wb_window::show(app);
    // 視窗在啟動時就掛載、之後只 show/hide（React 不重新掛載），所以每次展開都推事件，
    // 讓前端更新內容並清掉殘留的單字卡。
    let _ = app.emit_to(wb_window::WORKBENCH_LABEL, INPUT_EVENT, input);
}

/// Glance「展開」鈕（前端滑鼠點擊）→ 帶入內容開 Workbench。
#[tauri::command]
pub fn open_workbench(app: AppHandle, original: String, translated: String, target_lang: String) {
    open_with(&app, original, translated, target_lang);
}

/// 全域 ⌘↩（在 event tap 中觸發，因浮窗拿不到鍵盤焦點）→ 用當下 Glance 內容展開。
pub fn expand_from_glance(app: &AppHandle) {
    if let Some(e) = glance::take_expandable(app) {
        open_with(app, e.original, e.translated, e.target_lang);
    }
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
    // 走路由層：配對模式會自動解析方向（與 Glance 一致）。
    match crate::router::translate_routed(&snapshot, provider, &api_key, &client, &text).await {
        Ok(routed) => {
            log::info!(
                "workbench re-translated {} chars via {} → {}",
                text.chars().count(),
                provider.display_name(),
                routed.target_lang
            );
            WbTranslation::Ok {
                translated: routed.text,
                detected_source: routed.detected_source,
                truncated,
                target_lang: routed.target_lang,
            }
        }
        Err(e) => WbTranslation::Error {
            message: e.user_message(provider),
        },
    }
}

/// 點字 → 真字典（第一段，ECDICT 本地英漢，非 LLM）。回傳含「還原後原形(lemma)」+ entry。
/// 查無 entry 由前端走 Gemini fallback。全本地、不送任何東西出去（隱私）。
#[tauri::command]
pub fn dictionary_lookup(app: AppHandle, word: String) -> DictLookup {
    let wb = app.state::<WorkbenchState>();
    let mut guard = wb.ecdict.lock().expect("ecdict mutex poisoned");
    if guard.is_none() {
        match ecdict_path(&app).and_then(|p| providers::dictionary::open(&p).ok()) {
            Some(conn) => *guard = Some(conn),
            None => {
                log::warn!("ecdict.sqlite 找不到——請先跑 `npm run build:dict`");
                return DictLookup {
                    entry: None,
                    lemma: word.trim().to_lowercase(),
                };
            }
        }
    }
    providers::dictionary::lookup(guard.as_ref().unwrap(), &word)
}

/// 兩條 Gemini 串流通道：下段文法 / 上段字典查無補充。
#[derive(Clone, Copy)]
enum Channel {
    Grammar,
    Define,
}

impl Channel {
    fn events(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Channel::Grammar => (LLM_TOKEN_EVENT, LLM_DONE_EVENT, LLM_ERROR_EVENT),
            Channel::Define => (DEF_TOKEN_EVENT, DEF_DONE_EVENT, DEF_ERROR_EVENT),
        }
    }
}

fn current_seq(wb: &WorkbenchState, channel: Channel) -> u64 {
    match channel {
        Channel::Grammar => wb.llm_seq.load(Ordering::SeqCst),
        Channel::Define => wb.def_seq.load(Ordering::SeqCst),
    }
}

/// 共用：起一條 Gemini 串流，token 經對應事件通道串給前端；回傳 request id（供取消比對）。
fn run_gemini_stream(app: AppHandle, prompt: String, channel: Channel) -> u64 {
    let state = app.state::<WorkbenchState>();
    let seq = match channel {
        Channel::Grammar => state.llm_seq.fetch_add(1, Ordering::SeqCst) + 1,
        Channel::Define => state.def_seq.fetch_add(1, Ordering::SeqCst) + 1,
    };
    let (tok_event, done_event, err_event) = channel.events();

    let Some(api_key) = app.state::<SettingsState>().llm_api_key() else {
        let _ = app.emit_to(
            wb_window::WORKBENCH_LABEL,
            err_event,
            LlmEvent::Error {
                seq,
                message: "尚未設定 Gemini API key — 到設定視窗貼上即可".into(),
            },
        );
        return seq;
    };

    let client = state.llm_client.clone();
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let result = providers::llm::stream_generate(&client, &api_key, &prompt, |delta| {
            // 被新的查詢取代就停止這條串流。
            if current_seq(&app2.state::<WorkbenchState>(), channel) != seq {
                return false;
            }
            let _ = app2.emit_to(
                wb_window::WORKBENCH_LABEL,
                tok_event,
                LlmEvent::Token {
                    seq,
                    delta: delta.to_string(),
                },
            );
            true
        })
        .await;

        if current_seq(&app2.state::<WorkbenchState>(), channel) != seq {
            return; // 已被取代，不發 done/error
        }
        match result {
            Ok(()) => {
                let _ = app2.emit_to(wb_window::WORKBENCH_LABEL, done_event, LlmEvent::Done { seq });
            }
            Err(e) => {
                // 診斷：API 錯誤訊息（如 404 列出可用 model），非使用者內容。
                log::warn!("gemini stream failed: {e:?}");
                let _ = app2.emit_to(
                    wb_window::WORKBENCH_LABEL,
                    err_event,
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

/// 點字 → Gemini 文法/語境（下段，LLM，真串流）。token 經 `workbench://llm-*`。
#[tauri::command]
pub fn gemini_explain(app: AppHandle, word: String, sentence: String, target_lang: String) -> u64 {
    let prompt = build_explain_prompt(&word, &sentence, &target_lang);
    run_gemini_stream(app, prompt, Channel::Grammar)
}

/// 字典查無 → Gemini 短中文釋義補充（上段，標明是 LLM）。token 經 `workbench://def-*`。
#[tauri::command]
pub fn gemini_define(app: AppHandle, word: String, sentence: String, target_lang: String) -> u64 {
    let prompt = build_define_prompt(&word, &sentence, &target_lang);
    run_gemini_stream(app, prompt, Channel::Define)
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

/// 字典查無時的補充釋義 prompt：要求一行字典式釋義（含詞性），台灣用語。
fn build_define_prompt(word: &str, sentence: &str, target_lang: &str) -> String {
    let lang = lang_name(target_lang);
    format!(
        "你是英漢字典。用{lang}（台灣用語）給出單字「{word}」在這個句子裡最貼切的簡短意思，\
         像字典釋義一行（可含詞性縮寫，如 n./v./adj.），不要例句、不要文法說明、不要客套。\n\
         句子：{sentence}"
    )
}
