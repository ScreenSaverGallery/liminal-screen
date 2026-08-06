// Liminal Screen — Options Window

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ask, message } from "@tauri-apps/plugin-dialog";

import { formatIdle } from "./app/format";
import { PowerMonitor } from "./app/power-monitor/power-monitor";
import { Preview } from "./app/preview/preview";
import { Signal } from "./app/reactive";
import type { AppOptions } from "./app/types";

// ── State ──────────────────────────────────────────────────────────────────

const options = new Signal<AppOptions | null>(null);

interface ScreensaverStatus {
  active: boolean;
  idleSeconds: number;
  previewActive: boolean;
}
const status = new Signal<ScreensaverStatus>({
  active: false,
  idleSeconds: 0,
  previewActive: false,
});

const isActive = status.derive((s) => s.active || s.previewActive);
const idleSignal = status.derive((s) => s.idleSeconds);

interface UpdateInfo {
  version: string;
  notes?: string;
}
const updateAvailable = new Signal<UpdateInfo | null>(null);
const updateChecking = new Signal<boolean>(false);

let previewWindow: Preview = new Preview();

// ── UI Elements ────────────────────────────────────────────────────────────

let idleTimeElement: HTMLElement | null = null;
let statusTextElement: HTMLElement | null = null;
let statusDotElement: HTMLElement | null = null;
let startsInInput: HTMLInputElement | null = null;
let displayOffInput: HTMLInputElement | null = null;
let requirePassInInput: HTMLInputElement | null = null;
let runOnBatteryInput: HTMLInputElement | null = null;
let autostartInput: HTMLInputElement | null = null;
let debugInput: HTMLInputElement | null = null;
let notificationsEnabledInput: HTMLInputElement | null = null;
let notificationsItem: HTMLElement | null = null;
let saverUrlDisplay: HTMLElement | null = null;
let conflictWarningElement: HTMLElement | null = null;
let conflictWarningTextElement: HTMLElement | null = null;
let conflictWarningIconElement: HTMLElement | null = null;
let disableScreensaverBtn: HTMLButtonElement | null = null;
let restoreScreensaverBtn: HTMLButtonElement | null = null;

// ── Helpers ────────────────────────────────────────────────────────────────

async function openExternalLink(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch {
    window.open(url, "_blank");
  }
}

// ── Init ───────────────────────────────────────────────────────────────────

let initialized = false;

async function init(): Promise<void> {
  if (initialized) return;
  initialized = true;
  console.log("Initializing...");
  try {
    options.set(await invoke<AppOptions>("get_options"));
    setupEventListeners();
    const name = options.get()?.appName ?? "Liminal Screen";
    console.log(`${name} - Ready`, options.get());
  } catch (error) {
    console.error("Failed to initialize:", error);
  }
}

function setupEventListeners(): void {
  // Options sync
  listen<AppOptions>("options-updated", (event) => options.set(event.payload));
  listen("reset-options", async () => {
    try {
      options.set(await invoke<AppOptions>("get_options"));
    } catch {
      /* ignore */
    }
  });

  // Window management
  listen("preview-screensaver", () => previewScreensaver());
  listen("open-options-window", async () => {
    try {
      await invoke("open_options");
    } catch {
      /* ignore */
    }
  });
  listen("preview-closed", () => {
    status.update((s) => ({ ...s, previewActive: false }));
  });
  listen<string>("webview-closed", (event) => {
    if (previewWindow.getLabel() === event.payload) {
      status.update((s) => ({ ...s, previewActive: false }));
    }
  });
  getCurrentWindow().onCloseRequested((event: any) => {
    event.preventDefault();
    getCurrentWindow().hide();
  });

  // Screensaver state — driven by Tauri events, not polling
  listen("screensaver-started", () =>
    status.update((s) => ({ ...s, active: true })),
  );
  listen("screensaver-ended", () =>
    status.update((s) => ({ ...s, active: false })),
  );

  // Updates
  listen<UpdateInfo>("update-available", (event) => {
    updateChecking.set(false);
    updateAvailable.set(event.payload);
  });
  listen("update-not-available", () => {
    updateChecking.set(false);
    updateAvailable.set(null);
  });
  listen("update-installed", () => updateAvailable.set(null));

  // Idle time — poll every second (no Rust event available for this yet)
  setInterval(async () => {
    try {
      const secs = await PowerMonitor.getSystemIdleTime();
      status.update((s) => ({ ...s, idleSeconds: secs }));
    } catch {
      /* ignore */
    }
  }, 1000);
}

