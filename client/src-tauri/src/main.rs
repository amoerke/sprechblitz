// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod commands;
mod dictation;
mod history;
mod paste;
mod server_api;
mod settings;
mod sound;
mod state;
mod tray;

use tauri::Manager;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use crate::state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_settings_window(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(dictation::on_shortcut)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::list_input_devices,
            commands::get_cached_models,
            commands::get_history,
            commands::clear_history,
            commands::copy_text,
            commands::cancel_recording,
            commands::test_connection,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            let app_settings = settings::load(&handle);
            let models = settings::load_models_cache(&handle);
            let history_entries = history::load(&handle);
            app.manage(AppState::new(app_settings.clone(), models, history_entries));

            // Menu-bar-only app on macOS: no dock icon.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            tray::init(&handle)?;

            // The overlay must never steal focus from the paste target.
            if let Some(overlay) = handle.get_webview_window("overlay") {
                let _ = overlay.set_focusable(false);
            }

            if let Err(e) = commands::register_hotkey(&handle, &app_settings.hotkey) {
                dictation::notify(&handle, &e);
            }

            let autolaunch = handle.autolaunch();
            let _ = if app_settings.autostart {
                autolaunch.enable()
            } else {
                autolaunch.disable()
            };

            // First run: open the settings window so URL/token can be entered.
            if app_settings.server_url.is_empty() {
                tray::show_settings_window(&handle);
            }

            let models_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                commands::refresh_models(models_handle).await;
            });

            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                check_for_updates(update_handle).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the settings window hides it; the app lives in the tray.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building SprechBlitz")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                // Keep running with all windows hidden (tray app), but let
                // an explicit app.exit() through.
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

async fn check_for_updates(app: tauri::AppHandle) {
    use tauri_plugin_updater::UpdaterExt;
    let Ok(updater) = app.updater() else { return };
    let Ok(Some(update)) = updater.check().await else {
        return;
    };
    if update.download_and_install(|_, _| {}, || {}).await.is_ok() {
        dictation::notify(
            &app,
            "Update installiert – wird beim nächsten Start aktiv",
        );
    }
}
