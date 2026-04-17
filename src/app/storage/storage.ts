// Storage Module - Persistent configuration storage using Tauri Store plugin
// Handles both mandatory options and remote form parameters

import { load } from "@tauri-apps/plugin-store";

/**
 * Mandatory options interface - Application-level configuration
 */
export interface MandatoryOptions {
  /** Minutes of inactivity before screensaver activates */
  startsIn: number;
  /** Minutes before display turns off */
  displayOffIn: number;
  /** Minutes after which password is required (0 = no password) */
  requirePassIn: number;
  /** Whether to run screensaver on battery power */
  runOnBattery: boolean;
  /** Enable debug mode */
  debug: boolean;
}

/**
 * Remote options interface - Form data from Options window
 */
export interface RemoteOptions {
  [key: string]: unknown;
}

/**
 * Complete application options
 */
export interface AppOptions extends MandatoryOptions {
  /** Remote form parameters */
  remoteOptions?: RemoteOptions;
}

// Default values for mandatory options
// These are loaded from .env at build time via Vite's import.meta.env
// Users can override via the options UI; these apply on first install or factory reset
const DEFAULT_MANDATORY_OPTIONS: MandatoryOptions = {
  startsIn: parseFloat(import.meta.env.VITE_DEFAULT_STARTS_IN) || 0.2,
  displayOffIn: parseFloat(import.meta.env.VITE_DEFAULT_DISPLAY_OFF_IN) || 1,
  requirePassIn: parseFloat(import.meta.env.VITE_DEFAULT_REQUIRE_PASS_IN) || 1,
  runOnBattery: import.meta.env.VITE_DEFAULT_RUN_ON_BATTERY === "true" || false,
  debug: import.meta.env.VITE_DEFAULT_DEBUG === "true" || false,
};

// Store file name
const STORE_FILE = "options.json";

// Keys for storage
const KEYS = {
  STARTS_IN: "startsIn",
  DISPLAY_OFF_IN: "displayOffIn",
  REQUIRE_PASS_IN: "requirePassIn",
  RUN_ON_BATTERY: "runOnBattery",
  DEBUG: "debug",
  REMOTE_OPTIONS: "remoteOptions",
};

/**
 * Storage class - Manages persistent configuration
 */
export class Storage {
  private static store: Awaited<ReturnType<typeof load>> | null = null;
  private static initialized = false;

  /**
   * Initialize the storage
   * Must be called before using other methods
   */
  static async init(): Promise<void> {
    if (this.initialized) return;

    this.store = await load(STORE_FILE, { autoSave: true, defaults: {} });
    this.initialized = true;

    // Set defaults if not present
    await this.setDefaults();

    console.log("Storage initialized");
  }

  /**
   * Set default values for all mandatory options
   */
  private static async setDefaults(): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    // Only set defaults if values don't exist
    if ((await this.store.get(KEYS.STARTS_IN)) === undefined) {
      await this.store.set(KEYS.STARTS_IN, DEFAULT_MANDATORY_OPTIONS.startsIn);
    }
    if ((await this.store.get(KEYS.DISPLAY_OFF_IN)) === undefined) {
      await this.store.set(
        KEYS.DISPLAY_OFF_IN,
        DEFAULT_MANDATORY_OPTIONS.displayOffIn,
      );
    }
    if ((await this.store.get(KEYS.REQUIRE_PASS_IN)) === undefined) {
      await this.store.set(
        KEYS.REQUIRE_PASS_IN,
        DEFAULT_MANDATORY_OPTIONS.requirePassIn,
      );
    }
    if ((await this.store.get(KEYS.RUN_ON_BATTERY)) === undefined) {
      await this.store.set(
        KEYS.RUN_ON_BATTERY,
        DEFAULT_MANDATORY_OPTIONS.runOnBattery,
      );
    }
    if ((await this.store.get(KEYS.DEBUG)) === undefined) {
      await this.store.set(KEYS.DEBUG, DEFAULT_MANDATORY_OPTIONS.debug);
    }
  }

  /**
   * Get mandatory options
   */
  static async getMandatoryOptions(): Promise<MandatoryOptions> {
    if (!this.store) throw new Error("Storage not initialized");

    return {
      startsIn:
        (await this.store.get<number>(KEYS.STARTS_IN)) ??
        DEFAULT_MANDATORY_OPTIONS.startsIn,
      displayOffIn:
        (await this.store.get<number>(KEYS.DISPLAY_OFF_IN)) ??
        DEFAULT_MANDATORY_OPTIONS.displayOffIn,
      requirePassIn:
        (await this.store.get<number>(KEYS.REQUIRE_PASS_IN)) ??
        DEFAULT_MANDATORY_OPTIONS.requirePassIn,
      runOnBattery:
        (await this.store.get<boolean>(KEYS.RUN_ON_BATTERY)) ??
        DEFAULT_MANDATORY_OPTIONS.runOnBattery,
      debug:
        (await this.store.get<boolean>(KEYS.DEBUG)) ??
        DEFAULT_MANDATORY_OPTIONS.debug,
    };
  }

  /**
   * Set mandatory options
   */
  static async setMandatoryOptions(options: MandatoryOptions): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    await this.store.set(KEYS.STARTS_IN, options.startsIn);
    await this.store.set(KEYS.DISPLAY_OFF_IN, options.displayOffIn);
    await this.store.set(KEYS.REQUIRE_PASS_IN, options.requirePassIn);
    await this.store.set(KEYS.RUN_ON_BATTERY, options.runOnBattery);
    await this.store.set(KEYS.DEBUG, options.debug);

    await this.save();
  }

  /**
   * Get remote options (form data from Options window)
   */
  static async getRemoteOptions(): Promise<RemoteOptions> {
    if (!this.store) throw new Error("Storage not initialized");

    return (await this.store.get<RemoteOptions>(KEYS.REMOTE_OPTIONS)) ?? {};
  }

  /**
   * Set remote options (form data from Options window)
   */
  static async setRemoteOptions(options: RemoteOptions): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    await this.store.set(KEYS.REMOTE_OPTIONS, options);
    await this.save();
  }

  /**
   * Get all options (mandatory + remote)
   */
  static async getOptions(): Promise<AppOptions> {
    const mandatory = await this.getMandatoryOptions();
    const remote = await this.getRemoteOptions();

    return {
      ...mandatory,
      remoteOptions: remote,
    };
  }

  /**
   * Set a single option value
   */
  static async set<T>(key: string, value: T): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    await this.store.set(key, value);
    await this.save();
  }

  /**
   * Get a single option value
   */
  static async get<T>(key: string): Promise<T | undefined> {
    if (!this.store) throw new Error("Storage not initialized");

    return await this.store.get<T>(key);
  }

  /**
   * Delete a single option
   */
  static async delete(key: string): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    await this.store.delete(key);
    await this.save();
  }

  /**
   * Clear all options (factory reset)
   */
  static async factoryReset(): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    await this.store.clear();
    await this.setDefaults();
    await this.save();

    console.log("Factory reset complete");
  }

  /**
   * Save changes to disk
   */
  static async save(): Promise<void> {
    if (!this.store) throw new Error("Storage not initialized");

    await this.store.save();
  }

  /**
   * Get the underlying store instance (for advanced usage)
   */
  static getStore(): Awaited<ReturnType<typeof load>> {
    if (!this.store) throw new Error("Storage not initialized");
    return this.store;
  }
}
