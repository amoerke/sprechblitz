import { invoke } from "@tauri-apps/api/core";

interface Settings {
  server_url: string;
  token: string;
  hotkey: string;
  mode: "ptt" | "toggle";
  model: string;
  language: "de" | "en" | "auto";
  commands_enabled: boolean;
  sounds_enabled: boolean;
  autostart: boolean;
  history_enabled: boolean;
  mic_device: string | null;
  max_seconds: number;
}

interface ModelEntry {
  id: string;
  label: string;
}

interface HistoryEntry {
  timestamp: string;
  text: string;
}

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const serverUrl = el<HTMLInputElement>("server-url");
const token = el<HTMLInputElement>("token");
const model = el<HTMLSelectElement>("model");
const language = el<HTMLSelectElement>("language");
const mode = el<HTMLSelectElement>("mode");
const hotkey = el<HTMLInputElement>("hotkey");
const commands = el<HTMLInputElement>("commands");
const sounds = el<HTMLInputElement>("sounds");
const maxSeconds = el<HTMLInputElement>("max-seconds");
const mic = el<HTMLSelectElement>("mic");
const autostart = el<HTMLInputElement>("autostart");
const historyEnabled = el<HTMLInputElement>("history-enabled");
const historyBox = el<HTMLDivElement>("history");
const connectionStatus = el<HTMLSpanElement>("connection-status");
const saveStatus = el<HTMLSpanElement>("save-status");

function fillModels(models: ModelEntry[], selected: string) {
  model.innerHTML = "";
  const list = models.length > 0 ? models : [{ id: selected, label: selected }];
  for (const m of list) {
    const option = document.createElement("option");
    option.value = m.id;
    option.textContent = m.label;
    model.appendChild(option);
  }
  if (!list.some((m) => m.id === selected)) {
    const option = document.createElement("option");
    option.value = selected;
    option.textContent = selected;
    model.appendChild(option);
  }
  model.value = selected;
}

async function loadAll() {
  const settings = await invoke<Settings>("get_settings");
  serverUrl.value = settings.server_url;
  token.value = settings.token;
  language.value = settings.language;
  mode.value = settings.mode;
  hotkey.value = settings.hotkey;
  commands.checked = settings.commands_enabled;
  sounds.checked = settings.sounds_enabled;
  maxSeconds.value = String(settings.max_seconds);
  autostart.checked = settings.autostart;
  historyEnabled.checked = settings.history_enabled;

  const models = await invoke<ModelEntry[]>("get_cached_models");
  fillModels(models, settings.model);

  const devices = await invoke<string[]>("list_input_devices");
  mic.innerHTML = "";
  const defaultOption = document.createElement("option");
  defaultOption.value = "";
  defaultOption.textContent = "Systemstandard";
  mic.appendChild(defaultOption);
  for (const device of devices) {
    const option = document.createElement("option");
    option.value = device;
    option.textContent = device;
    mic.appendChild(option);
  }
  mic.value = settings.mic_device ?? "";

  await loadHistory();
}

async function loadHistory() {
  const entries = await invoke<HistoryEntry[]>("get_history");
  historyBox.innerHTML = "";
  if (entries.length === 0) {
    historyBox.innerHTML = '<span class="history-empty">Noch keine Einträge.</span>';
    return;
  }
  for (const entry of entries) {
    const row = document.createElement("div");
    row.className = "history-entry";
    const text = document.createElement("span");
    text.className = "text";
    text.textContent = entry.text;
    text.title = entry.text;
    const copy = document.createElement("button");
    copy.textContent = "Kopieren";
    copy.addEventListener("click", async () => {
      await invoke("copy_text", { text: entry.text });
      copy.textContent = "Kopiert ✓";
      setTimeout(() => (copy.textContent = "Kopieren"), 1500);
    });
    row.append(text, copy);
    historyBox.appendChild(row);
  }
}

// --- Hotkey capture: click the field, press the combination ---
hotkey.addEventListener("keydown", (event) => {
  event.preventDefault();
  if (event.key === "Escape") {
    hotkey.blur();
    return;
  }
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("ctrl");
  if (event.altKey) parts.push("alt");
  if (event.shiftKey) parts.push("shift");
  if (event.metaKey) parts.push("super");
  const key = event.key.toLowerCase();
  if (["control", "alt", "shift", "meta"].includes(key)) return;
  const keyName = event.code.startsWith("Key")
    ? event.code.slice(3).toLowerCase()
    : event.code.startsWith("Digit")
      ? event.code.slice(5)
      : key === " "
        ? "space"
        : key;
  if (parts.length === 0) return; // require at least one modifier
  parts.push(keyName);
  hotkey.value = parts.join("+");
});

el<HTMLButtonElement>("test-connection").addEventListener("click", async () => {
  connectionStatus.textContent = "Prüfe …";
  connectionStatus.className = "";
  try {
    const models = await invoke<ModelEntry[]>("test_connection", {
      serverUrl: serverUrl.value.trim(),
      token: token.value.trim(),
    });
    connectionStatus.textContent = `Verbunden – ${models.length} Modelle verfügbar`;
    connectionStatus.className = "ok";
    fillModels(models, model.value);
  } catch (error) {
    connectionStatus.textContent = String(error);
    connectionStatus.className = "err";
  }
});

el<HTMLButtonElement>("clear-history").addEventListener("click", async () => {
  await invoke("clear_history");
  await loadHistory();
});

el<HTMLButtonElement>("save").addEventListener("click", async () => {
  saveStatus.textContent = "Speichere …";
  saveStatus.className = "";
  const settings: Settings = {
    server_url: serverUrl.value.trim().replace(/\/+$/, ""),
    token: token.value.trim(),
    hotkey: hotkey.value.trim() || "ctrl+alt+d",
    mode: mode.value as Settings["mode"],
    model: model.value,
    language: language.value as Settings["language"],
    commands_enabled: commands.checked,
    sounds_enabled: sounds.checked,
    autostart: autostart.checked,
    history_enabled: historyEnabled.checked,
    mic_device: mic.value === "" ? null : mic.value,
    max_seconds: Math.max(30, Math.min(1800, Number(maxSeconds.value) || 300)),
  };
  try {
    await invoke("save_settings", { settings });
    saveStatus.textContent = "Gespeichert ✓";
    saveStatus.className = "ok";
    setTimeout(() => (saveStatus.textContent = ""), 2000);
  } catch (error) {
    saveStatus.textContent = String(error);
    saveStatus.className = "err";
  }
});

void loadAll();
