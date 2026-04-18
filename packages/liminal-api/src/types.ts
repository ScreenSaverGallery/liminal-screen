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
}

/**
 * Payload accepted by setOptions — mandatory fields plus optional custom options.
 * Read-only identity fields (saverUrl, appName, etc.) are always preserved by the backend.
 */
export type SetOptionsPayload = MandatoryOptions & { customOptions?: CustomOptions };
