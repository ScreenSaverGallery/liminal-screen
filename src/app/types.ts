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
