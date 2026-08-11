import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const pill = document.getElementById("pill")!;
const label = document.getElementById("label")!;
const meterFill = document.getElementById("meter-fill")!;

type OverlayState = {
  state: "recording" | "transcribing" | "error";
  message?: string;
};

await listen<OverlayState>("overlay-state", (event) => {
  const { state, message } = event.payload;
  pill.className = state;
  if (state === "recording") label.textContent = "Aufnahme …";
  else if (state === "transcribing") label.textContent = "Transkribiere …";
  else label.textContent = message ?? "Fehler";
  if (state !== "recording") meterFill.style.width = "0%";
});

await listen<number>("level", (event) => {
  // RMS of speech is small; scale up and clamp for a lively meter.
  const pct = Math.min(100, Math.round(event.payload * 700));
  meterFill.style.width = `${pct}%`;
});

document.body.addEventListener("click", () => {
  void invoke("cancel_recording");
});
