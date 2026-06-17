//! 語言/路由層：觸發 → 過濾 → 快取/翻譯 → 餵給 Glance 浮窗。
//! 紅線：機密內容永不送出；任何 log 不含剪貼簿內容。

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::monitor::{filter, pasteboard};
use crate::providers::{self, Provider};
use crate::router::{self, Routed};
use crate::settings::{Settings, SettingsState};
use crate::windows::glance;

pub const STATE_EVENT: &str = "glance://state";
const CACHE_CAP: usize = 50;

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GlanceMsg {
    Loading {
        original: String,
        truncated: bool,
        target_lang: String,
        provider: String,
    },
    Result {
        original: String,
        translated: String,
        detected_source: Option<String>,
        truncated: bool,
        target_lang: String,
        provider: String,
    },
    Secret,
    Error {
        message: String,
    },
}

pub struct PipelineState {
    request_seq: AtomicU64,
    cache: Mutex<Vec<(u64, Routed)>>,
    client: reqwest::Client,
}

impl PipelineState {
    pub fn new() -> Self {
        Self {
            request_seq: AtomicU64::new(0),
            cache: Mutex::new(Vec::new()),
            client: providers::http_client(),
        }
    }

    fn cache_get(&self, key: u64) -> Option<Routed> {
        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        let idx = cache.iter().position(|(k, _)| *k == key)?;
        let entry = cache.remove(idx);
        let value = entry.1.clone();
        cache.push(entry); // 移到最近使用端
        Some(value)
    }

    fn cache_put(&self, key: u64, value: Routed) {
        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        cache.retain(|(k, _)| *k != key);
        cache.push((key, value));
        if cache.len() > CACHE_CAP {
            cache.remove(0);
        }
    }
}

/// 快取鍵：把「決定路由結果的設定」納入（同一段文字在不同配對/目標下結果不同）。
fn cache_key(provider: Provider, routing_sig: &str, text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    provider.hash(&mut h);
    routing_sig.hash(&mut h);
    text.hash(&mut h);
    h.finish()
}

/// 代表當下路由設定的字串簽章，用於快取鍵。
fn routing_signature(settings: &Settings) -> String {
    match settings.lang_mode {
        router::LangMode::Fixed => format!("fixed:{}", settings.target_lang),
        router::LangMode::Pairing => {
            format!("pair:{}>{}", settings.my_lang, settings.counterpart_lang)
        }
    }
}

/// 「有新複製 → 翻譯」的主執行緒處理（讀剪貼簿 → 過濾 → Glance）。
/// 由 monitor 的雙擊分流在主執行緒呼叫（NSPasteboard / 視窗操作都要在主執行緒）。
pub(crate) fn handle_on_main(app: AppHandle) {
    if pasteboard::has_file_url() {
        log::info!("clipboard contains file URL(s), ignoring trigger");
        return;
    }
    let Some(raw) = read_clipboard_text() else {
        log::info!("clipboard has no usable text, ignoring trigger");
        return;
    };

    match filter::classify(&raw) {
        filter::Classification::Empty => {}
        filter::Classification::UrlOrPath => {
            log::info!("clipboard is a pure URL/path, skipping");
        }
        filter::Classification::Secret => {
            // 紅線：內容永不送出、永不進 log、也不顯示原文。
            log::info!("clipboard looks like a secret, blocked from sending");
            show(&app, GlanceMsg::Secret);
        }
        filter::Classification::Text { text, truncated } => {
            translate_and_show(&app, text, truncated);
        }
    }
}

fn show(app: &AppHandle, msg: GlanceMsg) {
    // 只有 Result 可展開到 Workbench；其餘狀態清掉，避免 ⌘↩ 展開到舊內容。
    match &msg {
        GlanceMsg::Result {
            original,
            translated,
            target_lang,
            ..
        } => glance::set_expandable(app, original.clone(), translated.clone(), target_lang.clone()),
        _ => glance::clear_expandable(app),
    }
    let _ = app.emit_to(glance::GLANCE_LABEL, STATE_EVENT, msg);
    glance::show_at_cursor(app);
}

