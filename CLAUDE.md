# SprechBlitz – Projektgedächtnis

Diktier-App: Hotkey drücken, sprechen, Text erscheint an der Cursorposition der aktiven App.
Zwei Teile in einem Repo: **`client/`** (Tauri 2, Rust + TS) und **`server/`** (Node/TS, Fastify).

Repo: `https://github.com/amoerke/sprechblitz` · Branch `main` · Sprache im Code, in Commits und
in der UI: **Deutsch** (Kommentare im Rust-/TS-Code sind englisch – Stil beibehalten).

---

## Architektur in einem Absatz

Der Client nimmt per globalem Hotkey über **cpal** auf, resampelt auf 16 kHz Mono-WAV und schickt
das per Multipart an den eigenen Server. Der Server ist ein **zustandsloser Proxy**: er hält den
OpenAI-Key, ruft je nach Modell `POST {OPENAI_BASE_URL}/audio/transcriptions` oder das selbst
gehostete Whisper (speaches, `{SPEACHES_BASE_URL}/audio/transcriptions`) auf, ersetzt optional gesprochene
Befehle („neue Zeile" → `\n`) und gibt `{ text, model }` zurück. Der Client fügt den Text über
Zwischenablage + simuliertes `Ctrl/⌘+V` an der Cursorposition ein und stellt die alte Zwischenablage
wieder her. **Der API-Key erreicht den Client nie.**

```
Hotkey → audio.rs (cpal) → to_wav_16k() → server_api::transcribe()
   → [Server] auth.ts → transcribe.ts (OpenAI) → commands.ts → { text }
   → paste.rs (Clipboard + ⌘V) → history.rs
```

---

## Client (`client/`)

Tauri 2, Tray-/Menüleisten-App ohne Dock-Icon (`ActivationPolicy::Accessory` auf macOS).
Zwei Fenster: `main` (Einstellungen, startet versteckt) und `overlay` (randlos, transparent,
always-on-top, **nicht fokussierbar**).

| Datei | Zuständigkeit |
|---|---|
| `src-tauri/src/main.rs` | Plugin-Setup, State, Tray-Init, Hotkey-Registrierung, Update-Check |
| `src-tauri/src/dictation.rs` | Aufnahme-Lebenszyklus: start / stop / cancel / retry, Overlay-Steuerung |
| `src-tauri/src/audio.rs` | cpal-Aufnahme auf eigenem Thread, Pegel-Events, `to_wav_16k()` |
| `src-tauri/src/paste.rs` | Clipboard setzen, `⌘V`/`Ctrl+V` via enigo, Clipboard restaurieren |
| `src-tauri/src/server_api.rs` | HTTP zum Server, Fehlertexte auf Deutsch |
| `src-tauri/src/settings.rs` | `settings.json` im App-Config-Dir, Modell-Cache |
| `src-tauri/src/state.rs` | `AppState`, `Session`-Enum (Idle/Recording/Transcribing), `PendingDictation` |
| `src-tauri/src/tray.rs` | Tray-Menü (Modell, Sprache, Modus, Verlauf) – wird nach jeder Änderung neu gebaut |
| `src-tauri/src/history.rs` | Lokaler Verlauf, max. 20 Einträge, `history.json` im App-Data-Dir |
| `src-tauri/src/commands.rs` | `#[tauri::command]`-Bridge für das Einstellungsfenster |
| `src/settings.ts`, `src/overlay.ts` | Frontend (Vanilla TS, kein Framework), Vite mit 2 Entry-Points |

**Einstellungen** (Defaults in `settings.rs`): Hotkey `ctrl+alt+d`, Modus `toggle`,
Modell `gpt-4o-mini-transcribe`, Sprache `de`, Sprachbefehle an, Töne an, Autostart an,
Verlauf an, Auto-Stopp nach 300 s.

### Harte Invarianten – nicht versehentlich brechen

1. **Nie synchron `register`/`unregister` im `on_shortcut`-Handler aufrufen.** Das
   global-shortcut-Plugin hält währenddessen seinen Mutex → Deadlock, die App friert nach
   Aufnahmestart komplett ein. Deshalb existiert `run_deferred()` in `dictation.rs`; alle
   Shortcut-Aktionen laufen darüber. (Fix in `b43a540`.)
2. **Das Overlay darf niemals Fokus bekommen** (`set_focusable(false)` in `main.rs`), sonst geht
   das Einfüge-Ziel verloren und der Text landet im Nichts.
3. **Binary immer über das Tauri-CLI bauen** (`npm run tauri dev` / `npx tauri build`). Ein pures
   `cargo build` in `src-tauri/` erzeugt ein Binary, das die UI von `localhost:1420` erwartet →
   alle Fenster bleiben leer. Klassische Fehlersuche-Falle.
4. **PowerShell-Skripte als UTF-8 *mit* BOM speichern.** PowerShell 5.1 liest BOM-lose Dateien als
   ANSI und stolpert über Umlaute/Sonderzeichen.
5. `Session`-Übergänge laufen ausschließlich über `dictation.rs`; das `generation`-Feld verhindert,
   dass ein alter Auto-Stopp-Timer eine neue Aufnahme abwürgt.
6. **Das Einfügen (`paste::insert_text`) muss auf dem Main-Thread laufen.** enigo ruft auf macOS
   TIS-APIs (`TISCopyCurrentKeyboardInputSource`) auf; macOS 14+ bricht den Prozess bei Aufruf von
   einem anderen Thread sofort ab (Dispatch-Queue-Assertion) – die App verschwindet kommentarlos
   nach der Transkription. Deshalb `run_on_main_thread` + Oneshot-Channel in `dictation.rs`,
   kein `spawn_blocking`.
7. **Bleibt der Text nach dem Transkribieren aus, ist fast immer die macOS-Berechtigung
   „Bedienungshilfen" schuld.** Sie hängt an der Code-Signatur des Binaries; da sowohl Dev-Build
   als auch Bundle nur ad-hoc/linker-signiert sind (`security find-identity` → 0 Identitäten),
   erzeugt **jeder Neubau einen neuen CDHash** und entwertet die Freigabe – der Eintrag in den
   Systemeinstellungen bleibt dabei sichtbar und angehakt. enigo 0.2 prüft die Berechtigung nicht,
   `CGEventPost` verschluckt die Tasten still, `insert_text` meldete früher Erfolg. Deshalb prüft
   `paste::accessibility::is_trusted()` jetzt vor dem Einfügen und beim Start. Heilung: Eintrag
   entfernen und neu hinzufügen. Dev-Binary (`target/debug/sprechblitz`) und `/Applications/
   SprechBlitz.app` sind **getrennte** TCC-Identitäten und brauchen je eine eigene Freigabe.

---

## Server (`server/`)

Fastify 5, Node ≥ 22, ESM, `tsc` → `dist/`. Port **8790**, Host `0.0.0.0`.

| Methode | Pfad | Auth | Inhalt |
|---|---|---|---|
| GET | `/health` | – | `{ ok: true }` |
| GET | `/v1/models` | Bearer | `config/models.json`, gefiltert auf konfigurierte Anbieter, nur `id`/`label` |
| POST | `/v1/transcribe` | Bearer | Multipart `file` + `model`, `language`, `commands` → `{ text, model }` |

**Env** (`.env.example`): `AUTH_TOKENS` (Pflicht) und mindestens einer der Anbieter:
`OPENAI_API_KEY` und/oder `SPEACHES_BASE_URL` (z. B. `http://speaches:8000/v1`). Optional
`SPEACHES_API_KEY`, `PORT`, `HOST`, `OPENAI_BASE_URL`, `MODELS_CONFIG`, `COMMANDS_CONFIG_DIR`,
`MAX_UPLOAD_BYTES` (30 MB), `PARSE_MODEL`, `LOG_LEVEL`.
`.env` wird über `process.loadEnvFile()` geladen – Node tut das **nicht** von selbst (`b43a540`).

**Anbieter pro Modell**: Einträge in `models.json` haben optional `"provider": "speaches"`
(Default `openai`). Modelle eines nicht konfigurierten Anbieters werden beim Start ausgeblendet;
liegt der Default darunter, wird das erste nutzbare Modell Default. Ohne `OPENAI_API_KEY`
antworten `/v1/parse-*` mit 503.

**speaches (selbst gehostetes Whisper)**: speaches bedient nur lokal installierte Modelle
(sonst 404 „not installed locally"). `PRELOAD_MODELS` greift in der aktuellen Version **nicht** –
deshalb lädt `src/speaches.ts` beim Serverstart alle speaches-Modelle aus `models.json` per
`POST /v1/models/{id}` nach (Hintergrund, mit Retry). Im Compose-Profil `speaches` läuft der
Container ohne Port nach außen. CPU-Richtwerte (8 s Audio): `faster-whisper-small` ~3 s,
`large-v3-turbo` ~12 s – turbo auf CPU ist langsamer als Echtzeit, Client-Timeout ist 120 s.
**Nur mehrsprachige Modelle eintragen** – `distil-large-v3` u. ä. können kein Deutsch.

**Auth**: `AUTH_TOKENS="name:token,name2:token2"`, mindestens 16 Zeichen pro Token, Vergleich mit
`timingSafeEqual` gegen *alle* Einträge (kein Early-Return, damit Timing nichts verrät). Ein Mensch
wird durch Entfernen seines Eintrags gesperrt. Der Token-Name landet im Log.

### Regeln

- **Niemals Inhalte loggen.** Geloggt werden nur Token-Name, Modell, Sprache, `audioBytes`,
  `durationMs`. Kein Transkript, kein Audio, keine Persistenz.
- Sprachbefehle (`config/commands.de.json`) werden **in Konfigurations-Reihenfolge** angewandt –
  längere Phrasen müssen vor kürzeren stehen („Anführungszeichen Ende" vor „Anführungszeichen").
  Die Datei wird bei mtime-Änderung automatisch neu eingelesen (Volume-Mount ohne Neustart möglich).
- Befehle greifen **nur bei gesetzter Sprache**; bei `language=auto` bleibt der Text unverändert
  (`index.ts`: `if (commandsEnabled && language)`).
- Neue Modelle nur in `config/models.json` eintragen – der Client holt die Liste von `/v1/models`,
  ein Client-Update ist dafür nicht nötig.

**Deployment**: Coolify-Application aus dem GitHub-Repo, Base Directory `/server`, Build Pack
Dockerfile, Env-Vars setzen, Domain zuweisen, Port 8790. Lokal alternativ `docker compose up`
(mit lokalem Whisper: `docker compose --profile speaches up -d` + `SPEACHES_BASE_URL` in `.env`).

---

## Build & Release

```powershell
cd client
.\build-release.ps1          # Release + NSIS-Installer + signierte Updater-Artefakte
.\build-release.ps1 -Debug   # schneller Debug-Build ohne Installer
.\release.ps1 -Version 0.1.1 # Version setzen → bauen → latest.json → GitHub-Release
```

macOS-Pendants: `./build-release.sh` / `./build-release.sh --debug` und `./release.sh 0.1.1`.

- **Windows- und Mac-Releases laufen unabhängig**: Existiert das Release `v<Version>` schon,
  lädt das jeweilige Release-Skript die vorhandene `latest.json` herunter, ergänzt nur den
  eigenen Plattform-Eintrag (`gh release upload --clobber`) und lässt die anderen stehen.
  Übernommen werden dabei nur Einträge **derselben** Version – sonst würde der Updater einen
  alten Build unter neuer Versionsnummer ausliefern.

- Signaturschlüssel: `updater-signing.key` im Repo-Root, **gitignored** (`*.key`), **passwortgeschützt**.
  Das Passwort ist Pflicht – ein passwortloser Schlüssel funktioniert unter Windows nicht, weil
  PowerShell keine leeren Env-Variablen setzen kann (`fe6b798`).
- **Geht der Schlüssel verloren, können bestehende Installationen nie wieder Updates bekommen.**
- Updater-Endpunkt: `https://github.com/amoerke/sprechblitz/releases/latest/download/latest.json`,
  Pubkey in `tauri.conf.json`.
- `release.ps1` committet den Versionsbump lokal, **pusht aber nicht** – `git push` manuell.

---

## Stand & offene Punkte (Stand 2026-08-15)

- 9 Commits, letzter `c358fe7` (docker-compose). **Keine Git-Tags, kein GitHub-Release** – v0.1.0
  ist noch nicht veröffentlicht, der Updater-Pfad also noch nie end-to-end gelaufen.
- **macOS in Arbeit** (seit 2026-08-16): Dev-Build läuft auf Apple Silicon, Paste-Crash behoben
  (Invariante 6), `build-release.sh`/`release.sh` vorhanden (`darwin-aarch64` via `uname -m`).
  Es fehlen noch: Signierung/Notarisierung (Gatekeeper-Rechtsklick-Workaround nötig) und ein
  End-to-End-Test des Release-/Updater-Pfads auf beiden Plattformen.
- Nur `commands.de.json` existiert; `commands.en.json` fehlt (bei `language=en` greifen keine Befehle).
- Keine automatisierten Tests, keine CI.
- `.claude/settings.local.json` ist untracked – bewusst so lassen oder ignorieren.

---

## Wo die bisherige Historie liegt

Das Projekt wurde mit Claude Code in PowerShell gebaut. Es gab bis zu dieser Datei **kein**
Projektgedächtnis. Rekonstruierbar ist die Historie aus:

- **Commit-Messages** – ausführlich und mit Begründungen, das beste vorhandene Protokoll.
- **Claude-Session-Trailer** in den Commits: `session_019KbLnzWcGaQXHmhYS13nLP`
  (→ `https://claude.ai/code/session_019KbLnzWcGaQXHmhYS13nLP`) für Commits vom 2026-08-11.
- **Lokale Transkripte**: `%USERPROFILE%\.claude\projects\C--Users-a-moerke-Projekte-SprechBlitz\*.jsonl`,
  u. a. Session `c2e72351-8b9b-4422-ac25-e024fe5113ca`. Im Projektordner: `claude --resume`.

---

## Konventionen

- Commit-Messages **deutsch**, Betreffzeile ohne Präfix, Umlaute in Commits ausgeschrieben
  (`fuer`, `Schluessel`) – bisherige Praxis, beibehalten.
- Benutzersichtbare Texte (UI, Fehlermeldungen, Notifications) auf Deutsch, Code-Kommentare englisch.
- Fehler aus `server_api.rs` sind bereits benutzerfertig formuliert – nicht nochmal umschreiben.
- Kein Frontend-Framework im Client; Vanilla TS beibehalten.
