use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::server_api::ModelEntry;

pub const DEFAULT_HOTKEY: &str = "ctrl+alt+d";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Ptt,
    Toggle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub server_url: String,
    pub token: String,
    pub hotkey: String,
    pub mode: Mode,
    pub model: String,
    pub language: String,
    pub commands_enabled: bool,
    pub sounds_enabled: bool,
    pub autostart: bool,
    pub history_enabled: bool,
    pub mic_device: Option<String>,
    pub max_seconds: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: String::new(),
            token: String::new(),
            hotkey: DEFAULT_HOTKEY.to_string(),
            mode: Mode::Toggle,
            model: "gpt-4o-mini-transcribe".to_string(),
            language: "de".to_string(),
            commands_enabled: true,
            sounds_enabled: true,
            autostart: true,
            history_enabled: true,
            mic_device: None,
            max_seconds: 300,
        }
    }
}

fn config_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .expect("app config dir must be resolvable")
}

fn settings_path(app: &AppHandle) -> PathBuf {
    config_dir(app).join("settings.json")
}

fn models_cache_path(app: &AppHandle) -> PathBuf {
    config_dir(app).join("models_cache.json")
}

pub fn load(app: &AppHandle) -> Settings {
    fs::read_to_string(settings_path(app))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let dir = config_dir(app);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(settings_path(app), raw).map_err(|e| e.to_string())
}

/// Last model list received from the server; used to populate the tray menu
/// before (or without) a successful `/v1/models` fetch.
pub fn load_models_cache(app: &AppHandle) -> Vec<ModelEntry> {
    fs::read_to_string(models_cache_path(app))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(default_models)
}

pub fn save_models_cache(app: &AppHandle, models: &[ModelEntry]) {
    let dir = config_dir(app);
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(raw) = serde_json::to_string_pretty(models) {
            let _ = fs::write(models_cache_path(app), raw);
        }
    }
}

fn default_models() -> Vec<ModelEntry> {
    vec![
        ModelEntry {
            id: "gpt-4o-mini-transcribe".into(),
            label: "GPT-4o mini Transcribe".into(),
        },
        ModelEntry {
            id: "gpt-4o-transcribe".into(),
            label: "GPT-4o Transcribe".into(),
        },
        ModelEntry {
            id: "whisper-1".into(),
            label: "Whisper v2".into(),
        },
    ]
}
