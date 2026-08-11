use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState,
};
use tauri_plugin_notification::NotificationExt;

use crate::settings::Mode;
use crate::state::{AppState, PendingDictation, Session};
use crate::{audio, history, paste, server_api, sound, tray};

const MIN_RECORDING_SECS: f32 = 0.3;

fn escape_shortcut() -> Shortcut {
    Shortcut::new(None, Code::Escape)
}

pub fn notify(app: &AppHandle, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title("SprechBlitz")
        .body(body)
        .show();
}

// --- Global shortcut dispatch ------------------------------------------------

/// Runs an action on the main thread AFTER the shortcut handler has returned.
/// The plugin holds its internal shortcuts mutex while invoking the handler, so
/// calling register/unregister synchronously from here deadlocks the main
/// thread. Deferring through the async runtime releases that lock first.
fn run_deferred(app: &AppHandle, action: fn(&AppHandle)) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || action(&handle));
    });
}

pub fn on_shortcut(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let state = app.state::<AppState>();
    let (hotkey, mode) = {
        let s = state.settings.lock().unwrap();
        (s.hotkey.clone(), s.mode)
    };

    if *shortcut == escape_shortcut() {
        if event.state() == ShortcutState::Pressed {
            run_deferred(app, cancel);
        }
        return;
    }

    let Ok(main_shortcut) = hotkey.parse::<Shortcut>() else {
        return;
    };
    if *shortcut != main_shortcut {
        return;
    }

    match (mode, event.state()) {
        (Mode::Ptt, ShortcutState::Pressed) => run_deferred(app, start),
        (Mode::Ptt, ShortcutState::Released) => run_deferred(app, stop_and_transcribe),
        (Mode::Toggle, ShortcutState::Pressed) => {
            // Guard against OS key-repeat double toggles.
            let mut last = state.last_toggle.lock().unwrap();
            if last.elapsed() < Duration::from_millis(400) {
                return;
            }
            *last = Instant::now();
            drop(last);
            run_deferred(app, toggle);
        }
        _ => {}
    }
}

pub fn toggle(app: &AppHandle) {
    let state = app.state::<AppState>();
    let session = *state.session.lock().unwrap();
    match session {
        Session::Idle => start(app),
        Session::Recording { .. } => stop_and_transcribe(app),
        Session::Transcribing => {}
    }
}

// --- Recording lifecycle -----------------------------------------------------

pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let generation;
    {
        let mut session = state.session.lock().unwrap();
        if *session != Session::Idle {
            return;
        }
        generation = state.next_generation();
        *session = Session::Recording { generation };
    }

    let (mic, sounds_on, max_seconds) = {
        let s = state.settings.lock().unwrap();
        (s.mic_device.clone(), s.sounds_enabled, s.max_seconds)
    };

    if let Err(e) = state.audio.start(mic, app.clone()) {
        *state.session.lock().unwrap() = Session::Idle;
        notify(app, &format!("Aufnahme konnte nicht gestartet werden: {e}"));
        return;
    }

    if sounds_on {
        sound::play(sound::Cue::Start);
    }
    show_overlay(app, "recording", None);
    if let Err(e) = app.global_shortcut().register(escape_shortcut()) {
        eprintln!("[shortcut] Esc-Registrierung fehlgeschlagen: {e}");
    }
    tray::rebuild(app);

    // Safety auto-stop: a forgotten open microphone stops itself.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(max_seconds)).await;
        let still_recording = {
            let state = handle.state::<AppState>();
            let session = state.session.lock().unwrap();
            matches!(*session, Session::Recording { generation: g } if g == generation)
        };
        if still_recording {
            notify(&handle, "Auto-Stopp: maximale Aufnahmedauer erreicht");
            stop_and_transcribe(&handle);
        }
    });
}

