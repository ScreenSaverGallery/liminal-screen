import { invoke } from "@tauri-apps/api/core";

import type { OsScreensaverStatus } from "../types";

export class PowerMonitor {
  static async getSystemIdleTime(): Promise<number> {
    try {
      return await invoke<number>("get_system_idle_time");
    } catch {
      return 0;
    }
  }

  /** Read the OS-native screensaver config to detect conflicts with Liminal. */
  static async getOsScreensaverStatus(): Promise<OsScreensaverStatus> {
    try {
      return await invoke<OsScreensaverStatus>("get_os_screensaver_status");
    } catch {
      return { detected: false, enabled: false, idleSeconds: null };
    }
  }

  /** Disable the OS screensaver so it can't appear over Liminal (reversible). */
  static async disableOsScreensaver(): Promise<void> {
    await invoke("disable_os_screensaver");
  }

  /** Restore the OS screensaver to the value saved when it was disabled. */
  static async restoreOsScreensaver(): Promise<void> {
    await invoke("restore_os_screensaver");
  }

  /** Saved OS screensaver timeout (seconds) if Liminal disabled it, else null. */
  static async getSavedOsScreensaverIdle(): Promise<number | null> {
    try {
      return await invoke<number | null>("get_saved_os_screensaver_idle");
    } catch {
      return null;
    }
  }
}
