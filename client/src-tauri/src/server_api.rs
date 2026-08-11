use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct ModelsResponse {
    #[serde(rename = "default")]
    #[allow(dead_code)]
    pub default_model: String,
    pub models: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct TranscribeResponse {
    text: String,
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .expect("reqwest client")
}

fn network_error(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "Zeitüberschreitung – Server antwortet nicht".to_string()
    } else if e.is_connect() {
        "Server nicht erreichbar".to_string()
    } else {
        format!("Netzwerkfehler: {e}")
    }
}

async fn ensure_ok(response: reqwest::Response) -> Result<reqwest::Response, String> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    Err(match status.as_u16() {
        401 => "Zugriff verweigert – Token prüfen".to_string(),
        502 => "Transkriptionsdienst (OpenAI) meldet einen Fehler".to_string(),
        code => format!("Serverfehler (HTTP {code})"),
    })
}

pub async fn fetch_models(base_url: &str, token: &str) -> Result<ModelsResponse, String> {
    if base_url.is_empty() {
        return Err("Server-URL ist nicht konfiguriert".to_string());
    }
    let response = client()
        .get(format!("{base_url}/v1/models"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(network_error)?;
    ensure_ok(response)
        .await?
        .json::<ModelsResponse>()
        .await
        .map_err(|e| format!("Ungültige Serverantwort: {e}"))
}

pub async fn transcribe(
    base_url: &str,
    token: &str,
    wav: Vec<u8>,
    model: &str,
    language: &str,
    commands: bool,
) -> Result<String, String> {
    if base_url.is_empty() {
        return Err("Server-URL ist nicht konfiguriert – Einstellungen öffnen".to_string());
    }
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string())
        .text("language", language.to_string())
        .text("commands", if commands { "1" } else { "0" });

    let response = client()
        .post(format!("{base_url}/v1/transcribe"))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(network_error)?;
    let response = ensure_ok(response).await?;
    Ok(response
        .json::<TranscribeResponse>()
        .await
        .map_err(|e| format!("Ungültige Serverantwort: {e}"))?
        .text)
}