// ── Identity ──────────────────────────────────────────────────────────────

function setIdentity(opts: AppOptions): void {
  const nameEl = document.getElementById("app-name");
  const descEl = document.getElementById("app-description");
  const aboutEl = document.getElementById("about-text");
  const titleEl = document.getElementById("app-title");

  if (nameEl) nameEl.textContent = opts.appName;
  if (descEl) descEl.textContent = opts.appDescription;
  if (titleEl) titleEl.textContent = `${opts.appName} - Options`;
  if (aboutEl)
    aboutEl.textContent = `${opts.appName} runs in your system tray and activates after a period of inactivity. ${opts.appDescription}`;
}

// ── Form ───────────────────────────────────────────────────────────────────

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
  requirePassInInput = document.getElementById(
    "require-pass-in",
  ) as HTMLInputElement | null;
  runOnBatteryInput = document.getElementById(
    "run-on-battery",
  ) as HTMLInputElement | null;
  autostartInput = document.getElementById(
    "autostart",
  ) as HTMLInputElement | null;
  debugInput = document.getElementById("debug-mode") as HTMLInputElement | null;
  notificationsEnabledInput = document.getElementById(
    "notifications-enabled",
  ) as HTMLInputElement | null;
  notificationsItem = document.getElementById("notifications-item");
  saverUrlDisplay = document.getElementById("saver-url-display");
  conflictWarningElement = document.getElementById(
    "screensaver-conflict-warning",
  );
  conflictWarningTextElement = document.getElementById(
    "screensaver-conflict-text",
  );
  conflictWarningIconElement = conflictWarningElement?.querySelector(
    ".conflict-warning-icon",
  ) as HTMLElement | null;
  disableScreensaverBtn = document.getElementById(
    "disable-screensaver-btn",
  ) as HTMLButtonElement | null;
  restoreScreensaverBtn = document.getElementById(
    "restore-screensaver-btn",
  ) as HTMLButtonElement | null;

  [
    startsInInput,
    displayOffInput,
    requirePassInInput,
    runOnBatteryInput,
    autostartInput,
    debugInput,
    notificationsEnabledInput,
  ].forEach((el) => el?.addEventListener("change", () => saveOptions(true)));
}

function setupUIButtonHandlers(): void {
  document
    .getElementById("save-btn")
    ?.addEventListener("click", () => saveOptions());
  document
    .getElementById("preview-btn")
    ?.addEventListener("click", () => previewScreensaver());

  document
    .getElementById("disable-screensaver-btn")
    ?.addEventListener("click", async () => {
      try {
        await PowerMonitor.disableOsScreensaver();
      } catch (error) {
        console.error("Failed to disable system screensaver:", error);
        await message(`Could not disable the system screensaver: ${error}`, {
          title: "System Screensaver",
          kind: "error",
        });
      }
      await checkScreensaverConflict();
    });

  document
    .getElementById("restore-screensaver-btn")
    ?.addEventListener("click", async () => {
      try {
        await PowerMonitor.restoreOsScreensaver();
      } catch (error) {
        console.error("Failed to restore system screensaver:", error);
        await message(`Could not restore the system screensaver: ${error}`, {
          title: "System Screensaver",
          kind: "error",
        });
      }
      await checkScreensaverConflict();
    });
  document.getElementById("reset-btn")?.addEventListener("click", async () => {
    const confirmed = await ask("Reset all options to defaults?", {
      title: "Reset",
      kind: "warning",
      okLabel: "Reset",
      cancelLabel: "Cancel",
    });
    if (!confirmed) return;
    try {
      await invoke("factory_reset_options");
      options.set(await invoke<AppOptions>("get_options"));
      // Form updates reactively via options.effect() — no dialog needed
    } catch (error) {
      console.error("Failed to reset options:", error);
      await message("Failed to reset options. Please try again.", {
        title: "Error",
        kind: "error",
      });
    }
  });

  document
    .getElementById("check-updates-btn")
    ?.addEventListener("click", async () => {
      updateChecking.set(true);
      try {
        await invoke("check_for_updates");
      } catch (error) {
        updateChecking.set(false);
        console.error("Update check failed:", error);
        await message(`Update check failed: ${error}`, {
          title: "Updates",
          kind: "error",
        });
      }
    });
  document
    .getElementById("install-update-btn")
    ?.addEventListener("click", async () => {
      try {
        await invoke("install_update");
      } catch (error) {
        console.error("Update install failed:", error);
        await message(`Update install failed: ${error}`, {
          title: "Updates",
          kind: "error",
        });
      }
    });

  document.querySelectorAll(".external-link").forEach((el: Element) => {
    el?.addEventListener("click", () => {
      const link = el.getAttribute("data");
      if (link) openExternalLink(link);
    });
  });
}

