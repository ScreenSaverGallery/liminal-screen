import { invoke } from "@tauri-apps/api/core";

export class Preview {
  private readonly label: string;

  constructor() {
    // The Rust backend owns a single reusable preview webview with this label.
    this.label = "preview";
  }

  async show(url: string): Promise<void> {
    // create_preview_window creates the singleton on first use and reuses it
    // (re-navigating + re-showing) on subsequent calls.
    await invoke("create_preview_window", { url, label: this.label });
  }

  async hide(): Promise<void> {
    // Park the preview window: stop media, navigate to about:blank, hide.
    // The Rust CloseRequested handler does the same when the user clicks the
    // window chrome. We avoid destroy() because wry leaks the underlying webview
    // process on macOS.
    try {
      await invoke("park_webview_window_command", { label: this.label });
    } catch { /* ignore */ }
  }

  getLabel(): string { return this.label; }
}
