//! Workbench 服務層：展開、重翻（沿用過濾層 + MT）、字典查詢（ECDICT + 查無時單一 AI 字義）。
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
use crate::providers::{self, dictionary::DictLookup, ProviderError};
use crate::settings::SettingsState;
use crate::windows::{glance, workbench as wb_window};

pub const INPUT_EVENT: &str = "workbench://input";
// 字典查無 → 單一 AI 字義串流（字典卡只剩這一條 Gemini 路徑）。
pub const DEF_TOKEN_EVENT: &str = "workbench://def-token";
pub const DEF_DONE_EVENT: &str = "workbench://def-done";
pub const DEF_ERROR_EVENT: &str = "workbench://def-error";

/// AI 字義 fallback 的 503/429/網路錯誤重試次數（短退避）。
const DEF_MAX_RETRIES: usize = 2;

/// 串流的「兩次 chunk 之間」閒置上限。不設整體 timeout（避免腰斬長回應），
/// 但首 token 後若連線 stall，read_timeout 會把它轉成 Network 錯誤，而非永遠卡「串流中」。
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Serialize)]
pub struct WorkbenchInput {
    pub original: String,
    pub translated: String,
    pub target_lang: String,
}

pub struct WorkbenchState {
    input: Mutex<Option<WorkbenchInput>>,
    def_seq: AtomicU64,
    /// 重翻請求序號（M2）：連續編輯時丟棄慢回的舊請求，避免過時譯文蓋掉新的。
    mt_seq: AtomicU64,
    dict_client: reqwest::Client,
    llm_client: reqwest::Client,
    /// ECDICT SQLite 連線（唯讀），首次查詢時 lazy 開啟。
    ecdict: Mutex<Option<Connection>>,
}

