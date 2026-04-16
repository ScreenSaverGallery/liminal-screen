// Liminal Screen - Main Application Entry Point
// Handles initialization, monitoring loop, and application flow

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { PowerMonitor } from "./app/power-monitor/power-monitor";
import { Preview } from "./app/preview/preview";
import {
  Storage,
  MandatoryOptions,
  RemoteOptions,
} from "./app/storage/storage";
import {
  OptionsManager,
  registerServiceWorker,
  openExternalLink,
} from "./app/options/options";

// State variables - now mainly for UI state synchronization
let isScreensaverActive = false;
// let activeSavers: string[] = []; // Not used in autonomous engine - managed by Rust

let previewWindow: Preview | null = null;
let options: MandatoryOptions | null = null;
// let remoteOptions: RemoteOptions = {}; // Managed by Rust engine
let displayOffTimeout: number | null = null;

// Constants
const MONITORING_INTERVAL_MS = 1000; // Check every second for UI updates only

// UI Elements (cached after DOM load)
let idleTimeElement: HTMLElement | null = null;
let statusTextElement: HTMLElement | null = null;
let statusDotElement: HTMLElement | null = null;
let startsInInput: HTMLInputElement | null = null;
let displayOffInput: HTMLInputElement | null = null;
let runOnBatteryInput: HTMLInputElement | null = null;
let debugInput: HTMLInputElement | null = null;
let saverUrlDisplay: HTMLElement | null = null;

/**
 * Initialize the application
 */
async function init(): Promise<void> {
  console.log("Liminal Screen - Initializing UI...");

  try {
    // Initialize storage first
    await Storage.init();

    // Initialize options manager
    await OptionsManager.init();

    // Load options
    await loadOptions();
    loadOptionsIntoForm();

    // Register service worker for offline support
    await registerServiceWorker();

    // Set up event listeners
    setupEventListeners();

    console.log("Liminal Screen - UI Initialized successfully");
    console.log("Current options:", options);
  } catch (error) {
    console.error("Failed to initialize application:", error);
  }
}

/**
 * Load options from storage
 */
async function loadOptions(): Promise<void> {
  try {
    const storedOptions = await Storage.getOptions();
    options = {
      startsIn: storedOptions.startsIn,
      displayOffIn: storedOptions.displayOffIn,
      requirePassIn: storedOptions.requirePassIn,
      runOnBattery: storedOptions.runOnBattery,
      debug: storedOptions.debug,
    };
    // remoteOptions = storedOptions.remoteOptions || {}; // Managed by Rust engine

    console.log("Options loaded:", options);
  } catch (error) {
    console.error("Failed to load options:", error);
    // Use defaults
    options = {
      startsIn: 0.2,
      displayOffIn: 1,
      requirePassIn: 1,
      runOnBattery: false,
      debug: false,
    };
  }
}

/**
 * Set up event listeners for IPC communication
 */
