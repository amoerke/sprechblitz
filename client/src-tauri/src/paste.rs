use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::thread;
use std::time::Duration;

/// Inserts text at the current cursor position via clipboard + simulated paste,
/// restoring the previous clipboard text afterwards. If the paste keystroke
/// fails, the text stays in the clipboard so nothing is lost.
pub fn insert_text(text: &str) -> Result<(), String> {
    let mut clipboard =
        Clipboard::new().map_err(|e| format!("Zwischenablage nicht verfügbar: {e}"))?;
    let previous = clipboard.get_text().ok();
    clipboard
        .set_text(text)
        .map_err(|e| format!("Zwischenablage konnte nicht gesetzt werden: {e}"))?;
    thread::sleep(Duration::from_millis(120));

    if let Err(e) = send_paste() {
        // Leave the transcript in the clipboard as a fallback.
        return Err(format!("{e} – der Text liegt in der Zwischenablage"));
    }

    // Give the target application time to read the clipboard before restoring.
    thread::sleep(Duration::from_millis(300));
    if let Some(prev) = previous {
        let _ = clipboard.set_text(prev);
    }
    Ok(())
}

fn send_paste() -> Result<(), String> {
    let mut enigo = Enigo::new(&EnigoSettings::default())
        .map_err(|e| format!("Tastatursimulation nicht möglich: {e}"))?;
    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    let err = |e: enigo::InputError| format!("Einfügen fehlgeschlagen: {e}");
    enigo.key(modifier, Direction::Press).map_err(err)?;
    enigo.key(Key::Unicode('v'), Direction::Click).map_err(err)?;
    enigo.key(modifier, Direction::Release).map_err(err)?;
    Ok(())
}

pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    Clipboard::new()
        .and_then(|mut c| c.set_text(text))
        .map_err(|e| format!("Zwischenablage nicht verfügbar: {e}"))
}
