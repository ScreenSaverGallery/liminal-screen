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
}