pub fn stop_and_transcribe(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut session = state.session.lock().unwrap();
        if !matches!(*session, Session::Recording { .. }) {
            return;
        }
        *session = Session::Transcribing;
    }
    let _ = app.global_shortcut().unregister(escape_shortcut());

    let sounds_on = state.settings.lock().unwrap().sounds_enabled;
    if sounds_on {
        sound::play(sound::Cue::Stop);
    }

    let recorded = match state.audio.stop() {
        Ok(r) => r,
        Err(e) => {
            finish_session(app);
            hide_overlay(app);
            notify(app, &format!("Aufnahmefehler: {e}"));
            return;
        }
    };

    if recorded.duration_secs() < MIN_RECORDING_SECS {
        finish_session(app);
        hide_overlay(app);
        return;
    }

    show_overlay(app, "transcribing", None);
    tray::rebuild(app);

    let wav = audio::to_wav_16k(&recorded);
    let (model, language, commands) = {
        let s = state.settings.lock().unwrap();
        (s.model.clone(), s.language.clone(), s.commands_enabled)
    };
    let pending = PendingDictation { wav, model, language, commands };
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        transcribe_and_paste(handle, pending).await;
    });
}

pub fn cancel(app: &AppHandle) {
    let state = app.state::<AppState>();
    {
        let mut session = state.session.lock().unwrap();
        if !matches!(*session, Session::Recording { .. }) {
            return;
        }
        *session = Session::Idle;
    }
    state.audio.cancel();
    let _ = app.global_shortcut().unregister(escape_shortcut());
    hide_overlay(app);
    tray::rebuild(app);
}

pub fn retry_last(app: &AppHandle) {
    let state = app.state::<AppState>();
    let pending = state.last_failed.lock().unwrap().clone();
    let Some(pending) = pending else {
        return;
    };
    {
        let mut session = state.session.lock().unwrap();
        if *session != Session::Idle {
            return;
        }
        *session = Session::Transcribing;
    }
    show_overlay(app, "transcribing", None);
    tray::rebuild(app);
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        transcribe_and_paste(handle, pending).await;
    });
}

async fn transcribe_and_paste(app: AppHandle, pending: PendingDictation) {
    let (server_url, token) = {
        let state = app.state::<AppState>();
        let s = state.settings.lock().unwrap();
        (s.server_url.clone(), s.token.clone())
    };

    let result = server_api::transcribe(
        &server_url,
        &token,
        pending.wav.clone(),
        &pending.model,
        &pending.language,
        pending.commands,
    )
    .await;

    match result {
        Ok(text) if !text.trim().is_empty() => {
            let paste_result = tauri::async_runtime::spawn_blocking({
                let text = text.clone();
                move || paste::insert_text(&text)
            })
            .await;
            match paste_result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => notify(&app, &e),
                Err(e) => notify(&app, &format!("Einfügen fehlgeschlagen: {e}")),
            }
            history::add(&app, &text);
            {
                let state = app.state::<AppState>();
                *state.last_failed.lock().unwrap() = None;
            }
            hide_overlay(&app);
        }
        Ok(_) => {
            // Empty transcript (silence) – nothing to insert.
            hide_overlay(&app);
        }
        Err(e) => {
            {
                let state = app.state::<AppState>();
                *state.last_failed.lock().unwrap() = Some(pending);
                if state.settings.lock().unwrap().sounds_enabled {
                    sound::play(sound::Cue::Error);
                }
            }
            show_overlay(&app, "error", Some(&e));
            notify(&app, &format!("{e} – Diktat kann über das Tray-Menü erneut gesendet werden"));
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_secs(4)).await;
                hide_overlay(&handle);
            });
        }
    }

    finish_session(&app);
}

fn finish_session(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.session.lock().unwrap() = Session::Idle;
    tray::rebuild(app);
}

// --- Overlay -----------------------------------------------------------------

fn show_overlay(app: &AppHandle, state_name: &str, message: Option<&str>) {
    let Some(window) = app.get_webview_window("overlay") else {
        return;
    };
    // Top-center of the primary monitor.
    if let Ok(Some(monitor)) = app.primary_monitor() {
        if let Ok(size) = window.outer_size() {
            let x = monitor.position().x
                + ((monitor.size().width as i32 - size.width as i32) / 2).max(0);
            let y = monitor.position().y + 12;
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        }
    }
    let _ = window.show();
    let _ = app.emit_to(
        "overlay",
        "overlay-state",
        serde_json::json!({ "state": state_name, "message": message }),
    );
}

fn hide_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.hide();
    }
}
