use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::history::HistoryEntry;
use crate::server_api::{self, ModelEntry};
use crate::settings::Settings;
use crate::state::AppState;
use crate::{dictation, history, paste, tray};

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    settings
        .hotkey
        .parse::<Shortcut>()
        .map_err(|_| format!("Ungültiger Hotkey: „{}“", settings.hotkey))?;

    let old_hotkey = {
        let state = app.state::<AppState>();
        let mut current = state.settings.lock().unwrap();
        let old = current.hotkey.clone();
        *current = settings.clone();
        old
    };
    crate::settings::save(&app, &settings)?;

    if old_hotkey != settings.hotkey {
        if let Ok(old) = old_hotkey.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(old);
        }
        register_hotkey(&app, &settings.hotkey)?;
    }

    let autolaunch = app.autolaunch();
    let _ = if settings.autostart {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };

    tray::rebuild(&app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        refresh_models(handle).await;
    });
    Ok(())
}

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let shortcut = hotkey
        .parse::<Shortcut>()
        .map_err(|_| format!("Ungültiger Hotkey: „{hotkey}“"))?;
    app.global_shortcut()
        .register(shortcut)
        .map_err(|e| format!("Hotkey konnte nicht registriert werden: {e}"))
}

#[tauri::command]
pub fn list_input_devices() -> Vec<String> {
    crate::audio::list_input_devices()
}

#[tauri::command]
pub fn get_cached_models(state: State<'_, AppState>) -> Vec<ModelEntry> {
    state.models.lock().unwrap().clone()
}

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Vec<HistoryEntry> {
    state.history.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_history(app: AppHandle) {
    history::clear(&app);
    tray::rebuild(&app);
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    paste::copy_to_clipboard(&text)
}

#[tauri::command]
pub fn cancel_recording(app: AppHandle) {
    dictation::cancel(&app);
}

#[tauri::command]
pub async fn test_connection(
    app: AppHandle,
    server_url: String,
    token: String,
) -> Result<Vec<ModelEntry>, String> {
    let response = server_api::fetch_models(&server_url, &token).await?;
    {
        let state = app.state::<AppState>();
        *state.models.lock().unwrap() = response.models.clone();
    }
    crate::settings::save_models_cache(&app, &response.models);
    tray::rebuild(&app);
    Ok(response.models)
}

/// Fetches the model list from the configured server; silently keeps the
/// cached list when the server is unreachable.
pub async fn refresh_models(app: AppHandle) {
    let (server_url, token) = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        (s.server_url.clone(), s.token.clone())
    };
    if server_url.is_empty() {
        return;
    }
    if let Ok(response) = server_api::fetch_models(&server_url, &token).await {
        {
            let state = app.state::<AppState>();
            *state.models.lock().unwrap() = response.models.clone();
        }
        crate::settings::save_models_cache(&app, &response.models);
        tray::rebuild(&app);
    }
}
