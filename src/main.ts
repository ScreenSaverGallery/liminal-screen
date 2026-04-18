// Liminal Screen — Options Window

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";

import { PowerMonitor } from "./app/power-monitor/power-monitor";
import { Preview } from "./app/preview/preview";
import { Signal } from "./app/reactive";
import type { AppOptions } from "./app/types";

// ── State ──────────────────────────────────────────────────────────────────

const options = new Signal<AppOptions | null>(null);

interface ScreensaverStatus { active: boolean; idleSeconds: number; }
const status = new Signal<ScreensaverStatus>({ active: false, idleSeconds: 0 });

const isActive   = status.derive(s => s.active);
const idleSignal = status.derive(s => s.idleSeconds);

let previewWindow: Preview | null = null;

// ── UI Elements ────────────────────────────────────────────────────────────

let idleTimeElement:    HTMLElement | null = null;
let statusTextElement:  HTMLElement | null = null;
let statusDotElement:   HTMLElement | null = null;
let startsInInput:      HTMLInputElement | null = null;
let displayOffInput:    HTMLInputElement | null = null;
let requirePassInInput: HTMLInputElement | null = null;
let runOnBatteryInput:  HTMLInputElement | null = null;
let debugInput:         HTMLInputElement | null = null;
let saverUrlDisplay:    HTMLElement | null = null;

// ── Helpers ────────────────────────────────────────────────────────────────

async function registerServiceWorker(): Promise<void> {
  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js").catch(() => {});
  }
}

async function openExternalLink(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch {
    window.open(url, "_blank");
  }
}

