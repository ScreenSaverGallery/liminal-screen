// Power Monitor Class - TypeScript wrapper for Rust power monitor plugin
// Provides system idle detection and power management functionality

import { invoke } from "@tauri-apps/api/core";

/**
 * PowerMonitor class - Wrapper for Rust power monitor plugin commands
 * Tracks system idle time and manages power states across platforms
 */
export class PowerMonitor {
  private static blockerId: number | null = null;

  /**
   * Get system idle time in seconds
   * @returns Promise<number> - Idle time in seconds
   */
  static async getSystemIdleTime(): Promise<number> {
    try {
      const idleTime = await invoke<number>("get_system_idle_time");
      return idleTime;
    } catch (error) {
      console.error("Failed to get system idle time:", error);
      return 0;
    }
  }

  /**
   * Get system idle state ('idle' or 'active')
   * @param threshold - Threshold in seconds for considering system idle
   * @returns Promise<string> - 'idle' or 'active'
   */
  static async getSystemIdleState(
    threshold: number,
  ): Promise<"idle" | "active"> {
    try {
      const state = await invoke<string>("get_system_idle_state", {
        threshold,
      });
      return state as "idle" | "active";
    } catch (error) {
      console.error("Failed to get system idle state:", error);
      return "active";
    }
  }

  /**
   * Check if system is running on battery power
   * @returns Promise<boolean> - true if on battery, false if on AC
   */
  static async isOnBatteryPower(): Promise<boolean> {
    try {
      const onBattery = await invoke<boolean>("is_on_battery_power");
      return onBattery;
    } catch (error) {
      console.error("Failed to check battery power:", error);
      return false;
    }
  }

  /**
   * Prevent display from sleeping
   * Call this when screensaver activates
   */
  static async preventDisplaySleep(): Promise<void> {
    if (this.blockerId !== null) {
      console.warn("Display sleep already prevented");
      return;
    }

    try {
      const blockerId = await invoke<number>("prevent_display_sleep");
      this.blockerId = blockerId;
      console.log("Display sleep prevented, blocker ID:", blockerId);
    } catch (error) {
      console.error("Failed to prevent display sleep:", error);
    }
  }

  /**
   * Allow display to sleep
   * Call this when screensaver deactivates
   */
  static async allowDisplaySleep(): Promise<void> {
    if (this.blockerId === null) {
      console.warn("Display sleep not currently prevented");
      return;
    }

    try {
      await invoke("allow_display_sleep", {
        blockerId: this.blockerId,
      });
      console.log("Display sleep allowed, blocker ID:", this.blockerId);
      this.blockerId = null;
    } catch (error) {
      console.error("Failed to allow display sleep:", error);
    }
  }

  /**
   * Blank/turn off the screen immediately
   */
  static async blankScreen(): Promise<void> {
    try {
      await invoke("blank_screen");
      console.log("Screen blanked");
    } catch (error) {
      console.error("Failed to blank screen:", error);
    }
  }

  /**
   * Lock the screen
   */
  static async lockScreen(): Promise<void> {
    try {
      await invoke("lock_screen");
      console.log("Screen locked");
    } catch (error) {
      console.error("Failed to lock screen:", error);
    }
  }

  /**
   * Check if display sleep is currently prevented
   * @returns boolean
   */
  static isDisplaySleepPrevented(): boolean {
    return this.blockerId !== null;
  }

  /**
   * Reset the blocker ID (use with caution)
   * Useful for recovery if state gets out of sync
   */
  static resetBlocker(): void {
    this.blockerId = null;
  }
}

/**
 * MonitorInfo interface - Represents a connected display
 */
export interface MonitorInfo {
  /** Zero-based index */
  id: number;
  /** Display name or "Unknown" */
  name: string;
  /** x, y coordinates */
  position: Position;
  /** width, height in pixels */
  size: Size;
  /** DPI scaling factor (1.0 = 100%, 2.0 = 200%) */
  scale_factor: number;
}

/**
 * Position interface - Monitor coordinates
 */
export interface Position {
  x: number;
  y: number;
}

/**
 * Size interface - Monitor dimensions
 */
export interface Size {
  width: number;
  height: number;
}
