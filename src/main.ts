// Liminal Screen - Main Application Entry Point

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { PowerMonitor } from "./app/power-monitor/power-monitor";
import { Preview } from "./app/preview/preview";
import { Storage } from "./app/storage/storage";
import type { AppOptions } from "./app/storage/storage";
import {
  OptionsManager,
  registerServiceWorker,
  openExternalLink,
} from "./app/options/options";
import { Signal } from "./app/reactive";

let isScreensaverActive = false;
let previewWindow: Preview | null = null;
let displayOffTimeout: number | null = null;

const options = new Signal<AppOptions | null>(null);

const MONITORING_INTERVAL_MS = 1000;

// UI Elements (cached after DOM load)
let idleTimeElement: HTMLElement | null = null;
let statusTextElement: HTMLElement | null = null;
let statusDotElement: HTMLElement | null = null;
let startsInInput: HTMLInputElement | null = null;
let displayOffInput: HTMLInputElement | null = null;
let requirePassInInput: HTMLInputElement | null = null;
let runOnBatteryInput: HTMLInputElement | null = null;
let debugInput: HTMLInputElement | null = null;
let saverUrlDisplay: HTMLElement | null = null;

async function init(): Promise<void> {
  console.log("Liminal Screen - Initializing UI...");

  try {
    await Storage.init();
    await OptionsManager.init();
    await loadOptions();
    await registerServiceWorker();
    setupEventListeners();

    console.log("Liminal Screen - UI Initialized successfully");
    console.log("Current options:", options.get());
  } catch (error) {
    console.error("Failed to initialize application:", error);
  }
}

async function loadOptions(): Promise<void> {
  try {
    options.set(await invoke<AppOptions>("get_options"));
    console.log("Options loaded:", options.get());
  } catch (error) {
    console.error("Failed to load options:", error);
  }
}

function setupEventListeners(): void {
  listen<AppOptions>("options-updated", (event) => {
    console.log("Options updated:", event.payload);
    options.set(event.payload);
  });

  listen("preview-screensaver", () => {
    console.log("Preview screensaver requested");
    previewScreensaver();
  });

  listen("open-options-window", () => {
    console.log("Open options window requested");
    openOptionsWindow();
  });

  listen("reset-options", () => {
    console.log("Reset options requested");
    loadOptions();
  });

  getCurrentWindow().onCloseRequested((event: any) => {
    event.preventDefault();
    getCurrentWindow().hide();
  });

  listen("screensaver-started", async () => {
    console.log("Screensaver started (from Rust engine)");
    isScreensaverActive = true;
    updateStatusDisplay();
  });

  listen("screensaver-ended", async () => {
    console.log("Screensaver ended (from Rust engine)");
    isScreensaverActive = false;
    updateStatusDisplay();

    if (displayOffTimeout) {
      clearTimeout(displayOffTimeout);
      displayOffTimeout = null;
    }
  });

  setInterval(async () => {
    try {
      const status = await invoke<any>("get_screensaver_status");
      isScreensaverActive = status.is_active;
      updateStatusDisplay();

      if (idleTimeElement) {
        try {
          const idleTime = await PowerMonitor.getSystemIdleTime();
          updateIdleTimeDisplay(idleTime);
        } catch (e) {
          console.warn("Failed to get idle time for display:", e);
        }
      }
    } catch (e) {
      console.warn("Failed to sync with screensaver engine:", e);
    }
  }, MONITORING_INTERVAL_MS);
}

function setupUIButtonHandlers(): void {
  document.getElementById("save-btn")?.addEventListener("click", async () => {
    console.log("Save button clicked");
    await saveOptions();
  });

  document.getElementById("preview-btn")?.addEventListener("click", async () => {
    console.log("Preview button clicked");
    await previewScreensaver();
  });

  document.getElementById("reset-btn")?.addEventListener("click", async () => {
    if (!confirm("Reset all options to defaults?")) return;
    try {
      options.set(await invoke<AppOptions>("factory_reset_options"));
      alert("Options reset to defaults");
    } catch (error) {
      console.error("Failed to reset options:", error);
      alert("Failed to reset options. Please try again.");
    }
  });
}

function updateIdleTimeDisplay(idleSeconds: number): void {
  if (!idleTimeElement) return;

  if (idleSeconds < 60) {
    idleTimeElement.textContent = `Idle: ${Math.floor(idleSeconds)}s`;
  } else if (idleSeconds < 3600) {
    const minutes = Math.floor(idleSeconds / 60);
    const seconds = Math.floor(idleSeconds % 60);
    idleTimeElement.textContent = `Idle: ${minutes}m ${seconds}s`;
  } else {
    const hours = Math.floor(idleSeconds / 3600);
    const minutes = Math.floor((idleSeconds % 3600) / 60);
    idleTimeElement.textContent = `Idle: ${hours}h ${minutes}m`;
  }
}

