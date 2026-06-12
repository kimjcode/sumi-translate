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
use crate::providers::{self, Provider, Translation};
use crate::settings::{self, SettingsState};
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
    cache: Mutex<Vec<(u64, Translation)>>,
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

    fn cache_get(&self, key: u64) -> Option<Translation> {
        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        let idx = cache.iter().position(|(k, _)| *k == key)?;
        let entry = cache.remove(idx);
        let value = entry.1.clone();
        cache.push(entry); // 移到最近使用端
        Some(value)
    }

    fn cache_put(&self, key: u64, value: Translation) {
        let mut cache = self.cache.lock().expect("cache mutex poisoned");
        cache.retain(|(k, _)| *k != key);
        cache.push((key, value));
        if cache.len() > CACHE_CAP {
            cache.remove(0);
        }
    }
}

fn cache_key(provider: Provider, target_lang: &str, text: &str) -> u64 {
    let mut h = DefaultHasher::new();
    provider.hash(&mut h);
    target_lang.hash(&mut h);
    text.hash(&mut h);
    h.finish()
}

/// 雙擊觸發入口。可從任意執行緒（event tap）呼叫；實際處理排到主執行緒
/// （NSPasteboard / 視窗操作都要在主執行緒）。
pub fn trigger(app: &AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || handle_on_main(app2));
}

fn handle_on_main(app: AppHandle) {
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
    let _ = app.emit_to(glance::GLANCE_LABEL, STATE_EVENT, msg);
    glance::show_at_cursor(app);
}

fn translate_and_show(app: &AppHandle, text: String, truncated: bool) {
    let settings = app.state::<SettingsState>().snapshot();
    let provider = settings.provider;
    let target_lang = settings.target_lang.clone();
    let state = app.state::<PipelineState>();

    let key = cache_key(provider, &target_lang, &text);
    if let Some(hit) = state.cache_get(key) {
        log::info!("cache hit, showing stored translation");
        show(
            app,
            GlanceMsg::Result {
                original: text,
                translated: hit.text,
                detected_source: hit.detected_source,
                truncated,
                target_lang,
                provider: provider.display_name().into(),
            },
        );
        return;
    }

    show(
        app,
        GlanceMsg::Loading {
            original: text.clone(),
            truncated,
            target_lang: target_lang.clone(),
            provider: provider.display_name().into(),
        },
    );

    let request_id = state.request_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let client = state.client.clone();
    let app2 = app.clone();

    tauri::async_runtime::spawn(async move {
        let outcome = run_translation(provider, &client, &text, &target_lang).await;
        let state = app2.state::<PipelineState>();
        if state.request_seq.load(Ordering::SeqCst) != request_id {
            return; // 已有更新的觸發，丟棄這筆結果
        }
        match outcome {
            Ok(translation) => {
                state.cache_put(key, translation.clone());
                // 使用者已按 Esc 關掉就不要再彈回來；結果已入快取。
                if !glance::is_visible(&app2) {
                    return;
                }
                let _ = app2.emit_to(
                    glance::GLANCE_LABEL,
                    STATE_EVENT,
                    GlanceMsg::Result {
                        original: text,
                        translated: translation.text,
                        detected_source: translation.detected_source,
                        truncated,
                        target_lang,
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
    provider: Provider,
    client: &reqwest::Client,
    text: &str,
    target_lang: &str,
) -> Result<Translation, String> {
    let Some(api_key) = settings::api_key(provider) else {
        return Err(providers::ProviderError::MissingKey.user_message(provider));
    };
    let started = Instant::now();
    match providers::translate(provider, client, &api_key, text, target_lang).await {
        Ok(translation) => {
            // 紅線：只 log 統計值，不 log 內容。
            log::info!(
                "{} translated {} chars in {}ms",
                provider.display_name(),
                text.chars().count(),
                started.elapsed().as_millis()
            );
            Ok(translation)
        }
        Err(e) => {
            log::warn!(
                "{} translation failed after {}ms: {}",
                provider.display_name(),
                started.elapsed().as_millis(),
                match &e {
                    providers::ProviderError::MissingKey => "missing key".into(),
                    providers::ProviderError::Network(_) => "network error".into(),
                    providers::ProviderError::Api { status, .. } => format!("HTTP {status}"),
                    providers::ProviderError::Parse(_) => "parse error".into(),
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
