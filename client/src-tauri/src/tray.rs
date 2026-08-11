use tauri::menu::{CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::settings::Mode;
use crate::state::{AppState, Session};
use crate::{commands, dictation, paste};

const TRAY_ID: &str = "main";

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().expect("window icon").clone())
        .tooltip("SprechBlitz")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| handle_menu(app, event.id().as_ref()))
        .build(app)?;
    Ok(())
}

pub fn rebuild(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let session = *state.session.lock().unwrap();
    let retry_available = state.last_failed.lock().unwrap().is_some();
    let models = state.models.lock().unwrap().clone();
    let history_entries = state.history.lock().unwrap().clone();

    let (record_label, record_enabled) = match session {
        Session::Idle => (format!("Aufnahme starten\t{}", settings.hotkey), true),
        Session::Recording { .. } => ("Aufnahme stoppen".to_string(), true),
        Session::Transcribing => ("Transkribiere …".to_string(), false),
    };
    let record = MenuItemBuilder::with_id("record", record_label)
        .enabled(record_enabled)
        .build(app)?;
    let retry = MenuItemBuilder::with_id("retry", "Letztes Diktat erneut senden")
        .enabled(retry_available && session == Session::Idle)
        .build(app)?;

    let mut model_menu = SubmenuBuilder::new(app, "Modell");
    for model in &models {
        let item = CheckMenuItemBuilder::with_id(format!("model:{}", model.id), &model.label)
            .checked(settings.model == model.id)
            .build(app)?;
        model_menu = model_menu.item(&item);
    }
    let model_menu = model_menu.build()?;

    let language_menu = SubmenuBuilder::new(app, "Sprache")
        .item(
            &CheckMenuItemBuilder::with_id("lang:de", "Deutsch")
                .checked(settings.language == "de")
                .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id("lang:en", "Englisch")
                .checked(settings.language == "en")
                .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id("lang:auto", "Automatisch")
                .checked(settings.language == "auto")
                .build(app)?,
        )
        .build()?;

    let mode_menu = SubmenuBuilder::new(app, "Modus")
        .item(
            &CheckMenuItemBuilder::with_id("mode:toggle", "Umschalten")
                .checked(settings.mode == Mode::Toggle)
                .build(app)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id("mode:ptt", "Push-to-talk (halten)")
                .checked(settings.mode == Mode::Ptt)
                .build(app)?,
        )
        .build()?;

    let commands_item = CheckMenuItemBuilder::with_id("cmds", "Sprachbefehle („neue Zeile“ …)")
        .checked(settings.commands_enabled)
        .build(app)?;

    let mut history_menu = SubmenuBuilder::new(app, "Verlauf");
    if history_entries.is_empty() {
        history_menu = history_menu.item(
            &MenuItemBuilder::with_id("hist:none", "(leer)")
                .enabled(false)
                .build(app)?,
        );
    } else {
        for (index, entry) in history_entries.iter().take(5).enumerate() {
            let mut label: String = entry.text.chars().take(40).collect();
            if entry.text.chars().count() > 40 {
                label.push('…');
            }
            history_menu = history_menu
                .item(&MenuItemBuilder::with_id(format!("hist:{index}"), label).build(app)?);
        }
    }
    let history_menu = history_menu.build()?;

    let settings_item = MenuItemBuilder::with_id("settings", "Einstellungen …").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Beenden").build(app)?;

    MenuBuilder::new(app)
        .item(&record)
        .item(&retry)
        .separator()
        .item(&model_menu)
        .item(&language_menu)
        .item(&mode_menu)
        .item(&commands_item)
        .separator()
        .item(&history_menu)
        .item(&settings_item)
        .separator()
        .item(&quit)
        .build()
}

fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "record" => dictation::toggle(app),
        "retry" => dictation::retry_last(app),
        "settings" => show_settings_window(app),
        "quit" => app.exit(0),
        "cmds" => update_settings(app, |s| s.commands_enabled = !s.commands_enabled),
        "lang:de" => update_settings(app, |s| s.language = "de".into()),
        "lang:en" => update_settings(app, |s| s.language = "en".into()),
        "lang:auto" => update_settings(app, |s| s.language = "auto".into()),
        "mode:ptt" => update_settings(app, |s| s.mode = Mode::Ptt),
        "mode:toggle" => update_settings(app, |s| s.mode = Mode::Toggle),
        id if id.starts_with("model:") => {
            let model = id["model:".len()..].to_string();
            update_settings(app, move |s| s.model = model.clone());
        }
        id if id.starts_with("hist:") => {
            if let Ok(index) = id["hist:".len()..].parse::<usize>() {
                let state = app.state::<AppState>();
                let text = state.history.lock().unwrap().get(index).map(|e| e.text.clone());
                if let Some(text) = text {
                    if let Err(e) = paste::copy_to_clipboard(&text) {
                        dictation::notify(app, &e);
                    }
                }
            }
        }
        _ => {}
    }
}

fn update_settings(app: &AppHandle, mutate: impl Fn(&mut crate::settings::Settings)) {
    let state = app.state::<AppState>();
    let updated = {
        let mut settings = state.settings.lock().unwrap();
        mutate(&mut settings);
        settings.clone()
    };
    if let Err(e) = crate::settings::save(app, &updated) {
        dictation::notify(app, &format!("Einstellungen konnten nicht gespeichert werden: {e}"));
    }
    rebuild(app);
}

pub fn show_settings_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    // Model list may have changed on the server; refresh in the background.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        commands::refresh_models(handle).await;
    });
}
