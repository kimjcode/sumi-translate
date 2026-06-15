//! 設定層：偏好存 app config 目錄 JSON；API key 一律存 macOS Keychain（紅線）。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::providers::Provider;

const KEYCHAIN_SERVICE: &str = "com.kimj.sumi";
const LLM_ACCOUNT: &str = "gemini_api_key";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: Provider,
    /// 語言模式：固定目標 / 語言配對。
    pub lang_mode: crate::router::LangMode,
    /// 固定模式的目標語言（舊行為）。
    pub target_lang: String,
    /// 配對模式：我的語言（A）。
    pub my_lang: String,
    /// 配對模式：對照語言（B）。
    pub counterpart_lang: String,
    pub double_press_ms: u64,
    pub idle_close_ms: u64,
    pub always_on_monitor: bool,
    // 非機密的「key 是否已設定」旗標。存這個是為了讓前端的存在性檢查不必讀 Keychain
    // （未簽章的 dev binary 每次讀 Keychain 都會跳密碼框）。實際 key 仍只在 Keychain。
    pub google_key_set: bool,
    pub deepl_key_set: bool,
    /// Gemini（LLM，Workbench 文法/語境用）的 key 是否已設定。
    pub gemini_key_set: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: Provider::Google,
            // 預設：配對模式，我的語言=繁中、對照語言=English（主要使用者＝台灣工程師）。
            lang_mode: crate::router::LangMode::Pairing,
            target_lang: "zh-TW".into(),
            my_lang: "zh-TW".into(),
            counterpart_lang: "en".into(),
            double_press_ms: crate::monitor::double_press::DOUBLE_PRESS_WINDOW_MS,
            idle_close_ms: 6000,
            always_on_monitor: false,
            google_key_set: false,
            deepl_key_set: false,
            gemini_key_set: false,
        }
    }
}

impl Settings {
    fn key_set_flag(&self, provider: Provider) -> bool {
        match provider {
            Provider::Google => self.google_key_set,
            Provider::Deepl => self.deepl_key_set,
        }
    }

    fn set_key_flag(&mut self, provider: Provider, value: bool) {
        match provider {
            Provider::Google => self.google_key_set = value,
            Provider::Deepl => self.deepl_key_set = value,
        }
    }
}

/// 全域設定狀態。
/// - `double_press_ms` 另存 atomic，給 event tap callback 無鎖讀取。
/// - `key_cache` 快取已讀過的 API key，避免每次翻譯都讀 Keychain（減少密碼提示）。
pub struct SettingsState {
    pub current: Mutex<Settings>,
    pub double_press_ms: AtomicU64,
    key_cache: Mutex<HashMap<Provider, String>>,
    /// Gemini key 另存一格（不屬於 MT 的 Provider enum）。
    llm_key_cache: Mutex<Option<String>>,
}

impl SettingsState {
    pub fn new(settings: Settings) -> Self {
        Self {
            double_press_ms: AtomicU64::new(settings.double_press_ms),
            current: Mutex::new(settings),
            key_cache: Mutex::new(HashMap::new()),
            llm_key_cache: Mutex::new(None),
        }
    }

    pub fn snapshot(&self) -> Settings {
        self.current.lock().expect("settings mutex poisoned").clone()
    }

    /// 取 API key：先看記憶體快取，沒有才讀 Keychain 並快取。key 絕不進 log。
    pub fn api_key(&self, provider: Provider) -> Option<String> {
        if let Some(cached) = self
            .key_cache
            .lock()
            .expect("key cache poisoned")
            .get(&provider)
        {
            return Some(cached.clone());
        }
        let key = read_keychain(provider)?;
        self.key_cache
            .lock()
            .expect("key cache poisoned")
            .insert(provider, key.clone());
        Some(key)
    }

    fn cache_key(&self, provider: Provider, key: String) {
        self.key_cache
            .lock()
            .expect("key cache poisoned")
            .insert(provider, key);
    }

    fn evict_key(&self, provider: Provider) {
        self.key_cache
            .lock()
            .expect("key cache poisoned")
            .remove(&provider);
    }

    /// 取 Gemini key：先看快取，沒有才讀 Keychain 並快取。key 絕不進 log。
    pub fn llm_api_key(&self) -> Option<String> {
        if let Some(cached) = self.llm_key_cache.lock().expect("llm key cache poisoned").as_ref() {
            return Some(cached.clone());
        }
        let key = keyring::Entry::new(KEYCHAIN_SERVICE, LLM_ACCOUNT)
            .ok()?
            .get_password()
            .ok()?;
        *self.llm_key_cache.lock().expect("llm key cache poisoned") = Some(key.clone());
        Some(key)
    }
}

