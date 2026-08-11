import Fastify, { type FastifyError } from "fastify";
import multipart from "@fastify/multipart";
import { loadConfig, loadModelsConfig } from "./config.js";
import { authenticate } from "./auth.js";
import { loadCommands, applyCommands } from "./commands.js";
import { transcribe, UpstreamError } from "./transcribe.js";

const config = loadConfig();
const modelsConfig = loadModelsConfig(config.modelsConfigPath);

const app = Fastify({
  logger: { level: process.env.LOG_LEVEL ?? "info" },
  bodyLimit: config.maxUploadBytes,
});

await app.register(multipart, {
  limits: { fileSize: config.maxUploadBytes, files: 1 },
});

// --- Auth hook: everything except /health requires a named Bearer token ---
app.addHook("onRequest", async (request, reply) => {
  if (request.url === "/health") return;
  const name = authenticate(request.headers.authorization, config.tokens);
  if (!name) {
    return reply.code(401).send({ error: "unauthorized" });
  }
  request.tokenName = name;
});

declare module "fastify" {
  interface FastifyRequest {
    tokenName?: string;
  }
}

app.get("/health", async () => ({ ok: true }));

app.get("/v1/models", async () => modelsConfig);

app.post("/v1/transcribe", async (request, reply) => {
  const started = Date.now();

  const file = await request.file();
  if (!file) {
    return reply.code(400).send({ error: "missing audio file (multipart field 'file')" });
  }
  const audio = await file.toBuffer();

  // Multipart text fields arrive alongside the file part.
  const fieldValue = (name: string): string | undefined => {
    const field = file.fields[name];
    if (field && !Array.isArray(field) && field.type === "field") {
      return String(field.value);
    }
    return undefined;
  };

  const model = fieldValue("model") ?? modelsConfig.default;
  if (!modelsConfig.models.some((m) => m.id === model)) {
    return reply.code(400).send({ error: `unknown model "${model}"` });
  }

  const languageRaw = fieldValue("language") ?? "auto";
  const language = languageRaw === "auto" ? undefined : languageRaw;
  const commandsEnabled = fieldValue("commands") === "1";

  let rawText: string;
  try {
    rawText = await transcribe({
      apiKey: config.openaiApiKey,
      baseUrl: config.openaiBaseUrl,
      model,
      language,
      audio,
      filename: file.filename || "audio.wav",
      mimeType: file.mimetype || "audio/wav",
    });
  } catch (error) {
    if (error instanceof UpstreamError) {
      request.log.error(
        { status: error.status, upstreamBody: error.body },
        "openai request failed"
      );
      return reply.code(502).send({ error: "transcription service failed" });
    }
    throw error;
  }

  let text = rawText;
  if (commandsEnabled && language) {
    const commandsConfig = loadCommands(config.commandsConfigDir, language);
    if (commandsConfig) {
      text = applyCommands(rawText, commandsConfig);
    }
  }

  // Metadata-only logging: never log transcript content or audio.
  request.log.info(
    {
      token: request.tokenName,
      model,
      language: languageRaw,
      commands: commandsEnabled,
      audioBytes: audio.length,
      durationMs: Date.now() - started,
    },
    "transcription completed"
  );

  return { text, model };
});

app.setErrorHandler((error: FastifyError, request, reply) => {
  request.log.error(error);
  const status = error.statusCode && error.statusCode >= 400 ? error.statusCode : 500;
  reply.code(status).send({ error: status === 500 ? "internal error" : error.message });
});

await app.listen({ port: config.port, host: config.host });
