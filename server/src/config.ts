import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export interface ModelEntry {
  id: string;
  label: string;
}

export interface ModelsConfig {
  models: ModelEntry[];
  default: string;
}

export interface AppConfig {
  port: number;
  host: string;
  openaiApiKey: string;
  openaiBaseUrl: string;
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
    openaiApiKey: requireEnv("OPENAI_API_KEY"),
    openaiBaseUrl: process.env.OPENAI_BASE_URL ?? "https://api.openai.com/v1",
    tokens: parseTokens(requireEnv("AUTH_TOKENS")),
    modelsConfigPath: resolve(process.env.MODELS_CONFIG ?? "config/models.json"),
    commandsConfigDir: resolve(process.env.COMMANDS_CONFIG_DIR ?? "config"),
    maxUploadBytes: Number(process.env.MAX_UPLOAD_BYTES ?? 30 * 1024 * 1024),
    parseModel: process.env.PARSE_MODEL ?? "gpt-4o-mini",
  };
}

export function loadModelsConfig(path: string): ModelsConfig {
  const parsed = JSON.parse(readFileSync(path, "utf8")) as ModelsConfig;
  if (!Array.isArray(parsed.models) || parsed.models.length === 0) {
    throw new Error(`No models defined in ${path}`);
  }
  if (!parsed.models.some((m) => m.id === parsed.default)) {
    throw new Error(`Default model "${parsed.default}" is not in the models list`);
  }
  return parsed;
}