async function saveOptions(silent = false): Promise<void> {
  const current = options.get();
  if (!current) return;

  const startsIn = startsInInput
    ? parseFloat(startsInInput.value)
    : current.startsIn;
  const displayOffIn = displayOffInput
    ? parseFloat(displayOffInput.value)
    : current.displayOffIn;
  const requirePassIn = requirePassInInput
    ? parseFloat(requirePassInInput.value)
    : current.requirePassIn;
  const runOnBattery = runOnBatteryInput
    ? runOnBatteryInput.checked
    : current.runOnBattery;
  const autostart = autostartInput
    ? autostartInput.checked
    : current.autostart;
  const debug = debugInput ? debugInput.checked : current.debug;
  const notificationsEnabled = notificationsEnabledInput
    ? notificationsEnabledInput.checked
    : current.notificationsEnabled;

  if (isNaN(startsIn) || startsIn < 0.1) {
    if (!silent)
      await message("Start After must be at least 0.1 minutes", {
        title: "Validation Error",
        kind: "error",
      });
    return;
  }
  if (isNaN(displayOffIn) || displayOffIn < 0.5) {
    if (!silent)
      await message("Display Off must be at least 0.5 minutes", {
        title: "Validation Error",
        kind: "error",
      });
    return;
  }
  if (isNaN(requirePassIn) || requirePassIn < 0) {
    if (!silent)
      await message("Require Password must be 0 or a positive number", {
        title: "Validation Error",
        kind: "error",
      });
    return;
  }

  try {
    await invoke("set_options", {
      options: {
        ...current,
        startsIn,
        displayOffIn,
        requirePassIn,
        runOnBattery,
        autostart,
        debug,
        notificationsEnabled,
      },
    });
    options.set(await invoke<AppOptions>("get_options"));
    if (!silent)
      await message("Settings saved successfully!", {
        title: "Settings",
        kind: "info",
      });
  } catch {
    if (!silent)
      await message("Failed to save settings. Please try again.", {
        title: "Error",
        kind: "error",
      });
  }
}

// ── OS screensaver conflict ──────────────────────────────────────────────────

/** Human-readable duration for the warning text ("90s" → "1m 30s"). */
function formatSeconds(secs: number): string {
  if (secs < 60) return `${secs}s`;
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return s ? `${m}m ${s}s` : `${m}m`;
}

/**
 * Liminal is meant to be the only screensaver: a system screensaver on an
 * overlapping timer draws over Liminal's windows. Three states:
 *   1. OS screensaver enabled  → amber warning + [Disable] (severe if it wins the race)
 *   2. Liminal disabled it      → info note + [Restore]
 *   3. neither                  → banner hidden
 */
async function checkScreensaverConflict(): Promise<void> {
  if (!conflictWarningElement || !conflictWarningTextElement) return;

  const [os, savedIdle] = await Promise.all([
    PowerMonitor.getOsScreensaverStatus(),
    PowerMonitor.getSavedOsScreensaverIdle(),
  ]);

  // State 1: a conflicting OS screensaver is enabled.
  if (os.detected && os.enabled) {
    const startsInSecs = (options.get()?.startsIn ?? 0) * 60;
    const firesFirst = os.idleSeconds != null && os.idleSeconds <= startsInSecs;
    const timing =
      os.idleSeconds != null
        ? ` (starts after ${formatSeconds(os.idleSeconds)})`
        : "";
    const lead = firesFirst
      ? `Your system screensaver${timing} starts before Liminal and will appear on top of it.`
      : `Your system screensaver is enabled${timing} and may appear on top of Liminal.`;

    conflictWarningTextElement.textContent =
      `${lead} Liminal works best as your only screensaver.`;
    conflictWarningElement.classList.remove("is-info");
    if (conflictWarningIconElement) conflictWarningIconElement.textContent = "⚠";
    if (disableScreensaverBtn) disableScreensaverBtn.hidden = false;
    if (restoreScreensaverBtn) restoreScreensaverBtn.hidden = true;
    conflictWarningElement.hidden = false;
    return;
  }

  // State 2: Liminal disabled the OS screensaver — offer to restore it.
  if (savedIdle != null && savedIdle > 0) {
    conflictWarningTextElement.textContent =
      `Liminal disabled your system screensaver (was ${formatSeconds(savedIdle)}) ` +
      `so it won't appear over Liminal.`;
    conflictWarningElement.classList.add("is-info");
    if (conflictWarningIconElement) conflictWarningIconElement.textContent = "ℹ";
    if (disableScreensaverBtn) disableScreensaverBtn.hidden = true;
    if (restoreScreensaverBtn) restoreScreensaverBtn.hidden = false;
    conflictWarningElement.hidden = false;
    return;
  }

  // State 3: nothing to show.
  conflictWarningElement.hidden = true;
}

