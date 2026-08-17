use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
pub mod accessibility {
    use core_foundation::base::{Boolean, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
    }

    pub const MISSING_HINT: &str = "Berechtigung „Bedienungshilfen“ fehlt – \
        Systemeinstellungen → Datenschutz & Sicherheit → Bedienungshilfen: \
        SprechBlitz entfernen und neu hinzufügen";

    /// Whether this process may post synthetic keyboard events. Without the
    /// permission macOS drops every event silently and enigo still reports
    /// success, so the paste looks fine and nothing arrives at the cursor.
    /// The grant is bound to the binary's code signature, so every ad-hoc
    /// signed rebuild invalidates it while the stale entry keeps showing up as
    /// enabled in System Settings – hence the "remove and re-add" hint.
    /// With `prompt` macOS shows its own dialog and registers the app in the
    /// Accessibility list, which is what makes re-granting possible at all.
    pub fn is_trusted(prompt: bool) -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(
                key.as_CFType(),
                CFBoolean::from(prompt).as_CFType(),
            )]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0
        }
    }
}

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

    // Checked after the clipboard is filled: without the permission the paste
    // would fail silently, so bail out with a usable error instead.
    #[cfg(target_os = "macos")]
    if !accessibility::is_trusted(true) {
        return Err(format!(
            "{} – der Text liegt in der Zwischenablage",
            accessibility::MISSING_HINT
        ));
    }

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
