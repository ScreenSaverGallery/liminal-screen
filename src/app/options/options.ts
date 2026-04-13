// Options Module - Manages the Options configuration window
// Handles loading remote options pages with offline support via service worker

import { emit, listen, Event } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Storage, RemoteOptions } from "../storage/storage";

/**
 * Options window manager class
 */
export class OptionsManager {
  private static isInitialized = false;

  /**
   * Initialize the options manager
   * Sets up event listeners for options-related events
   */
  static async init(): Promise<void> {
    if (this.isInitialized) return;

    // Listen for save-options event from Options window
    await listen<RemoteOptions>("save-options", async (event: Event<RemoteOptions>) => {
      await this.handleSaveOptions(event.payload);
    });

    // Listen for request-current-options event
    await listen("request-current-options", async () => {
      await this.sendCurrentOptions();
    });

    // Listen for reset-options event
    await listen("reset-options", async () => {
      await this.handleResetOptions();
    });

    this.isInitialized = true;
    console.log("OptionsManager initialized");
  }

  /**
   * Handle save-options event
   * Stores form data from Options window
   */
  private static async handleSaveOptions(formData: RemoteOptions): Promise<void> {
    try {
      // Store in persistent storage
      await Storage.setRemoteOptions(formData);

      // Emit options-updated event to notify all windows
      await emit("options-updated", formData);

      console.log("Options saved:", formData);
    } catch (error) {
      console.error("Failed to save options:", error);
    }
  }

  /**
   * Send current options to requester
   */
  private static async sendCurrentOptions(): Promise<void> {
    try {
      const options = await Storage.getOptions();
      await emit("current-options", options);
    } catch (error) {
      console.error("Failed to send current options:", error);
    }
  }

  /**
   * Handle reset-options event
   * Clears all options to defaults
   */
  private static async handleResetOptions(): Promise<void> {
    try {
      await Storage.factoryReset();

      // Notify all windows
      const options = await Storage.getOptions();
      await emit("options-updated", options);

      console.log("Options reset to defaults");
    } catch (error) {
      console.error("Failed to reset options:", error);
    }
  }

  /**
   * Open the options window
   * This triggers the Rust side to create the window
   */
  static async openOptions(): Promise<void> {
    await emit("open-options-window");
  }

  /**
   * Get the options URL from environment
   */
  static getOptionsUrl(): string {
    return import.meta.env.VITE_OPTIONS_URL || "https://example.com/options";
  }

  /**
   * Get the screensaver URL with query parameters
   * Combines base URL with stored remote options
   */
  static async getSaverUrl(withDebug: boolean = false): Promise<string> {
    const baseUrl = withDebug
      ? import.meta.env.VITE_SAVER_URL_DEBUG || "https://example.com/debug"
      : import.meta.env.VITE_SAVER_URL || "https://example.com/screensaver";

    const remoteOptions = await Storage.getRemoteOptions();

    // Build query string from remote options
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(remoteOptions)) {
      if (value !== undefined && value !== null) {
        params.append(key, String(value));
      }
    }

    const queryString = params.toString();
    return queryString ? `${baseUrl}?${queryString}` : baseUrl;
  }
}

/**
 * Service Worker Registration
 * Registers the service worker for offline support
 */
export async function registerServiceWorker(): Promise<void> {
  if ("serviceWorker" in navigator) {
    try {
      const registration = await navigator.serviceWorker.register("/sw.js");
      console.log("Service Worker registered:", registration);
    } catch (error) {
      console.error("Service Worker registration failed:", error);
    }
  }
}

/**
 * Open external link in system browser
 */
export async function openExternalLink(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch (error) {
    console.error("Failed to open external link:", error);
    // Fallback to window.open
    window.open(url, "_blank");
  }
}