// ── Preview ────────────────────────────────────────────────────────────────

async function previewScreensaver(): Promise<void> {
  try {
    const opts = options.get();
    const url = opts?.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG ||
        "https://saver.example.com/debug"
      : import.meta.env.VITE_SAVER_URL || "https://saver.example.com";
    await previewWindow.show(url);
    status.update((s) => ({ ...s, previewActive: true }));
  } catch (error) {
    console.error("Failed to create preview window:", error);
    status.update((s) => ({ ...s, previewActive: false }));
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

  // Show the real app version in the footer (compiled from Cargo.toml)
  invoke<string>("get_app_version")
    .then((v) => {
      const el = document.getElementById("app-version");
      if (el) el.textContent = `v${v}`;
    })
    .catch(() => {
      /* leave the fallback text */
    });

  // Reactive effects — each fires immediately then whenever the signal changes

  options.effect((opts) => {
    if (!opts) return;
    if (startsInInput) startsInInput.value = String(opts.startsIn);
    if (displayOffInput) displayOffInput.value = String(opts.displayOffIn);
    if (requirePassInInput)
      requirePassInInput.value = String(opts.requirePassIn);
    if (runOnBatteryInput) runOnBatteryInput.checked = opts.runOnBattery;
    if (autostartInput) autostartInput.checked = opts.autostart;
    if (debugInput) debugInput.checked = opts.debug;
    if (notificationsEnabledInput)
      notificationsEnabledInput.checked = opts.notificationsEnabled;
    // Consent toggle only makes sense when the fork ships a notification feed
    if (notificationsItem) notificationsItem.hidden = !opts.notificationUrl;
    if (saverUrlDisplay) {
      saverUrlDisplay.textContent =
        (opts.debug ? opts.saverUrlDebug : opts.saverUrl) || "Not configured";
    }

    // Update app identity (title, h1, subtitle, about)
    setIdentity(opts);

    // Re-evaluate OS screensaver conflict (severity depends on startsIn)
    void checkScreensaverConflict();
  });

  // Re-check when the window regains focus — the user may have changed the OS
  // screensaver setting in System Settings while the options window was open.
  window.addEventListener("focus", () => void checkScreensaverConflict());

  isActive.effect((active) => {
    if (!statusDotElement || !statusTextElement) return;
    statusTextElement.textContent = active ? "Active" : "Inactive";
    statusDotElement.classList.toggle("active", active);
    statusDotElement.classList.toggle("inactive", !active);
  });

  idleSignal.effect((secs) => {
    if (idleTimeElement) idleTimeElement.textContent = formatIdle(secs);
  });

  const renderUpdateRow = () => {
    const statusEl = document.getElementById("update-status-text");
    const installBtn = document.getElementById(
      "install-update-btn",
    ) as HTMLButtonElement | null;
    const info = updateAvailable.get();
    if (statusEl) {
      statusEl.textContent = updateChecking.get()
        ? "Checking…"
        : info
          ? `v${info.version} available`
          : "Up to date";
    }
    if (installBtn) installBtn.hidden = !info;
  };
  updateAvailable.effect(renderUpdateRow);
  updateChecking.effect(renderUpdateRow);

  init();
});

// Also init immediately for hidden-window scenarios (Tauri may not fire DOMContentLoaded)
init().catch(console.error);

(
  window as unknown as { liminalScreen: Record<string, unknown> }
).liminalScreen = {
  deactivateScreensaver: forceDeactivateScreensaver,
  isScreensaverRunning,
  getCurrentOptions,
  openLink,
};
