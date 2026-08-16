# SprechBlitz Client

Tauri-2-App für macOS (Menüleiste) und Windows (Infobereich). Globaler Hotkey (Standard `Ctrl+Alt+D` bzw. `⌃⌥D`) startet die Aufnahme, das Transkript wird per Zwischenablage an der Cursorposition eingefügt (die vorherige Zwischenablage wird wiederhergestellt).

## Entwicklung

Voraussetzungen: Node ≥ 22, Rust (stable), unter Windows die MSVC Build Tools, unter macOS die
Xcode Command Line Tools (`xcode-select --install`).

### Rust installieren

Das Tauri-CLI ruft beim Bauen `cargo` auf – Rust muss deshalb global installiert sein (empfohlen
über [rustup](https://rustup.rs), den offiziellen Installer):

**macOS / Linux:**

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Der Installer richtet `~/.cargo/bin` im PATH ein (Eintrag in `~/.zshenv` bzw. `~/.profile`).
Danach ein neues Terminal öffnen oder einmalig `source "$HOME/.cargo/env"` ausführen.

**Windows:**

```powershell
winget install Rustlang.Rustup
```

(alternativ `rustup-init.exe` von [rustup.rs](https://rustup.rs) laden). Rustup installiert die
MSVC-Toolchain-Anbindung; die MSVC Build Tools selbst kommen über den Visual Studio Installer
(Workload „Desktopentwicklung mit C++").

**Prüfen:**

```sh
rustc --version   # z. B. rustc 1.97.1
cargo --version
```

### Starten

```sh
cd client
npm install
npm run tauri dev
```

Beim ersten Start öffnet sich das Einstellungsfenster: Server-URL und Zugangs-Token eintragen, „Verbindung testen“, speichern.

**Achtung Build-Falle:** Ein lauffähiges Binary immer über das Tauri-CLI bauen (`npm run tauri dev` oder `npx tauri build --debug --no-bundle`). Ein pures `cargo build` in `src-tauri/` erzeugt ein Binary, das die UI vom Vite-Dev-Server (`localhost:1420`) erwartet – ohne laufenden Dev-Server bleiben alle Fenster leer.

## Release-Build

Am einfachsten per Buildscript (fragt das Schlüssel-Passwort ab, richtet PATH selbst ein):

```powershell
cd client
.\build-release.ps1            # Release + Installer + signierte Updater-Artefakte
.\build-release.ps1 -Debug     # schneller Debug-Build ohne Installer
```

Unter macOS entsprechend:

```sh
cd client
./build-release.sh             # Release + .app/.dmg + signierte Updater-Artefakte
./build-release.sh --debug     # schneller Debug-Build ohne Bundle
```

Manuell entspricht das:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "..\updater-signing.key" -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "<Schlüssel-Passwort>"   # darf nicht leer sein
npm run tauri build
```

Artefakte: `src-tauri/target/release/bundle/` (NSIS-Installer unter Windows, `.app`/`.dmg` unter macOS).

## Auto-Updates

Der Client prüft beim Start gegen den in `src-tauri/tauri.conf.json` konfigurierten Endpunkt (GitHub Releases, `latest.json`). Release-Ablauf:

1. Version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` und `package.json` erhöhen.
2. `npm run tauri build` mit gesetztem `TAURI_SIGNING_PRIVATE_KEY_PATH` (erzeugt `.sig`-Dateien und Update-Bundles).
3. GitHub-Release anlegen; Bundles, `.sig`-Dateien und eine `latest.json` hochladen.
4. In `tauri.conf.json` unter `plugins.updater.endpoints` `OWNER` durch den GitHub-Namen ersetzen (einmalig).

**`updater-signing.key` niemals verlieren oder veröffentlichen** – ohne ihn können bestehende Installationen nie wieder aktualisiert werden.

## macOS-Hinweise

- **Mikrofon**: Erlaubnis wird beim ersten Diktat abgefragt.
- **Bedienungshilfen (Accessibility)**: Für das simulierte `⌘V` muss die App unter *Systemeinstellungen → Datenschutz & Sicherheit → Bedienungshilfen* freigegeben werden – sonst schlägt das Einfügen fehl (der Text liegt dann trotzdem in der Zwischenablage).
- **Gatekeeper** (unsignierter Build): Beim ersten Öffnen Rechtsklick → „Öffnen“.

## Windows-Hinweise

- SmartScreen kann beim unsignierten Installer warnen: „Weitere Informationen“ → „Trotzdem ausführen“.
- Autostart legt einen Eintrag unter `HKCU\...\Run` an (abschaltbar in den Einstellungen).
