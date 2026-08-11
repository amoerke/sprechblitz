import { readFileSync, statSync, existsSync } from "node:fs";
import { join } from "node:path";

interface CommandRule {
  phrase: string;
  replace: string;
}

interface CommandsConfig {
  language: string;
  rules: CommandRule[];
}

interface CachedConfig {
  mtimeMs: number;
  config: CommandsConfig;
}

const cache = new Map<string, CachedConfig>();

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Loads commands.<lang>.json from the config dir. Re-reads when the file's
 * mtime changes, so the list can be edited on a mounted volume without restart.
 */
export function loadCommands(configDir: string, language: string): CommandsConfig | null {
  const path = join(configDir, `commands.${language}.json`);
  if (!existsSync(path)) return null;
  const mtimeMs = statSync(path).mtimeMs;
  const cached = cache.get(path);
  if (cached && cached.mtimeMs === mtimeMs) return cached.config;
  const config = JSON.parse(readFileSync(path, "utf8")) as CommandsConfig;
  cache.set(path, { mtimeMs, config });
  return config;
}

/**
 * Replaces spoken commands ("neue Zeile", …) with their symbols.
 * Rules are applied in config order, so longer phrases must come first
 * ("Anführungszeichen Ende" before "Anführungszeichen").
 * Surrounding punctuation that Whisper tends to add around command words
 * is consumed together with the phrase — except a leading period, which is
 * usually the real end of the previous sentence and must survive.
 */
export function applyCommands(text: string, config: CommandsConfig): string {
  let result = text;
  for (const rule of config.rules) {
    const pattern = new RegExp(
      `[ \\t]*[,;:]?[ \\t]*\\b${escapeRegExp(rule.phrase)}\\b[ \\t]*[,.;:]?`,
      "giu"
    );
    result = result.replace(pattern, rule.replace);
  }
  return cleanup(result);
}

function cleanup(text: string): string {
  return (
    text
      // no space after opening bracket/quote, none before closing ones
      .replace(/([(„])[ \t]+/g, "$1")
      .replace(/[ \t]+([)“])/g, "$1")
      // no space before punctuation
      .replace(/[ \t]+([,.;:!?])/g, "$1")
      // collapse runs of spaces, strip spaces at line edges
      .replace(/[ \t]{2,}/g, " ")
      .replace(/[ \t]+\n/g, "\n")
      .replace(/\n[ \t]+/g, "\n")
      // sentence after a newline starts uppercase
      .replace(/\n([a-zäöüß])/gu, (_, ch: string) => "\n" + ch.toUpperCase())
      .trim()
  );
}
