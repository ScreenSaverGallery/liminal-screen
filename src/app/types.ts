/** Full application options — mirrors the Rust AppOptions struct (camelCase via serde rename). */
export interface AppOptions {
  saverUrl: string;
  saverUrlDebug: string;
  optionsUrl: string;
  appName: string;
  appDescription: string;
  startsIn: number;
  displayOffIn: number;
  requirePassIn: number;
  runOnBattery: boolean;
  debug: boolean;
  customOptions: Record<string, string | number | boolean>;
  instanceId: string;
  /** Notification feed URL — env only, empty = disabled */
  notificationUrl: string;
  /** Notification poll interval in seconds — env only */
  notificationCheckIntervalSecs: number;
  /** User consent for notifications — persisted, opt-in (default false) */
  notificationsEnabled: boolean;
}

/**
 * Navigator extensions injected at document-start into every remote window
 * (saver, options, preview) by the native init script — see `build_init_script`
 * in `src-tauri/src/lib.rs`. The same identity is appended to both
 * `navigator.userAgent` and `navigator.appVersion` as
 * `LiminalScreen/{version} ({appName})`.
 */
declare global {
  interface Navigator {
    /** Instance UUID — equals AppOptions.instanceId; changes on factory reset. */
    readonly id: string;
    /** Frozen snapshot of all app options, plus the native app version. */
    readonly liminalScreen: Readonly<AppOptions & { version: string }>;
  }
}
