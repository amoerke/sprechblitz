import { readFileSync } from "node:fs";
import { resolve } from "node:path";

/** Which upstream serves a model. Defaults to "openai" when omitted. */
export type Provider = "openai" | "speaches";

export interface ModelEntry {
  id: string;
  label: string;
  provider?: Provider;
}

export interface ModelsConfig {
  models: ModelEntry[];
  default: string;
}

export interface AppConfig {
  port: number;
  host: string;
  /** undefined = OpenAI disabled (fully self-hosted setup) */
  openaiApiKey?: string;
  openaiBaseUrl: string;
  /** e.g. http://speaches:8000/v1 – undefined = speaches disabled */
  speachesBaseUrl?: string;
  /** Only needed if speaches itself runs with API_KEY set */
  speachesApiKey?: string;
  /** name -> token */
  tokens: Map<string, string>;
  modelsConfigPath: string;
  commandsConfigDir: string;
  maxUploadBytes: number;
  /** Chat model for the kraft. parse endpoints */
  parseModel: string;
}

// Load ./.env if present (local development; in Docker/Coolify the
// environment comes from the platform). Does not override existing vars.
try {
  process.loadEnvFile();
} catch {
  // no .env file – fine
}

/** Treats unset and empty variables alike (Compose passes "" for empty defaults). */
function optionalEnv(name: string): string | undefined {
  const value = process.env[name]?.trim();
  return value ? value : undefined;
}

function requireEnv(name: string): string {
  const value = process.env[name];
  if (!value) {
    throw new Error(`Missing required environment variable: ${name}`);
  }
  return value;
}

/** AUTH_TOKENS format: "andre:secret1,kollege:secret2" */
function parseTokens(raw: string): Map<string, string> {
  const tokens = new Map<string, string>();
  for (const entry of raw.split(",")) {
    const trimmed = entry.trim();
    if (!trimmed) continue;
    const sep = trimmed.indexOf(":");
    if (sep <= 0 || sep === trimmed.length - 1) {
      throw new Error(`Invalid AUTH_TOKENS entry (expected name:token): "${trimmed}"`);
    }
    const name = trimmed.slice(0, sep).trim();
    const token = trimmed.slice(sep + 1).trim();
    if (token.length < 16) {
      throw new Error(`Token for "${name}" is too short (min. 16 characters)`);
    }
    tokens.set(name, token);
  }
  if (tokens.size === 0) {
    throw new Error("AUTH_TOKENS contains no valid entries");
  }
  return tokens;
}

export function loadConfig(): AppConfig {
  return {
    port: Number(process.env.PORT ?? 8790),
    host: process.env.HOST ?? "0.0.0.0",
    openaiApiKey: optionalEnv("OPENAI_API_KEY"),
    openaiBaseUrl: optionalEnv("OPENAI_BASE_URL") ?? "https://api.openai.com/v1",
    speachesBaseUrl: optionalEnv("SPEACHES_BASE_URL")?.replace(/\/+$/, ""),
    speachesApiKey: optionalEnv("SPEACHES_API_KEY"),
    tokens: parseTokens(requireEnv("AUTH_TOKENS")),
    modelsConfigPath: resolve(process.env.MODELS_CONFIG ?? "config/models.json"),
    commandsConfigDir: resolve(process.env.COMMANDS_CONFIG_DIR ?? "config"),
    maxUploadBytes: Number(process.env.MAX_UPLOAD_BYTES ?? 30 * 1024 * 1024),
    parseModel: process.env.PARSE_MODEL ?? "gpt-4o-mini",
  };
}

export function providerOf(model: ModelEntry): Provider {
  return model.provider ?? "openai";
}

/**
 * Loads models.json and drops models whose provider is not configured, so one
 * models.json serves OpenAI-only, speaches-only and mixed deployments.
 * Returns the dropped model ids for a startup log line.
 */
export function loadModelsConfig(
  path: string,
  config: AppConfig
): { modelsConfig: ModelsConfig; dropped: string[] } {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as ModelsConfig;
  if (!Array.isArray(parsed.models) || parsed.models.length === 0) {
    throw new Error(`No models defined in ${path}`);
  }
  for (const m of parsed.models) {
    const provider = providerOf(m);
    if (provider !== "openai" && provider !== "speaches") {
      throw new Error(`Model "${m.id}" has unknown provider "${provider}"`);
    }
  }
  if (!parsed.models.some((m) => m.id === parsed.default)) {
    throw new Error(`Default model "${parsed.default}" is not in the models list`);
  }

  const enabled = (m: ModelEntry): boolean =>
    providerOf(m) === "openai" ? Boolean(config.openaiApiKey) : Boolean(config.speachesBaseUrl);
  const models = parsed.models.filter(enabled);
  const dropped = parsed.models.filter((m) => !enabled(m)).map((m) => m.id);

  if (models.length === 0) {
    throw new Error(
      `No usable models in ${path}: set OPENAI_API_KEY and/or SPEACHES_BASE_URL for the providers listed there`
    );
  }
  // Default belongs to a disabled provider → fall back to the first usable model.
  const defaultModel = models.some((m) => m.id === parsed.default) ? parsed.default : models[0].id;
  return { modelsConfig: { models, default: defaultModel }, dropped };
}