function updateStatusDisplay(): void {
  if (!statusTextElement || !statusDotElement) return;

  if (isScreensaverActive) {
    statusTextElement.textContent = "Active";
    statusDotElement.classList.remove("inactive");
    statusDotElement.classList.add("active");
  } else {
    statusTextElement.textContent = "Inactive";
    statusDotElement.classList.remove("active");
    statusDotElement.classList.add("inactive");
  }
}

async function saveOptions(silent = false): Promise<void> {
  const current = options.get();
  if (!current) return;

  const startsIn = startsInInput ? parseFloat(startsInInput.value) : current.startsIn;
  const displayOffIn = displayOffInput ? parseFloat(displayOffInput.value) : current.displayOffIn;
  const requirePassIn = requirePassInInput ? parseFloat(requirePassInInput.value) : current.requirePassIn;
  const runOnBattery = runOnBatteryInput ? runOnBatteryInput.checked : current.runOnBattery;
  const debug = debugInput ? debugInput.checked : current.debug;

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
    console.log("Options saved:", options.get());
    if (!silent) alert("Settings saved successfully!");
  } catch (error) {
    console.error("Failed to save options:", error);
    if (!silent) alert("Failed to save settings. Please try again.");
  }
}

function cacheUIElements(): void {
  idleTimeElement = document.getElementById("idle-time");
  statusTextElement = document.getElementById("status-text");
  statusDotElement = document.querySelector(".status-dot");
  startsInInput = document.getElementById("starts-in") as HTMLInputElement | null;
  displayOffInput = document.getElementById("display-off") as HTMLInputElement | null;
  requirePassInInput = document.getElementById("require-pass-in") as HTMLInputElement | null;
  runOnBatteryInput = document.getElementById("run-on-battery") as HTMLInputElement | null;
  debugInput = document.getElementById("debug-mode") as HTMLInputElement | null;
  saverUrlDisplay = document.getElementById("saver-url-display");

  [startsInInput, displayOffInput, requirePassInInput, runOnBatteryInput, debugInput]
    .forEach((el) => el?.addEventListener("change", () => saveOptions(true)));
}

async function previewScreensaver(): Promise<void> {
  if (previewWindow) {
    await previewWindow.hide();
  }

  try {
    const opts = options.get();
    const previewUrl = opts?.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG || "https://save.screensaver.gallery/debug"
      : import.meta.env.VITE_SAVER_URL || "https://save.screensaver.gallery";

    previewWindow = new Preview(previewUrl);
    await previewWindow.show();

    console.log("Preview window created. Use window.closePreviewWindow() to close it manually if needed.");
  } catch (error) {
    console.error("Failed to create preview window:", error);
  }
}

async function openOptionsWindow(): Promise<void> {
  try {
    await invoke("open_options");
  } catch (error) {
    console.error("Failed to open options window:", error);
  }
}

export async function forceDeactivateScreensaver(): Promise<void> {
  try {
    await invoke("deactivate_screensaver_command");
  } catch (error) {
    console.error("Failed to force deactivate screensaver:", error);
  }
}

export function isScreensaverRunning(): boolean {
  return isScreensaverActive;
}

export function getCurrentOptions(): AppOptions | null {
  return options.get();
}

export async function openLink(url: string): Promise<void> {
  await openExternalLink(url);
}

window.addEventListener("DOMContentLoaded", () => {
  cacheUIElements();
  setupUIButtonHandlers();

  // Single reactive effect — reruns automatically whenever options.set() is called
  options.effect((opts) => {
    if (!opts) return;
    if (startsInInput) startsInInput.value = String(opts.startsIn);
    if (displayOffInput) displayOffInput.value = String(opts.displayOffIn);
    if (requirePassInInput) requirePassInInput.value = String(opts.requirePassIn);
    if (runOnBatteryInput) runOnBatteryInput.checked = opts.runOnBattery;
    if (debugInput) debugInput.checked = opts.debug;
    if (saverUrlDisplay) {
      saverUrlDisplay.textContent =
        (opts.debug ? opts.saverUrlDebug : opts.saverUrl) || "Not configured";
    }
  });

  init();
});

try {
  console.log("Liminal Screen immediate initialization attempt");
  init().catch((error) => {
    console.error("Immediate init failed:", error);
  });
} catch (error) {
  console.error("Immediate init threw error:", error);
}

(window as unknown as { liminalScreen: Record<string, unknown> }).liminalScreen = {
  deactivateScreensaver: forceDeactivateScreensaver,
  isScreensaverRunning,
  getCurrentOptions,
  openLink,
};