fn config_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let Some(path) = config_path(app) else {
        return Settings::default();
    };
    match fs::read_to_string(&path) {
        Ok(raw) => serde_json::from_str(&raw).unwrap_or_else(|e| {
            log::warn!("settings.json unreadable ({e}), falling back to defaults");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save(app: &AppHandle, settings: &Settings) {
    let Some(path) = config_path(app) else { return };
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(settings) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                log::error!("failed to write settings.json: {e}");
            }
        }
        Err(e) => log::error!("failed to serialize settings: {e}"),
    }
}

// ── Keychain ──────────────────────────────────────────────────────────────

fn keychain_entry(provider: Provider) -> Result<keyring::Entry, String> {
    let account = match provider {
        Provider::Google => "google_api_key",
        Provider::Deepl => "deepl_api_key",
    };
    keyring::Entry::new(KEYCHAIN_SERVICE, account).map_err(|e| e.to_string())
}

/// 實際讀 Keychain（可能跳密碼框）。一般走 `SettingsState::api_key` 的快取路徑。
fn read_keychain(provider: Provider) -> Option<String> {
    keychain_entry(provider).ok()?.get_password().ok()
}

// ── Commands ──────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, SettingsState>) -> Settings {
    state.snapshot()
}

#[tauri::command]
pub fn set_settings(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
    settings: Settings,
) -> Result<(), String> {
    if !(100..=1500).contains(&settings.double_press_ms) {
        return Err("雙擊時間窗需在 100–1500ms 之間".into());
    }
    if !(1500..=60_000).contains(&settings.idle_close_ms) {
        return Err("閒置關閉需在 1.5–60 秒之間".into());
    }
    state
        .double_press_ms
        .store(settings.double_press_ms, Ordering::Relaxed);
    *state.current.lock().expect("settings mutex poisoned") = settings.clone();
    save(&app, &settings);
    Ok(())
}

#[tauri::command]
pub fn set_api_key(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
    provider: Provider,
    key: String,
) -> Result<(), String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("API key 不可為空".into());
    }
    keychain_entry(provider)?
        .set_password(&key)
        .map_err(|e| format!("寫入 Keychain 失敗：{e}"))?;
    state.cache_key(provider, key);
    update_key_flag(&app, &state, provider, true);
    Ok(())
}

/// 存在性檢查：只讀 settings 旗標，不碰 Keychain（避免密碼提示）。
#[tauri::command]
pub fn api_key_set(state: tauri::State<'_, SettingsState>, provider: Provider) -> bool {
    state.snapshot().key_set_flag(provider)
}

#[tauri::command]
pub fn clear_api_key(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
    provider: Provider,
) -> Result<(), String> {
    state.evict_key(provider);
    update_key_flag(&app, &state, provider, false);
    match keychain_entry(provider)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("清除 Keychain 失敗：{e}")),
    }
}

/// 更新「key 是否已設定」旗標並持久化。
fn update_key_flag(
    app: &AppHandle,
    state: &SettingsState,
    provider: Provider,
    value: bool,
) {
    let snapshot = {
        let mut guard = state.current.lock().expect("settings mutex poisoned");
        guard.set_key_flag(provider, value);
        guard.clone()
    };
    save(app, &snapshot);
}

// ── Gemini（LLM）key ───────────────────────────────────────────────────────

fn update_gemini_flag(app: &AppHandle, state: &SettingsState, value: bool) {
    let snapshot = {
        let mut guard = state.current.lock().expect("settings mutex poisoned");
        guard.gemini_key_set = value;
        guard.clone()
    };
    save(app, &snapshot);
}

#[tauri::command]
pub fn set_llm_key(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
    key: String,
) -> Result<(), String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("API key 不可為空".into());
    }
    keyring::Entry::new(KEYCHAIN_SERVICE, LLM_ACCOUNT)
        .map_err(|e| e.to_string())?
        .set_password(&key)
        .map_err(|e| format!("寫入 Keychain 失敗：{e}"))?;
    *state.llm_key_cache.lock().expect("llm key cache poisoned") = Some(key);
    update_gemini_flag(&app, &state, true);
    Ok(())
}

#[tauri::command]
pub fn llm_key_set(state: tauri::State<'_, SettingsState>) -> bool {
    state.snapshot().gemini_key_set
}

#[tauri::command]
pub fn clear_llm_key(
    app: AppHandle,
    state: tauri::State<'_, SettingsState>,
) -> Result<(), String> {
    *state.llm_key_cache.lock().expect("llm key cache poisoned") = None;
    update_gemini_flag(&app, &state, false);
    match keyring::Entry::new(KEYCHAIN_SERVICE, LLM_ACCOUNT)
        .map_err(|e| e.to_string())?
        .delete_credential()
    {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("清除 Keychain 失敗：{e}")),
    }
}
