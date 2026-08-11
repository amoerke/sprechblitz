# SprechBlitz Server

Zustandsloser Transkriptions-Proxy: nimmt WAV-Audio vom Client entgegen, ruft die OpenAI Audio API auf, ersetzt optional gesprochene Befehle und liefert den Text zurück. **Persistiert keine Inhalte** – geloggt werden nur Metadaten (Token-Name, Modell, Sprache, Audiogröße, Dauer).

## Endpunkte

| Methode | Pfad             | Auth   | Beschreibung |
|---------|------------------|--------|--------------|
| GET     | `/health`        | nein   | Liveness-Check |
| GET     | `/v1/models`     | Bearer | Verfügbare Modelle + Default |
| POST    | `/v1/transcribe` | Bearer | Multipart: `file` (WAV), Felder `model`, `language` (`de`/`en`/`auto`), `commands` (`1`/`0`) → `{ "text": "…" }` |

Auth: `Authorization: Bearer <token>` – Tokens sind benannt und einzeln widerrufbar (`AUTH_TOKENS`, siehe `.env.example`).

## Lokal entwickeln

```sh
cd server
npm install
cp .env.example .env   # Werte eintragen
npm run dev
```

## Deployment mit Coolify

1. In Coolify eine neue **Application** aus dem GitHub-Repo anlegen.
2. **Base Directory** auf `/server` setzen, Build Pack **Dockerfile**.
3. Env-Variablen setzen: `OPENAI_API_KEY`, `AUTH_TOKENS`.
4. Domain zuweisen (z. B. `diktat.deine-domain.de`) – HTTPS macht Coolify/Traefik via Let's Encrypt automatisch. Port der App: **8790**.
5. Auto-Deploy bei Push auf `main` aktivieren.

### Befehlsliste erweitern

`config/commands.de.json` enthält die Ersetzungsregeln (Reihenfolge zählt: längere Phrasen zuerst). Die Datei wird bei Änderung automatisch neu eingelesen. Soll sie ohne Deployment editierbar sein, in Coolify ein Volume auf `/app/config` mounten. Weitere Sprachen: `commands.en.json` usw. anlegen.

Neue Modelle: `config/models.json` erweitern – Clients holen die Liste beim Start von `/v1/models`, ein Client-Update ist nicht nötig.
