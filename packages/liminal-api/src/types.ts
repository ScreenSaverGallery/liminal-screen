/**
 * Mandatory options — always present, user-configurable via the options page.
 */
export interface MandatoryOptions {
  /** Minutes of inactivity before screensaver activates */
  startsIn: number;
  /** Minutes before display turns off (0 = disabled) */
  displayOffIn: number;
  /** Minutes before system lock (0 = disabled) */
  requirePassIn: number;
  /** Run screensaver on battery power */
  runOnBattery: boolean;
  /** Enable debug mode (loads saverUrlDebug instead of saverUrl) */
  debug: boolean;
  /**
   * User consent for feed notifications — opt-in, defaults to false.
   * Only meaningful when the fork configures a notification feed URL.
   * Optional in payloads: liminalAPI.setOptions() merges with the current
   * options, so omitting it leaves the user's consent unchanged.
   */
  notificationsEnabled?: boolean;
  /**
   * Start Liminal at login. Mirrors the OS login-item state, which is the
   * source of truth — the backend applies the change and reports back what
   * the OS accepted, so the saved value may differ from the requested one.
   * Optional in payloads: omitting it leaves the current state unchanged.
   */
  autostart?: boolean;
}

/**
 * Custom options — fork-defined key/value pairs.
 * Primitives only; these are appended to the screensaver URL as query parameters.
 */
export type CustomOptions = Record<string, string | number | boolean>;

/**
 * Full application options as returned by get_options.
 * Read-only fields (saverUrl, optionsUrl, appName, appDescription) come from
 * the fork's .env and cannot be changed by the user.
 */
export interface AppOptions extends MandatoryOptions {
  /** Screensaver URL (production) */
  saverUrl: string;
  /** Screensaver URL (debug mode) */
  saverUrlDebug: string;
  /** Remote options page URL */
  optionsUrl: string;
  /** Fork display name */
  appName: string;
  /** Fork description */
  appDescription: string;
  /** Fork-defined custom fields */
  customOptions: CustomOptions;
  /** Instance UUID (read-only; regenerated on factory reset) */
  instanceId: string;
  /** User consent for feed notifications — always present in get_options results */
  notificationsEnabled: boolean;
  /** Notification feed URL (env-controlled, read-only; empty = feature disabled) */
  notificationUrl: string;
  /** Notification poll interval in seconds (env-controlled, read-only) */
  notificationCheckIntervalSecs: number;
  /** Start at login — always present in get_options results (reflects the OS state) */
  autostart: boolean;
}

/**
 * Payload accepted by setOptions — mandatory fields plus optional custom options.
 * Read-only identity fields (saverUrl, appName, etc.) are always preserved by the backend.
 */
export type SetOptionsPayload = MandatoryOptions & { customOptions?: CustomOptions };

/**
 * OS-native screensaver configuration, as returned by getOsScreensaverStatus().
 * Liminal is meant to be the only screensaver — a system screensaver on an
 * overlapping timer draws over Liminal, so the options page can warn about it
 * and offer to disable it.
 */
export interface OsScreensaverStatus {
  /** Whether the setting could be read on this platform/desktop. */
  detected: boolean;
  /** True when the OS screensaver is set to activate on an idle timer. */
  enabled: boolean;
  /** Idle seconds before the OS screensaver starts; null when disabled/unknown. */
  idleSeconds: number | null;
}

/**
 * Info about an available application update, as returned by
 * checkForUpdates() and delivered by the `update-available` event.
 */
export interface UpdateInfo {
  /** Version string of the available update */
  version: string;
  /** Release notes, when provided by the release */
  notes?: string;
}
