import { invoke } from "@tauri-apps/api/core";

/// Label of the single preview webview owned by the Rust backend.
const PREVIEW_LABEL = "preview";

export class Preview {
  async show(url: string): Promise<void> {
    // create_preview_window builds the webview on first use and re-navigates it
    // on later calls. It is never destroyed — see park_webview_window in lib.rs.
    await invoke("create_preview_window", { url });
  }

  async hide(): Promise<void> {
    // Park it: stop media, blank it, hide it. The Rust CloseRequested handler
    // does the same when the user clicks the window's own close button.
    try {
      await invoke("park_webview_window_command", { label: PREVIEW_LABEL });
    } catch { /* ignore */ }
  }

  getLabel(): string { return PREVIEW_LABEL; }
}
