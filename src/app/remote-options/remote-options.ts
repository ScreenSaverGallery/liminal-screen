// Remote Options Script
// Handles communication between remote options page and main Tauri app
// Gracefully degrades in non-Tauri environments

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// Check if we're in a Tauri environment
const isTauri = (() => {
  try {
    return (
      typeof window !== "undefined" &&
      (window as any)["__TAURI__"] !== undefined
    );
  } catch (e) {
    return false;
  }
})();

// UI Elements (cached after DOM load)
let startsInInput: HTMLInputElement | null = null;
let displayOffInput: HTMLInputElement | null = null;
let runOnBatteryInput: HTMLInputElement | null = null;
let debugInput: HTMLInputElement | null = null;
let saverUrlDisplay: HTMLElement | null = null;
let statusTextElement: HTMLElement | null = null;
let statusDotElement: HTMLElement | null = null;

/**
 * Initialize the remote options page
 */
async function init(): Promise<void> {
  console.log("Remote Options - Initializing...");

  try {
    // Cache UI elements
    cacheUIElements();

    // Set up event listeners
    setupEventListeners();

    // Only proceed with Tauri-specific functionality if in Tauri environment
    if (isTauri) {
      // Load current options
      await loadOptions();

      // Set up event listeners for IPC communication
      setupIPCListeners();

      console.log(
        "Remote Options - Initialized successfully with Tauri support",
      );
    } else {
      console.log("Remote Options - Initialized in non-Tauri environment");
      // Load mock options for demonstration
      loadMockOptions();
    }
  } catch (error) {
    console.error("Failed to initialize remote options:", error);
  }
}

/**
 * Cache UI elements for fast access
 */
function cacheUIElements(): void {
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
  statusTextElement = document.getElementById("status-text");
  statusDotElement = document.querySelector(".status-dot");
}

/**
 * Set up event listeners for UI buttons
 */
function setupEventListeners(): void {
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
        await resetOptions();
      }
    });
  }
}

/**
 * Set up IPC listeners for communication with main app
 */
function setupIPCListeners(): void {
  // Listen for options updates from main app
  listen("options-updated", async (event) => {
    console.log("Options updated from main app:", event.payload);
    await loadOptions();
  });
}

/**
 * Load options from main app
 */
async function loadOptions(): Promise<void> {
  try {
    // Get current options from main app
    const options = await invoke("get_options");
    console.log("Loaded options:", options);

    // Update form with current options
    loadOptionsIntoForm(options);

    // Update UI display
    updateOptionsDisplay(options);
  } catch (error) {
    console.error("Failed to load options:", error);
  }
}

/**
 * Load mock options for non-Tauri environments
 */
function loadMockOptions(): void {
  try {
    const mockOptions = {
      starts_in: 0.2,
      display_off_in: 1.0,
      run_on_battery: false,
      debug: false,
      saver_url: "https://save.screensaver.gallery",
      saver_url_debug: "https://save.screensaver.gallery/debug",
    };

    // Update form with mock options
    loadOptionsIntoForm(mockOptions);

    // Update UI display
    updateOptionsDisplay(mockOptions);

    console.log("Loaded mock options for demonstration");
  } catch (error) {
    console.error("Failed to load mock options:", error);
  }
}

/**
 * Load options into form inputs
 */
function loadOptionsIntoForm(options: any): void {
  if (startsInInput) {
    startsInInput.value = String(options.starts_in || 0.2);
  }

  if (displayOffInput) {
    displayOffInput.value = String(options.display_off_in || 1.0);
  }

  if (runOnBatteryInput) {
    runOnBatteryInput.checked = options.run_on_battery || false;
  }

  if (debugInput) {
    debugInput.checked = options.debug || false;
  }
}

/**
 * Update the options display in the UI
 */
function updateOptionsDisplay(options: any): void {
  // Display saver URL if available
  if (saverUrlDisplay) {
    const saverUrl = options.debug
      ? options.saver_url_debug
      : options.saver_url;
    saverUrlDisplay.textContent = saverUrl || "Not configured";
  }

  // Update status display
  updateStatusDisplay();
}

