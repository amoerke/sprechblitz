use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

const MAX_ENTRIES: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub text: String,
}

fn history_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .expect("app data dir must be resolvable")
        .join("history.json")
}

pub fn load(app: &AppHandle) -> Vec<HistoryEntry> {
    fs::read_to_string(history_path(app))
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn persist(app: &AppHandle, entries: &[HistoryEntry]) {
    let path = history_path(app);
    if let Some(dir) = path.parent() {
        if fs::create_dir_all(dir).is_ok() {
            if let Ok(raw) = serde_json::to_string_pretty(entries) {
                let _ = fs::write(path, raw);
            }
        }
    }
}

/// Adds a transcript to the history (newest first), respecting the
/// history_enabled setting and the entry cap.
pub fn add(app: &AppHandle, text: &str) {
    let state = app.state::<AppState>();
    if !state.settings.lock().unwrap().history_enabled {
        return;
    }
    let mut entries = state.history.lock().unwrap();
    entries.insert(
        0,
        HistoryEntry {
            timestamp: chrono::Local::now().to_rfc3339(),
            text: text.to_string(),
        },
    );
    entries.truncate(MAX_ENTRIES);
    persist(app, &entries);
}

pub fn clear(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut entries = state.history.lock().unwrap();
    entries.clear();
    persist(app, &entries);
}