fn translate_and_show(app: &AppHandle, text: String, truncated: bool) {
    let settings = app.state::<SettingsState>().snapshot();
    let provider = settings.provider;
    let state = app.state::<PipelineState>();

    let sig = routing_signature(&settings);
    let key = cache_key(provider, &sig, &text);
    if let Some(hit) = state.cache_get(key) {
        log::info!("cache hit, showing stored translation");
        show(
            app,
            GlanceMsg::Result {
                original: text,
                translated: hit.text,
                detected_source: hit.detected_source,
                truncated,
                target_lang: hit.target_lang,
                provider: provider.display_name().into(),
            },
        );
        return;
    }

    // 取 API key（走記憶體快取，第一次才讀 Keychain）。沒 key 直接顯示提示，不進 loading。
    let Some(api_key) = app.state::<SettingsState>().api_key(provider) else {
        show(
            app,
            GlanceMsg::Error {
                message: providers::ProviderError::MissingKey.user_message(provider),
            },
        );
        return;
    };

    // 配對模式下實際目標要等偵測才知道；loading 標籤先用「我的語言」作暫顯（過場用）。
    let loading_target = match settings.lang_mode {
        router::LangMode::Fixed => settings.target_lang.clone(),
        router::LangMode::Pairing => settings.my_lang.clone(),
    };
    show(
        app,
        GlanceMsg::Loading {
            original: text.clone(),
            truncated,
            target_lang: loading_target,
            provider: provider.display_name().into(),
        },
    );

    let request_id = state.request_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let client = state.client.clone();
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let outcome = run_translation(&settings, provider, &client, &api_key, &text).await;
        let state = app2.state::<PipelineState>();
        if state.request_seq.load(Ordering::SeqCst) != request_id {
            return; // 已有更新的觸發，丟棄這筆結果
        }
        match outcome {
            Ok(routed) => {
                state.cache_put(key, routed.clone());
                // 使用者已按 Esc 關掉就不要再彈回來；結果已入快取。
                if !glance::is_visible(&app2) {
                    return;
                }
                // ⌘↩ 展開用：記下當下可展開內容（含解析後的目標語言）。
                glance::set_expandable(
                    &app2,
                    text.clone(),
                    routed.text.clone(),
                    routed.target_lang.clone(),
                );
                let _ = app2.emit_to(
                    glance::GLANCE_LABEL,
                    STATE_EVENT,
                    GlanceMsg::Result {
                        original: text,
                        translated: routed.text,
                        detected_source: routed.detected_source,
                        truncated,
                        target_lang: routed.target_lang,
                        provider: provider.display_name().into(),
                    },
                );
                glance::touch_idle(&app2);
            }
            Err(message) => {
                if !glance::is_visible(&app2) {
                    return;
                }
                let _ = app2.emit_to(glance::GLANCE_LABEL, STATE_EVENT, GlanceMsg::Error { message });
                glance::touch_idle(&app2);
            }
        }
    });
}

async fn run_translation(
    settings: &Settings,
    provider: Provider,
    client: &reqwest::Client,
    api_key: &str,
    text: &str,
) -> Result<Routed, String> {
    // 診斷用：記 provider / 模式 / 字元數，不記內容（紅線）。
    log::info!(
        "translating via {} (mode={:?}, {} chars)",
        provider.display_name(),
        settings.lang_mode,
        text.chars().count()
    );
    let started = Instant::now();
    match router::translate_routed(settings, provider, api_key, client, text).await {
        Ok(routed) => {
            // 紅線：只 log 統計值，不 log 內容。
            log::info!(
                "{} translated {} chars → {} in {}ms",
                provider.display_name(),
                text.chars().count(),
                routed.target_lang,
                started.elapsed().as_millis()
            );
            Ok(routed)
        }
        Err(e) => {
            log::warn!(
                "{} translation failed after {}ms: {}",
                provider.display_name(),
                started.elapsed().as_millis(),
                match &e {
                    providers::ProviderError::MissingKey => "missing key".into(),
                    providers::ProviderError::Network(msg) => format!("network error: {msg}"),
                    // API 錯誤訊息來自服務端回應（如「Invalid Value」），不含剪貼簿原文。
                    providers::ProviderError::Api { status, message } => {
                        format!("HTTP {status}: {message}")
                    }
                    providers::ProviderError::Parse(msg) => format!("parse error: {msg}"),
                }
            );
            Err(e.user_message(provider))
        }
    }
}

fn read_clipboard_text() -> Option<String> {
    let mut clipboard = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            log::error!("failed to access clipboard: {e}");
            return None;
        }
    };
    match clipboard.get_text() {
        Ok(t) if !t.trim().is_empty() => Some(t),
        // get_text() 對圖片/空剪貼簿會回 Err，一律視為無內容。
        _ => None,
    }
}
