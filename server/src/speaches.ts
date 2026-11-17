import type { FastifyBaseLogger } from "fastify";

/**
 * speaches only serves models that are installed locally; unknown ones fail with 404
 * ("not installed locally"). This makes sure every speaches model from models.json is
 * downloaded, so adding a model means editing models.json only.
 *
 * Runs in the background: retries until speaches is reachable, then downloads missing
 * models one by one (POST /v1/models/{id} blocks until the download is done).
 */
export function ensureSpeachesModels(
  baseUrl: string,
  apiKey: string | undefined,
  modelIds: string[],
  log: FastifyBaseLogger
): void {
  if (modelIds.length === 0) return;
  const headers: Record<string, string> = apiKey ? { Authorization: `Bearer ${apiKey}` } : {};

  const run = async (): Promise<void> => {
    let installed: Set<string> | undefined;
    for (let attempt = 1; !installed; attempt++) {
      try {
        const response = await fetch(`${baseUrl}/models`, { headers });
        if (!response.ok) throw new Error(`status ${response.status}`);
        const body = (await response.json()) as { data?: { id: string }[] };
        installed = new Set((body.data ?? []).map((m) => m.id));
      } catch (error) {
        const delayS = Math.min(60, attempt * 5);
        log.warn({ attempt, retryInS: delayS, err: String(error) }, "speaches not reachable yet");
        await new Promise((r) => setTimeout(r, delayS * 1000));
      }
    }

    for (const id of modelIds) {
      if (installed.has(id)) continue;
      const started = Date.now();
      log.info({ model: id }, "speaches: downloading model");
      try {
        const response = await fetch(`${baseUrl}/models/${id}`, { method: "POST", headers });
        if (!response.ok) {
          log.error({ model: id, status: response.status, body: await response.text() }, "speaches: model download failed");
          continue;
        }
        log.info({ model: id, durationMs: Date.now() - started }, "speaches: model ready");
      } catch (error) {
        log.error({ model: id, err: String(error) }, "speaches: model download failed");
      }
    }
  };

  void run();
}
