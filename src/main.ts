// Liminal Screen - Main Application Entry Point
// Handles initialization, monitoring loop, and application flow

import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

import { PowerMonitor, MonitorInfo } from "./app/power-monitor/power-monitor";
import { Saver, SaverOptions } from "./app/saver/saver";
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

// State variables
let isScreensaverActive = false;
let activeSavers: Saver[] = [];

let previewWindow: WebviewWindow | null = null;
let options: MandatoryOptions | null = null;
let remoteOptions: RemoteOptions = {};
let monitoringInterval: number | null = null;
let displayOffTimeout: number | null = null;

// Constants
const MONITORING_INTERVAL_MS = 1000; // Check every second

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
  console.log("Liminal Screen - Initializing...");

  try {
    // Initialize storage first
    await Storage.init();

    // Initialize options manager
    await OptionsManager.init();

    // Load options
    await loadOptions();
    loadOptionsIntoForm();

    // Send current URLs to Rust backend
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

    // Register service worker for offline support
    await registerServiceWorker();

    // Set up event listeners
    setupEventListeners();

    // Start monitoring loop
    startMonitoring();

    console.log("Liminal Screen - Initialized successfully");
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
    remoteOptions = storedOptions.remoteOptions || {};

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
    remoteOptions = event.payload;
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
  getCurrentWindow().listen("tauri://close-requested", () => {
    // Hide window instead of closing when running in tray
    getCurrentWindow().hide();
  });

  // Listen for saver activity detected (user input in screensaver)
  listen("saver-activity-detected", async () => {
    console.log("Saver activity detected, deactivating screensaver");
    await deactivateScreensaver();
  });
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

  // Save to storage
  try {
    await Storage.setMandatoryOptions({
      startsIn: options.startsIn,
      displayOffIn: options.displayOffIn,
      requirePassIn: options.requirePassIn,
      runOnBattery: options.runOnBattery,
      debug: options.debug,
    });
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
 * Start the idle monitoring loop
 */
function startMonitoring(): void {
  if (monitoringInterval) {
    clearInterval(monitoringInterval);
  }

  monitoringInterval = window.setInterval(async () => {
    await checkIdleState();
  }, MONITORING_INTERVAL_MS);

  console.log("Monitoring started");
}

/**
 * Stop the idle monitoring loop
 */
function stopMonitoring(): void {
  if (monitoringInterval) {
    clearInterval(monitoringInterval);
    monitoringInterval = null;
  }
  console.log("Monitoring stopped");
}

/**
 * Check the current idle state and manage screensaver
 */
async function checkIdleState(): Promise<void> {
  if (!options) {
    console.warn("Options not loaded yet");
    return;
  }

  try {
    // Get idle time in seconds
    const idleTime = await PowerMonitor.getSystemIdleTime();
    const startsInSeconds = options.startsIn * 60;
    const displayOffInSeconds = options.displayOffIn * 60;

    // Update UI with idle time
    updateIdleTimeDisplay(idleTime);
    updateStatusDisplay();

    // Debug: Log idle time and threshold
    console.log(
      `Idle: ${idleTime}s, Threshold: ${startsInSeconds}s, Active: ${isScreensaverActive}`,
    );

    // Check battery status if needed
    if (!options.runOnBattery) {
      const onBattery = await PowerMonitor.isOnBatteryPower();
      if (onBattery) {
        // Don't run screensaver on battery
        if (isScreensaverActive) {
          await deactivateScreensaver();
        }
        return;
      }
    }

    // Handle screensaver activation
    if (idleTime >= startsInSeconds && !isScreensaverActive) {
      await activateScreensaver();
    }
    // Handle screensaver deactivation
    else if (idleTime < startsInSeconds && isScreensaverActive) {
      await deactivateScreensaver();
    }
    // Handle display blank
    else if (
      idleTime >= displayOffInSeconds &&
      isScreensaverActive &&
      !displayOffTimeout
    ) {
      // Schedule display blank
      displayOffTimeout = window.setTimeout(async () => {
        await PowerMonitor.blankScreen();
      }, 0);
    }
  } catch (error) {
    console.error("Error checking idle state:", error);
  }
}

/**
 * Activate the screensaver on all displays
 */
async function activateScreensaver(): Promise<void> {
  if (isScreensaverActive) return;

  console.log("Activating screensaver...");

  try {
    // Prevent display sleep
    await PowerMonitor.preventDisplaySleep();

    // Get all monitors - command registered directly without namespace
    const monitors = await invoke<MonitorInfo[]>("get_available_monitors");

    if (monitors.length === 0) {
      console.warn("No monitors found");
      return;
    }

    // Create saver for each display
    const saverOptions: SaverOptions = {
      debug: options?.debug || false,
      ...remoteOptions,
    };

    // Build URL with query parameters
    const baseUrl = options?.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG
      : import.meta.env.VITE_SAVER_URL;
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(remoteOptions)) {
      if (value !== undefined && value !== null) {
        params.append(key, String(value));
      }
    }
    const url = params.toString()
      ? `${baseUrl}?${params.toString()}`
      : baseUrl || "about:blank";

    for (const monitor of monitors) {
      const saver = new Saver(
        url,
        `saver-display-${monitor.id}`,
        { x: monitor.position.x, y: monitor.position.y },
        {
          width: monitor.size.width,
          height: monitor.size.height,
        },
        saverOptions,
      );

      await saver.show();
      activeSavers.push(saver);

      // Register with Rust side
      await invoke("add_active_saver", {
        label: `saver-display-${monitor.id}`,
      });
    }

    isScreensaverActive = true;
    console.log(`Screensaver activated on ${monitors.length} display(s)`);

    // Emit event
    await emit("screensaver-started");
  } catch (error) {
    console.error("Failed to activate screensaver:", error);
    await deactivateScreensaver();
  }
}

/**
 * Deactivate the screensaver
 */
async function deactivateScreensaver(): Promise<void> {
  if (!isScreensaverActive) return;

  console.log("Deactivating screensaver...");

  try {
    // Clear display off timeout
    if (displayOffTimeout) {
      clearTimeout(displayOffTimeout);
      displayOffTimeout = null;
    }

    // Emit screensaver ending event
    await emit("screensaver-ending");

    // Close all saver windows using the activeSavers array
    // This is more reliable than using WebviewWindow.getAll()
    const closePromises = activeSavers.map((saver) => saver.hide());
    await Promise.allSettled(closePromises);

    // Clear the active savers array
    activeSavers = [];

    // Clear active savers in Rust
    await invoke("clear_active_savers");

    // Allow display sleep
    await PowerMonitor.allowDisplaySleep();

    isScreensaverActive = false;

    console.log("Screensaver deactivated");

    // Emit event
    await emit("screensaver-ended");
  } catch (error) {
    console.error("Failed to deactivate screensaver:", error);
  }
}

/**
 * Preview the screensaver (immediate activation)
 */
async function previewScreensaver(): Promise<void> {
  if (isScreensaverActive) {
    console.warn("Screensaver already active");
    return;
  }

  // Close existing preview window if open
  if (previewWindow) {
    try {
      await previewWindow.close();
    } catch (error) {
      console.warn("Could not close existing preview window:", error);
    }
    previewWindow = null;
  }

  try {
    // Create a simple preview window like the main window
    previewWindow = new WebviewWindow("preview", {
      url: options?.debug
        ? import.meta.env.VITE_SAVER_URL_DEBUG
        : import.meta.env.VITE_SAVER_URL,
      title: "Screensaver Preview",
      width: 800,
      height: 600,
      resizable: true,
      decorations: true,
      visible: true,
      alwaysOnTop: false,
      skipTaskbar: false,
    });

    console.log("Preview window created");
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
 * Force screensaver deactivation (public API)
 */
export async function forceDeactivateScreensaver(): Promise<void> {
  await deactivateScreensaver();
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

// Cleanup on page unload
window.addEventListener("beforeunload", () => {
  stopMonitoring();
});

// Export for global access
(
  window as unknown as { liminalScreen: Record<string, unknown> }
).liminalScreen = {
  deactivateScreensaver: forceDeactivateScreensaver,
  isScreensaverRunning,
  getCurrentOptions,
  openLink,
};