function formatIdle(secs: number): string {
  if (secs < 60)   return `Idle: ${Math.floor(secs)}s`;
  if (secs < 3600) return `Idle: ${Math.floor(secs / 60)}m ${Math.floor(secs % 60)}s`;
  return `Idle: ${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

// ── Init ───────────────────────────────────────────────────────────────────

async function init(): Promise<void> {
  console.log("Liminal Screen - Initializing...");
  try {
    options.set(await invoke<AppOptions>("get_options"));
    await registerServiceWorker();
    setupEventListeners();
    console.log("Liminal Screen - Ready", options.get());
  } catch (error) {
    console.error("Failed to initialize:", error);
  }
}

function setupEventListeners(): void {
  // Options sync
  listen<AppOptions>("options-updated", (event) => options.set(event.payload));
  listen("reset-options", async () => {
    try { options.set(await invoke<AppOptions>("get_options")); } catch { /* ignore */ }
  });

  // Window management
  listen("preview-screensaver", () => previewScreensaver());
  listen("open-options-window", async () => {
    try { await invoke("open_options"); } catch { /* ignore */ }
  });
  getCurrentWindow().onCloseRequested((event: any) => {
    event.preventDefault();
    getCurrentWindow().hide();
  });

  // Screensaver state — driven by Tauri events, not polling
  listen("screensaver-started", () => status.update(s => ({ ...s, active: true })));
  listen("screensaver-ended",   () => status.update(s => ({ ...s, active: false })));

  // Idle time — poll every second (no Rust event available for this yet)
  setInterval(async () => {
    try {
      const secs = await PowerMonitor.getSystemIdleTime();
      status.update(s => ({ ...s, idleSeconds: secs }));
    } catch { /* ignore */ }
  }, 1000);
}

// ── Form ───────────────────────────────────────────────────────────────────

function cacheUIElements(): void {
  idleTimeElement    = document.getElementById("idle-time");
  statusTextElement  = document.getElementById("status-text");
  statusDotElement   = document.querySelector(".status-dot");
  startsInInput      = document.getElementById("starts-in")       as HTMLInputElement | null;
  displayOffInput    = document.getElementById("display-off")     as HTMLInputElement | null;
  requirePassInInput = document.getElementById("require-pass-in") as HTMLInputElement | null;
  runOnBatteryInput  = document.getElementById("run-on-battery")  as HTMLInputElement | null;
  debugInput         = document.getElementById("debug-mode")      as HTMLInputElement | null;
  saverUrlDisplay    = document.getElementById("saver-url-display");

  [startsInInput, displayOffInput, requirePassInInput, runOnBatteryInput, debugInput]
    .forEach(el => el?.addEventListener("change", () => saveOptions(true)));
}

function setupUIButtonHandlers(): void {
  document.getElementById("save-btn")?.addEventListener("click", () => saveOptions());
  document.getElementById("preview-btn")?.addEventListener("click", () => previewScreensaver());
  document.getElementById("reset-btn")?.addEventListener("click", async () => {
    if (!confirm("Reset all options to defaults?")) return;
    try {
      await invoke("factory_reset_options");
      options.set(await invoke<AppOptions>("get_options"));
      // Form updates reactively via options.effect() — no alert needed
    } catch (error) {
      console.error("Failed to reset options:", error);
      alert("Failed to reset options. Please try again.");
    }
  });
}

async function saveOptions(silent = false): Promise<void> {
  const current = options.get();
  if (!current) return;

  const startsIn     = startsInInput      ? parseFloat(startsInInput.value)      : current.startsIn;
  const displayOffIn = displayOffInput    ? parseFloat(displayOffInput.value)    : current.displayOffIn;
  const requirePassIn = requirePassInInput ? parseFloat(requirePassInInput.value) : current.requirePassIn;
  const runOnBattery = runOnBatteryInput  ? runOnBatteryInput.checked            : current.runOnBattery;
  const debug        = debugInput         ? debugInput.checked                   : current.debug;

  if (isNaN(startsIn) || startsIn < 0.1) {
    if (!silent) alert("Start After must be at least 0.1 minutes");
    return;
  }
  if (isNaN(displayOffIn) || displayOffIn < 0.5) {
    if (!silent) alert("Display Off must be at least 0.5 minutes");
    return;
  }
  if (isNaN(requirePassIn) || requirePassIn < 0) {
    if (!silent) alert("Require Password must be 0 or a positive number");
    return;
  }

  try {
    await invoke("set_options", {
      options: { ...current, startsIn, displayOffIn, requirePassIn, runOnBattery, debug },
    });
    options.set(await invoke<AppOptions>("get_options"));
    if (!silent) alert("Settings saved successfully!");
  } catch {
    if (!silent) alert("Failed to save settings. Please try again.");
  }
}

// ── Preview ────────────────────────────────────────────────────────────────

async function previewScreensaver(): Promise<void> {
  if (previewWindow) await previewWindow.hide();
  try {
    const opts = options.get();
    const url = opts?.debug
      ? (import.meta.env.VITE_SAVER_URL_DEBUG || "https://save.screensaver.gallery/debug")
      : (import.meta.env.VITE_SAVER_URL       || "https://save.screensaver.gallery");
    previewWindow = new Preview(url);
    await previewWindow.show();
  } catch (error) {
    console.error("Failed to create preview window:", error);
  }
}

// ── Public API ─────────────────────────────────────────────────────────────

export async function forceDeactivateScreensaver(): Promise<void> {
  try {
    await invoke("deactivate_screensaver_command");
  } catch (error) {
    console.error("Failed to force deactivate screensaver:", error);
  }
}

export function isScreensaverRunning(): boolean {
  return isActive.get();
}

export function getCurrentOptions(): AppOptions | null {
  return options.get();
}

export async function openLink(url: string): Promise<void> {
  await openExternalLink(url);
}

// ── Bootstrap ──────────────────────────────────────────────────────────────

window.addEventListener("DOMContentLoaded", () => {
  cacheUIElements();
  setupUIButtonHandlers();

  // Reactive effects — each fires immediately then whenever the signal changes

  options.effect((opts) => {
    if (!opts) return;
    if (startsInInput)      startsInInput.value      = String(opts.startsIn);
    if (displayOffInput)    displayOffInput.value    = String(opts.displayOffIn);
    if (requirePassInInput) requirePassInInput.value = String(opts.requirePassIn);
    if (runOnBatteryInput)  runOnBatteryInput.checked = opts.runOnBattery;
    if (debugInput)         debugInput.checked        = opts.debug;
    if (saverUrlDisplay) {
      saverUrlDisplay.textContent =
        (opts.debug ? opts.saverUrlDebug : opts.saverUrl) || "Not configured";
    }
  });

  isActive.effect((active) => {
    if (!statusDotElement || !statusTextElement) return;
    statusTextElement.textContent = active ? "Active" : "Inactive";
    statusDotElement.classList.toggle("active",   active);
    statusDotElement.classList.toggle("inactive", !active);
  });

  idleSignal.effect((secs) => {
    if (idleTimeElement) idleTimeElement.textContent = formatIdle(secs);
  });

  init();
});

// Also init immediately for hidden-window scenarios (Tauri may not fire DOMContentLoaded)
try {
  init().catch(console.error);
} catch (error) {
  console.error("Immediate init threw error:", error);
}

(window as unknown as { liminalScreen: Record<string, unknown> }).liminalScreen = {
  deactivateScreensaver: forceDeactivateScreensaver,
  isScreensaverRunning,
  getCurrentOptions,
  openLink,
};
