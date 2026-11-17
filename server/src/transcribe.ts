/** Works against any OpenAI-compatible /audio/transcriptions endpoint (OpenAI, speaches). */
export interface TranscribeOptions {
  /** Omitted for upstreams without auth (e.g. speaches without API_KEY) */
  apiKey?: string;
  baseUrl: string;
  model: string;
  /** ISO-639-1 code, undefined = auto-detect */
  language?: string;
  audio: Buffer;
  filename: string;
  mimeType: string;
}

export class UpstreamError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: string
  ) {
    super(`Transcription upstream failed with status ${status}`);
  }
}

export async function transcribe(options: TranscribeOptions): Promise<string> {
  const form = new FormData();
  form.append(
    "file",
    new Blob([new Uint8Array(options.audio)], { type: options.mimeType }),
    options.filename
  );
  form.append("model", options.model);
  form.append("response_format", "json");
  if (options.language) {
    form.append("language", options.language);
  }

  const response = await fetch(`${options.baseUrl}/audio/transcriptions`, {
    method: "POST",
    headers: options.apiKey ? { Authorization: `Bearer ${options.apiKey}` } : {},
    body: form,
  });

  if (!response.ok) {
    throw new UpstreamError(response.status, await response.text());
  }

  const result = (await response.json()) as { text?: string };
  return result.text ?? "";
}
