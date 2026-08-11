<p align="center">
  <img src="client/src-tauri/icons/128x128.png" alt="SprechBlitz-Icon" width="96" height="96" />
</p>

# SprechBlitz

Diktier-App: Hotkey drücken, sprechen, Text erscheint an der Cursorposition der aktiven App.

- **`client/`** – Tauri-2-App (macOS-Menüleiste / Windows-Infobereich), Aufnahme per globalem Hotkey (Push-to-talk oder Toggle), Einfügen per Zwischenablage.
- **`server/`** – Node/TypeScript-Proxy, hält den OpenAI-API-Key, transkribiert via OpenAI Audio API, ersetzt gesprochene Befehle („neue Zeile“ …).

## Schnellstart

1. **Server deployen**: privates GitHub-Repo anlegen, in Coolify eine Application mit Base Directory `/server` (Dockerfile) erstellen, `OPENAI_API_KEY` und `AUTH_TOKENS` setzen, Subdomain zuweisen → [server/README.md](server/README.md)
2. **Client bauen/starten**: `cd client && npm install && npm run tauri dev` → [client/README.md](client/README.md)
3. Im Einstellungsfenster Server-URL + Token eintragen, „Verbindung testen“, speichern.
4. `Ctrl+Alt+D` drücken, sprechen, nochmal drücken – der Text erscheint am Cursor.

## Funktionsumfang

- Push-to-talk **und** Toggle-Modus, Hotkey frei konfigurierbar
- Overlay oben mittig mit Pegelanzeige; Klick oder Esc bricht ab; Start/Stopp-Töne
- Modell- und Sprachwahl (de/en/auto) im Tray, Modellliste kommt vom Server
- Sprachbefehle („neue Zeile“, „neuer Absatz“, „in Klammern … Klammer zu“ …), serverseitig gepflegt
- Auto-Stopp nach 5 Minuten, Retry nach Fehlern, lokaler Verlauf (letzte 20)
- Autostart, Auto-Updater (GitHub Releases), benannte & einzeln widerrufbare Server-Tokens
- Server loggt ausschließlich Metadaten – keine Inhalte, kein Audio