/**
 * Update the status indicator in the UI
 */
function updateStatusDisplay(): void {
  if (!statusTextElement || !statusDotElement) return;

  // TODO: Get actual screensaver status from main app
  const isActive = false; // Placeholder

  if (isActive) {
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
 * Save options to main app
 */
async function saveOptions(): Promise<void> {
  try {
    // Fetch current options first to preserve URLs and other backend-controlled fields
    const current = isTauri ? await invoke<any>("get_options") : null;
    
    // Get values from form
    const newOptions = {
      starts_in: startsInInput ? parseFloat(startsInInput.value) : 0.2,
      display_off_in: displayOffInput ? parseFloat(displayOffInput.value) : 1.0,
      run_on_battery: runOnBatteryInput ? runOnBatteryInput.checked : false,
      debug: debugInput ? debugInput.checked : false,
      // Preserve backend-controlled fields from current state
      saver_url: current?.saver_url || "",
      saver_url_debug: current?.saver_url_debug || "",
      options_url: current?.options_url || "",
      require_pass_in: current?.require_pass_in || 1.0,
    };

    // Validate
    if (isNaN(newOptions.starts_in) || newOptions.starts_in < 0.1) {
      alert("Start After must be at least 0.1 minutes");
      return;
    }

    if (isNaN(newOptions.display_off_in) || newOptions.display_off_in < 0.5) {
      alert("Display Off must be at least 0.5 minutes");
      return;
    }

    // Save to main app (only if in Tauri environment)
    if (isTauri) {
      await invoke("set_options", { options: newOptions });
      console.log("Options saved successfully");

      // Notify user
      alert("Settings saved successfully!");
    } else {
      console.log("Options would be saved (mock):", newOptions);
      alert(
        "Settings saved successfully! (This is a demo - in Tauri app, these would be saved)",
      );
    }
  } catch (error) {
    console.error("Failed to save options:", error);
    alert("Failed to save settings. Please try again.");
  }
}

/**
 * Reset options to defaults
 */
async function resetOptions(): Promise<void> {
  try {
    let defaultOptions: any;

    // Get factory reset options from main app (only if in Tauri environment)
    if (isTauri) {
      defaultOptions = await invoke("factory_reset_options");
      console.log("Options reset to defaults:", defaultOptions);
    } else {
      // Mock default options for non-Tauri environment
      defaultOptions = {
        starts_in: 0.2,
        display_off_in: 1.0,
        run_on_battery: false,
        debug: false,
        saver_url: "https://save.screensaver.gallery",
        saver_url_debug: "https://save.screensaver.gallery/debug",
        options_url: "",
        require_pass_in: 1.0,
      };
      console.log("Options reset to mock defaults:", defaultOptions);
    }

    // Update form with default options
    loadOptionsIntoForm(defaultOptions);

    // Note: factory_reset_options() already persists to disk, no need to call saveOptions()

    alert("Options reset to defaults");
  } catch (error) {
    console.error("Failed to reset options:", error);
    alert("Failed to reset options. Please try again.");
  }
}

/**
 * Preview the screensaver
 */
async function previewScreensaver(): Promise<void> {
  try {
    // Preview screensaver (only if in Tauri environment)
    if (isTauri) {
      await invoke("preview_screensaver");
      console.log("Preview screensaver requested");
    } else {
      console.log("Preview screensaver requested (mock)");
      alert(
        "Preview would start! (This is a demo - in Tauri app, the screensaver would preview)",
      );
    }
  } catch (error) {
    console.error("Failed to preview screensaver:", error);
    alert("Failed to preview screensaver. Please try again.");
  }
}

// Initialize when DOM is ready
document.addEventListener("DOMContentLoaded", () => {
  init();
});

// Export for global access if needed
if (typeof window !== "undefined") {
  (window as any).remoteOptions = {
    loadOptions,
    saveOptions,
    resetOptions,
    isTauri,
  };
}
