//! 設定層：偏好存 app config 目錄 JSON；API key 一律存 macOS Keychain（紅線）。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::providers::Provider;

const KEYCHAIN_SERVICE: &str = "com.kimj.sumi";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub provider: Provider,
    pub target_lang: String,
    pub double_press_ms: u64,
    pub idle_close_ms: u64,
    pub always_on_monitor: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            provider: Provider::Google,
            target_lang: "zh-TW".into(),
            double_press_ms: crate::monitor::double_press::DOUBLE_PRESS_WINDOW_MS,
            idle_close_ms: 6000,
            always_on_monitor: false,
        }
    }
}

/// 全域設定狀態。`double_press_ms` 另存 atomic，給 event tap callback 無鎖讀取。
pub struct SettingsState {
    pub current: Mutex<Settings>,
    pub double_press_ms: AtomicU64,
}

impl SettingsState {
    pub fn new(settings: Settings) -> Self {
        Self {
            double_press_ms: AtomicU64::new(settings.double_press_ms),
            current: Mutex::new(settings),
        }
    }

    pub fn snapshot(&self) -> Settings {
        self.current.lock().expect("settings mutex poisoned").clone()
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

/// 後端取 key 用。key 絕不回傳給前端、絕不進 log。
pub fn api_key(provider: Provider) -> Option<String> {
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
pub fn set_api_key(provider: Provider, key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err("API key 不可為空".into());
    }
    keychain_entry(provider)?
        .set_password(key)
        .map_err(|e| format!("寫入 Keychain 失敗：{e}"))
}

#[tauri::command]
pub fn api_key_set(provider: Provider) -> bool {
    api_key(provider).is_some()
}

#[tauri::command]
pub fn clear_api_key(provider: Provider) -> Result<(), String> {
    match keychain_entry(provider)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("清除 Keychain 失敗：{e}")),
    }
}
