// Remote Options Script
// Built-in fallback options page — shown in the main window when no remote options URL is configured.
// For a remote options page, the fork developer hosts their own page and uses liminal-api.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AppOptions } from "../storage/storage";
import { Signal } from "../reactive";

const isTauri = (() => {
  try {
    return typeof window !== "undefined" && (window as any)["__TAURI__"] !== undefined;
  } catch {
    return false;
  }
})();

// UI Elements (cached after DOM load)
let startsInInput: HTMLInputElement | null = null;
let displayOffInput: HTMLInputElement | null = null;
let requirePassInInput: HTMLInputElement | null = null;
let runOnBatteryInput: HTMLInputElement | null = null;
let debugInput: HTMLInputElement | null = null;
let saverUrlDisplay: HTMLElement | null = null;
let statusTextElement: HTMLElement | null = null;
let statusDotElement: HTMLElement | null = null;

const options = new Signal<AppOptions | null>(null);

async function init(): Promise<void> {
  console.log("Remote Options - Initializing...");

  try {
    cacheUIElements();

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

    setupEventListeners();

    if (isTauri) {
      await loadOptions();
      setupIPCListeners();
      console.log("Remote Options - Initialized with Tauri support");
    } else {
      console.log("Remote Options - Non-Tauri environment, loading mock options");
      loadMockOptions();
    }
  } catch (error) {
    console.error("Failed to initialize remote options:", error);
  }
}

function cacheUIElements(): void {
  startsInInput = document.getElementById("starts-in") as HTMLInputElement | null;
  displayOffInput = document.getElementById("display-off") as HTMLInputElement | null;
  requirePassInInput = document.getElementById("require-pass-in") as HTMLInputElement | null;
  runOnBatteryInput = document.getElementById("run-on-battery") as HTMLInputElement | null;
  debugInput = document.getElementById("debug-mode") as HTMLInputElement | null;
  saverUrlDisplay = document.getElementById("saver-url-display");
  statusTextElement = document.getElementById("status-text");
  statusDotElement = document.querySelector(".status-dot");
}

function setupEventListeners(): void {
  document.getElementById("save-btn")?.addEventListener("click", () => saveOptions());
  document.getElementById("preview-btn")?.addEventListener("click", () => previewScreensaver());
  document.getElementById("reset-btn")?.addEventListener("click", () => {
    if (confirm("Reset all options to defaults?")) resetOptions();
  });

  [startsInInput, displayOffInput, requirePassInInput, runOnBatteryInput, debugInput]
    .forEach((el) => el?.addEventListener("change", () => saveOptions(true)));
}

function setupIPCListeners(): void {
  listen("options-updated", async () => {
    await loadOptions();
  });
}

async function loadOptions(): Promise<void> {
  try {
    options.set(await invoke<AppOptions>("get_options"));
    console.log("Loaded options:", options.get());
    updateStatusDisplay(false);
  } catch (error) {
    console.error("Failed to load options:", error);
  }
}

function loadMockOptions(): void {
  options.set({
    saverUrl: "https://save.screensaver.gallery",
    saverUrlDebug: "https://save.screensaver.gallery/debug",
    optionsUrl: "",
    appName: "Liminal Screen",
    appDescription: "",
    startsIn: 0.2,
    displayOffIn: 1.0,
    requirePassIn: 1.0,
    runOnBattery: false,
    debug: false,
    customOptions: {},
  });
}

function updateStatusDisplay(isActive: boolean): void {
  if (!statusTextElement || !statusDotElement) return;
  statusTextElement.textContent = isActive ? "Active" : "Inactive";
  statusDotElement.classList.toggle("active", isActive);
  statusDotElement.classList.toggle("inactive", !isActive);
}

async function saveOptions(silent = false): Promise<void> {
  try {
    const current = isTauri ? await invoke<AppOptions>("get_options") : options.get();

    const startsIn = startsInInput ? parseFloat(startsInInput.value) : 0.2;
    const displayOffIn = displayOffInput ? parseFloat(displayOffInput.value) : 1.0;
    const requirePassIn = requirePassInInput ? parseFloat(requirePassInInput.value) : 0.0;

    if (isNaN(startsIn) || startsIn < 0.1) {
      if (!silent) alert("Start After must be at least 0.1 minutes");
      return;
    }
    if (isNaN(displayOffIn) || displayOffIn < 0.5) {
      if (!silent) alert("Display Off must be at least 0.5 minutes");
      return;
    }
    if (isNaN(requirePassIn) || requirePassIn < 0) {
      if (!silent) alert("Require Password must be 0 (disabled) or a positive number of minutes");
      return;
    }

    const newOptions: AppOptions = {
      saverUrl: current?.saverUrl ?? "",
      saverUrlDebug: current?.saverUrlDebug ?? "",
      optionsUrl: current?.optionsUrl ?? "",
      appName: current?.appName ?? "Liminal Screen",
      appDescription: current?.appDescription ?? "",
      startsIn,
      displayOffIn,
      requirePassIn,
      runOnBattery: runOnBatteryInput?.checked ?? false,
      debug: debugInput?.checked ?? false,
      customOptions: current?.customOptions ?? {},
    };

    if (isTauri) {
      await invoke("set_options", { options: newOptions });
      options.set(newOptions);
      console.log("Options saved:", newOptions);
      if (!silent) alert("Settings saved successfully!");
    } else {
      options.set(newOptions);
      console.log("Options would be saved (mock):", newOptions);
      if (!silent) alert("Settings saved (demo mode — not persisted).");
    }
  } catch (error) {
    console.error("Failed to save options:", error);
    if (!silent) alert("Failed to save settings. Please try again.");
  }
}

async function resetOptions(): Promise<void> {
  try {
    if (isTauri) {
      options.set(await invoke<AppOptions>("factory_reset_options"));
      console.log("Options reset to defaults:", options.get());
    } else {
      options.set({
        saverUrl: "https://save.screensaver.gallery",
        saverUrlDebug: "https://save.screensaver.gallery/debug",
        optionsUrl: "",
        appName: "Liminal Screen",
        appDescription: "",
        startsIn: 0.2,
        displayOffIn: 1.0,
        requirePassIn: 1.0,
        runOnBattery: false,
        debug: false,
        customOptions: {},
      });
    }
    alert("Options reset to defaults");
  } catch (error) {
    console.error("Failed to reset options:", error);
    alert("Failed to reset options. Please try again.");
  }
}

async function previewScreensaver(): Promise<void> {
  try {
    if (isTauri) {
      await invoke("preview_screensaver");
    } else {
      alert("Preview would start! (demo mode)");
    }
  } catch (error) {
    console.error("Failed to preview screensaver:", error);
    alert("Failed to preview screensaver. Please try again.");
  }
}

document.addEventListener("DOMContentLoaded", () => {
  init();
});

if (typeof window !== "undefined") {
  (window as any).remoteOptions = { loadOptions, saveOptions, resetOptions, isTauri };
}