impl WorkbenchState {
    pub fn new() -> Self {
        Self {
            input: Mutex::new(None),
            def_seq: AtomicU64::new(0),
            mt_seq: AtomicU64::new(0),
            dict_client: providers::http_client(),
            // 串流不能套整體 timeout（會在串到一半時被切），只留 connect timeout +
            // read_timeout（每次讀的閒置上限，讀到資料就重置）→ stall 時轉錯不無限掛住。
            llm_client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(5))
                .read_timeout(STREAM_IDLE_TIMEOUT)
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
    /// 已被更新的編輯取代（M2）：前端忽略，不回填，避免慢回的舊譯文蓋掉新的。
    Stale,
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

/// 空白 ⌘CC（這次沒有新複製）→ 開全空 Workbench 讓使用者自己打字。
/// 原文/譯文皆空，不帶任何上次剪貼簿內容（這正是要消除的困惑）。
pub fn open_blank(app: &AppHandle) {
    let s = app.state::<SettingsState>().snapshot();
    // 目標語言先給設定值；配對模式會在使用者打字、首次翻譯後反映解析方向。
    let target = match s.lang_mode {
        crate::router::LangMode::Fixed => s.target_lang,
        crate::router::LangMode::Pairing => s.my_lang,
    };
    open_with(app, String::new(), String::new(), target);
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
    // M2：每次重翻領一個序號；await 回來後若已非最新就回 Stale，前端丟棄。
    // 比照 Glance（pipeline 的 request_seq），避免連續編輯時較早送出、較晚回的請求蓋掉新譯文。
    let seq = app
        .state::<WorkbenchState>()
        .mt_seq
        .fetch_add(1, Ordering::SeqCst)
        + 1;
    let result = match filter::classify(&text) {
        filter::Classification::Empty => WbTranslation::Empty,
        filter::Classification::Secret => WbTranslation::Secret,
        // Workbench 是使用者主動編輯，URL/路徑也照翻（仍經機密過濾，紅線不變）。
        filter::Classification::UrlOrPath => run_mt(&app, text.trim().to_string(), false).await,
        filter::Classification::Text { text, truncated } => run_mt(&app, text, truncated).await,
    };
    if app.state::<WorkbenchState>().mt_seq.load(Ordering::SeqCst) != seq {
        return WbTranslation::Stale;
    }
    result
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

/// 503/429/網路錯誤可重試（且必須是「還沒串出任何 token」才安全重試）。
fn is_retryable(e: &ProviderError) -> bool {
    matches!(
        e,
        ProviderError::Network(_) | ProviderError::Api { status: 503 | 429, .. }
    )
}

/// 在 blocking 執行緒池上短退避，不卡 async worker（避免引入 tokio::time 這個依賴）。
async fn backoff(attempt: usize) {
    let ms = 400u64 * (attempt as u64); // 400ms, 800ms…
    let _ = tauri::async_runtime::spawn_blocking(move || {
        std::thread::sleep(Duration::from_millis(ms));
    })
    .await;
}

/// 字典查無 → 單一 AI 字義串流（字典卡唯一的 Gemini 路徑）。token 經 `workbench://def-*`。
/// 503/429/網路錯誤會自動短退避重試（≤2 次，且僅在尚未串出 token 時），仍失敗才回友善訊息。
#[tauri::command]
pub fn gemini_define(app: AppHandle, word: String, sentence: String, target_lang: String) -> u64 {
    let state = app.state::<WorkbenchState>();
    let seq = state.def_seq.fetch_add(1, Ordering::SeqCst) + 1;

    // 紅線（H2）：字典 fallback 也是一條送外部 API 的出口，必須過機密過濾。
    // 命中 Secret（如貼進含 `api_key=…`／私鑰的設定/log）→ 不送 Gemini，回「已略過」。
    if matches!(filter::classify(&sentence), filter::Classification::Secret) {
        log::info!("AI define: sentence looks like a secret, blocked from sending");
        let _ = app.emit_to(
            wb_window::WORKBENCH_LABEL,
            DEF_ERROR_EVENT,
            LlmEvent::Error {
                seq,
                message: "已略過可能的機密內容".into(),
            },
        );
        return seq;
    }

    let Some(api_key) = app.state::<SettingsState>().llm_api_key() else {
        let _ = app.emit_to(
            wb_window::WORKBENCH_LABEL,
            DEF_ERROR_EVENT,
            LlmEvent::Error {
                seq,
                message: "尚未設定 Gemini API key — 到設定視窗貼上即可".into(),
            },
        );
        return seq;
    };

    let client = state.llm_client.clone();
    let prompt = build_define_prompt(&word, &sentence, &target_lang);
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut attempt = 0;
        let result = loop {
            let mut emitted = false;
            let r = providers::llm::stream_generate(&client, &api_key, &prompt, |delta| {
                emitted = true;
                if app2.state::<WorkbenchState>().def_seq.load(Ordering::SeqCst) != seq {
                    return false; // 已被新查詢取代 → 停止
                }
                let _ = app2.emit_to(
                    wb_window::WORKBENCH_LABEL,
                    DEF_TOKEN_EVENT,
                    LlmEvent::Token { seq, delta: delta.to_string() },
                );
                true
            })
            .await;

            match &r {
                // 只在「尚未串出 token」時重試，避免重複輸出。
                Err(e) if !emitted && attempt < DEF_MAX_RETRIES && is_retryable(e) => {
                    attempt += 1;
                    log::warn!("AI define retry {attempt} (retryable error)");
                    backoff(attempt).await;
                    continue;
                }
                _ => break r,
            }
        };

        if app2.state::<WorkbenchState>().def_seq.load(Ordering::SeqCst) != seq {
            return; // 已被取代，不發 done/error
        }
        match result {
            Ok(()) => {
                let _ = app2.emit_to(wb_window::WORKBENCH_LABEL, DEF_DONE_EVENT, LlmEvent::Done { seq });
            }
            Err(e) => {
                // 診斷：API 錯誤訊息（非使用者內容）；給前端的是友善繁中訊息，不露原始英文。
                log::warn!("AI define failed: {e:?}");
                let _ = app2.emit_to(
                    wb_window::WORKBENCH_LABEL,
                    DEF_ERROR_EVENT,
                    LlmEvent::Error {
                        seq,
                        // 字典卡已標示「AI 字義 · Gemini」；訊息用 Gemini 名稱，
                        // 認證類才會讀成「Gemini API key 無效 — 請到設定檢查或重新貼上」。
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

/// 字典查無時的 AI 字義 prompt：要求一行字典式釋義（含詞性），台灣用語。
fn build_define_prompt(word: &str, sentence: &str, target_lang: &str) -> String {
    let lang = lang_name(target_lang);
    format!(
        "你是英漢字典。用{lang}（台灣用語）給出單字「{word}」在這個句子裡最貼切的簡短意思，\
         像字典釋義一行（可含詞性縮寫，如 n./v./adj.），不要例句、不要文法說明、不要客套。\n\
         句子：{sentence}"
    )
}
