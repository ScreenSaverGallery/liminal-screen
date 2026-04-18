import { invoke } from "@tauri-apps/api/core";

export class PowerMonitor {
  static async getSystemIdleTime(): Promise<number> {
    try {
      return await invoke<number>("get_system_idle_time");
    } catch {
      return 0;
    }
  }
}