function setupEventListeners(): void {
  // Listen for options changes
  listen<RemoteOptions>("options-updated", (event) => {
    console.log("Options updated:", event.payload);
    // remoteOptions = event.payload; // Commented out as it's unused
    updateOptionsDisplay();
  });

  // Listen for preview request
  listen("preview-screensaver", () => {
    console.log("Preview screensaver requested");
    previewScreensaver();
  });

  // Listen for open-options event
  listen("open-options-window", () => {
    console.log("Open options window requested");
    openOptionsWindow();
  });

  // Listen for reset-options event
  listen("reset-options", () => {
    console.log("Reset options requested");
    loadOptions(); // Reload from storage
  });

  // Listen for window close events
  getCurrentWindow().onCloseRequested((event: any) => {
    event.preventDefault();
    // Hide window instead of closing when running in tray
    getCurrentWindow().hide();
  });

  // Listen for screensaver started/ended events from Rust engine
  listen("screensaver-started", async () => {
    console.log("Screensaver started (from Rust engine)");
    isScreensaverActive = true;
    updateStatusDisplay();
  });

  listen("screensaver-ended", async () => {
    console.log("Screensaver ended (from Rust engine)");
    isScreensaverActive = false;
    updateStatusDisplay();
    
    // Clear display off timeout
    if (displayOffTimeout) {
      clearTimeout(displayOffTimeout);
      displayOffTimeout = null;
    }
  });

  // Periodically sync state with Rust engine (for UI display only)
  setInterval(async () => {
    try {
      const status = await invoke<any>("get_screensaver_status");
      isScreensaverActive = status.is_active;
      updateStatusDisplay();
      
      // Get idle time for display purposes only
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

/**
 * Set up UI button handlers
 */
function setupUIButtonHandlers(): void {
  // Save button
  const saveBtn = document.getElementById("save-btn");
  if (saveBtn) {
    saveBtn.addEventListener("click", async () => {
      console.log("Save button clicked");
      await saveOptions();
    });
  }

  // Preview button
  const previewBtn = document.getElementById("preview-btn");
  if (previewBtn) {
    previewBtn.addEventListener("click", async () => {
      console.log("Preview button clicked");
      await previewScreensaver();
    });
  }

  // Reset button
  const resetBtn = document.getElementById("reset-btn");
  if (resetBtn) {
    resetBtn.addEventListener("click", async () => {
      console.log("Reset button clicked");
      if (confirm("Reset all options to defaults?")) {
        await Storage.factoryReset();
        await loadOptions();
        loadOptionsIntoForm();
        alert("Options reset to defaults");
      }
    });
  }
}

/**
 * Update the idle time display in the UI
 */
function updateIdleTimeDisplay(idleSeconds: number): void {
  if (!idleTimeElement) return;

  // Format idle time nicely
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

/**
 * Update the status indicator in the UI
 */
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

/**
 * Load options into form inputs
 */
function loadOptionsIntoForm(): void {
  if (!options) return;

  if (startsInInput) {
    startsInInput.value = String(options.startsIn);
  }

  if (displayOffInput) {
    displayOffInput.value = String(options.displayOffIn);
  }

  if (runOnBatteryInput) {
    runOnBatteryInput.checked = options.runOnBattery;
  }

  if (debugInput) {
    debugInput.checked = options.debug;
  }

  // Display saver URL if available
  if (saverUrlDisplay) {
    const saverUrl = options.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG
      : import.meta.env.VITE_SAVER_URL;
    saverUrlDisplay.textContent = saverUrl || "Not configured";
  }
}

/**
 * Save options from form inputs to storage
 */
async function saveOptions(): Promise<void> {
  if (!options) return;

  // Get values from form
  const newStartsIn = startsInInput
    ? parseFloat(startsInInput.value)
    : options.startsIn;
  const newDisplayOffIn = displayOffInput
    ? parseFloat(displayOffInput.value)
    : options.displayOffIn;
  const newRunOnBattery = runOnBatteryInput
    ? runOnBatteryInput.checked
    : options.runOnBattery;
  const newDebug = debugInput ? debugInput.checked : options.debug;

  // Validate
  if (isNaN(newStartsIn) || newStartsIn < 0.1) {
    alert("Start After must be at least 0.1 minutes");
    return;
  }

  if (isNaN(newDisplayOffIn) || newDisplayOffIn < 0.5) {
    alert("Display Off must be at least 0.5 minutes");
    return;
  }

  // Update options
  options.startsIn = newStartsIn;
  options.displayOffIn = newDisplayOffIn;
  options.runOnBattery = newRunOnBattery;
  options.debug = newDebug;

  // Save to storage and update Rust backend
  try {
    await Storage.setMandatoryOptions({
      startsIn: options.startsIn,
      displayOffIn: options.displayOffIn,
      requirePassIn: options.requirePassIn,
      runOnBattery: options.runOnBattery,
      debug: options.debug,
    });
    
    // Send updated options to Rust backend
    if (options) {
      const appOptions = {
        saver_url:
          import.meta.env.VITE_SAVER_URL || "https://save.screensaver.gallery",
        saver_url_debug:
          import.meta.env.VITE_SAVER_URL_DEBUG ||
          "https://save.screensaver.gallery/debug",
        options_url: import.meta.env.VITE_OPTIONS_URL || "",
        starts_in: options.startsIn,
        display_off_in: options.displayOffIn,
        require_pass_in: options.requirePassIn || 1.0,
        run_on_battery: options.runOnBattery,
        debug: options.debug,
      };

      try {
        await invoke("set_options", { options: appOptions });
        console.log("Sent app options to Rust backend:", appOptions);
      } catch (error) {
        console.error("Failed to send options to Rust backend:", error);
      }
    }
    
    console.log("Options saved successfully");
    alert("Settings saved successfully!");
  } catch (error) {
    console.error("Failed to save options:", error);
    alert("Failed to save settings. Please try again.");
  }
}

/**
 * Update the options display in the UI (deprecated - use loadOptionsIntoForm)
 */
function updateOptionsDisplay(): void {
  loadOptionsIntoForm();
}

/**
 * Cache UI elements for fast access
 */
function cacheUIElements(): void {
  idleTimeElement = document.getElementById("idle-time");
  statusTextElement = document.getElementById("status-text");
  statusDotElement = document.querySelector(".status-dot");
  startsInInput = document.getElementById(
    "starts-in",
  ) as HTMLInputElement | null;
  displayOffInput = document.getElementById(
    "display-off",
  ) as HTMLInputElement | null;
  runOnBatteryInput = document.getElementById(
    "run-on-battery",
  ) as HTMLInputElement | null;
  debugInput = document.getElementById("debug-mode") as HTMLInputElement | null;
  saverUrlDisplay = document.getElementById("saver-url-display");
}

/**
 * Preview the screensaver (immediate activation)
 */
async function previewScreensaver(): Promise<void> {
  // Close existing preview window if open
  if (previewWindow) {
    await previewWindow.hide();
  }

  try {
    // Determine which URL to use based on debug mode
    const previewUrl = options?.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG ||
        "https://save.screensaver.gallery/debug"
      : import.meta.env.VITE_SAVER_URL || "https://save.screensaver.gallery";

    // Create new preview instance (label will be auto-generated)
    previewWindow = new Preview(previewUrl);
    
    // Show the preview window
    await previewWindow.show();

    console.log(
      "Preview window created. Use window.closePreviewWindow() to close it manually if needed.",
    );
  } catch (error) {
    console.error("Failed to create preview window:", error);
  }
}

/**
 * Open the options window
 */
async function openOptionsWindow(): Promise<void> {
  try {
    await invoke("open_options");
  } catch (error) {
    console.error("Failed to open options window:", error);
  }
}

/**
 * Force screensaver deactivation (public API) - calls Rust engine
 */
export async function forceDeactivateScreensaver(): Promise<void> {
  try {
    // Tell Rust engine to deactivate
    await invoke("deactivate_screensaver_command");
  } catch (error) {
    console.error("Failed to force deactivate screensaver:", error);
  }
}

/**
 * Check if screensaver is currently active
 */
export function isScreensaverRunning(): boolean {
  return isScreensaverActive;
}

/**
 * Get current options
 */
export function getCurrentOptions(): MandatoryOptions | null {
  return options;
}

/**
 * Open external link in system browser
 */
export async function openLink(url: string): Promise<void> {
  await openExternalLink(url);
}

// Initialize application when DOM is ready
window.addEventListener("DOMContentLoaded", () => {
  // Cache UI elements
  cacheUIElements();

  // Set up UI button handlers
  setupUIButtonHandlers();

  // Initialize application
  init();
});

// Also try to initialize immediately in case DOMContentLoaded doesn't fire for hidden windows
// This ensures the JavaScript runs even when window is hidden
try {
  // This should run even for hidden windows
  console.log("Liminal Screen immediate initialization attempt");
  // Don't await this as it might not resolve immediately
  init().catch((error) => {
    console.error("Immediate init failed:", error);
  });
} catch (error) {
  console.error("Immediate init threw error:", error);
}

// Export for global access
(
  window as unknown as { liminalScreen: Record<string, unknown> }
).liminalScreen = {
  deactivateScreensaver: forceDeactivateScreensaver,
  isScreensaverRunning,
  getCurrentOptions,
  openLink,
};