# SprechBlitz Server

Zustandsloser Transkriptions-Proxy: nimmt WAV-Audio vom Client entgegen, ruft je nach Modell die OpenAI Audio API oder ein selbst gehostetes Whisper ([speaches](https://speaches.ai)) auf, ersetzt optional gesprochene Befehle und liefert den Text zurück. **Persistiert keine Inhalte** – geloggt werden nur Metadaten (Token-Name, Modell, Sprache, Audiogröße, Dauer).

## Endpunkte

| Methode | Pfad             | Auth   | Beschreibung |
|---------|------------------|--------|--------------|
| GET     | `/health`        | nein   | Liveness-Check |
| GET     | `/v1/models`     | Bearer | Verfügbare Modelle + Default |
| POST    | `/v1/transcribe` | Bearer | Multipart: `file` (WAV, M4A, MP3 – alles, was die OpenAI Audio API annimmt), Felder `model`, `language` (`de`/`en`/`auto`), `commands` (`1`/`0`) → `{ "text": "…" }` |
| POST    | `/v1/parse-workout` | Bearer | JSON `{ "text": "…", "now": "<ISO>" }` → strukturiertes Workout (Titel, Muskelgruppen, Übungen mit Sätzen/Wdh./kg, Unklarheiten) – für die kraft.-App |
| POST    | `/v1/parse-meal` | Bearer | JSON `{ "text": "…", "now": "<ISO>" }` → Mahlzeit mit geschätzten kcal/Makros und Mahlzeit-Typ – für die kraft.-App |

Auth: `Authorization: Bearer <token>` – Tokens sind benannt und einzeln widerrufbar (`AUTH_TOKENS`, siehe `.env.example`).

Die Parse-Endpunkte nutzen OpenAI Chat Completions mit Structured Outputs; das Modell ist über `PARSE_MODEL` konfigurierbar (Default `gpt-4o-mini`). Geloggt werden auch hier nur Metadaten.

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

## Selbst gehostetes Whisper (speaches)

Modelle mit `"provider": "speaches"` in `config/models.json` laufen über [speaches](https://speaches.ai), einen OpenAI-kompatiblen Whisper-Server (faster-whisper). Das Audio verlässt dann deinen Server nicht. OpenAI und speaches lassen sich mischen: Im Client erscheinen alle Modelle der konfigurierten Anbieter.

| Variable | Bedeutung |
|---|---|
| `SPEACHES_BASE_URL` | z. B. `http://speaches:8000/v1` – aktiviert die speaches-Modelle |
| `SPEACHES_API_KEY` | nur, wenn speaches selbst mit `API_KEY` abgesichert ist |
| `OPENAI_API_KEY` | optional, sobald speaches konfiguriert ist; ohne ihn sind nur lokale Modelle verfügbar und `/v1/parse-*` antwortet mit 503 |

Der Server lädt beim Start alle speaches-Modelle aus `models.json` automatisch herunter (`POST /v1/models/{id}`, im Hintergrund). Bis ein Modell fertig ist, schlagen Transkriptionen damit fehl. Der Fortschritt steht im Log (`speaches: model ready`).

### Lokal mit Docker Compose

```sh
# .env: SPEACHES_BASE_URL=http://speaches:8000/v1
docker compose --profile speaches up -d
```

speaches bekommt keinen Port nach außen und ist nur für den SprechBlitz-Server im Compose-Netz erreichbar. Die Modelle liegen im Volume `speaches-hf-cache`. Für GPU: `SPEACHES_IMAGE=ghcr.io/speaches-ai/speaches:latest-cuda` setzen und den `deploy`-Block in `docker-compose.yml` einkommentieren.

### In Coolify

1. speaches als eigene Ressource anlegen (Docker Image `ghcr.io/speaches-ai/speaches:latest-cpu` bzw. `latest-cuda`, Port 8000, Env `ENABLE_UI=false`, persistentes Volume auf `/home/ubuntu/.cache/huggingface/hub`).
2. Am besten **ohne Domain**, nur im internen Coolify-Netz – dann beim SprechBlitz-Server `SPEACHES_BASE_URL=http://<interner-hostname>:8000/v1` setzen. Bekommt speaches eine öffentliche Domain, unbedingt `API_KEY` setzen und denselben Wert als `SPEACHES_API_KEY` beim SprechBlitz-Server eintragen.

### Modellwahl

| Modell | CPU-Tempo (8 s Audio, gemessen) | Hinweis |
|---|---|---|
| `Systran/faster-whisper-small` | ~3 s | gut für CPU-Server |
| `deepdml/faster-whisper-large-v3-turbo-ct2` | ~12 s | beste Qualität, auf CPU langsamer als Echtzeit → GPU empfohlen |

Der Client wartet maximal 120 s auf eine Antwort; mit turbo auf CPU reicht das nur für Diktate bis etwa eine Minute. Nur mehrsprachige Modelle eintragen – `distil-large-v3` u. ä. sind rein englisch.
