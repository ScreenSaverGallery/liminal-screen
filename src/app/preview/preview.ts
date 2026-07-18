import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";

export class Preview {
  private webviewWindow: WebviewWindow | null = null;
  private readonly label: string;
  private readonly url: string;

  constructor(url: string, label?: string) {
    this.url = url;
    this.label = label || `preview-${Date.now()}`;
  }

  async show(onClose?: () => void): Promise<void> {
    if (this.webviewWindow) return;

    // Create via Rust so initialization_script (navigator.id) is injected before page scripts run.
    // The JS WebviewWindow constructor does not support initializationScript in this Tauri version.
    await invoke("create_preview_window", { url: this.url, label: this.label });

    const win = await WebviewWindow.getByLabel(this.label);
    if (!win) throw new Error(`Preview window created but reference not found: ${this.label}`);
    this.webviewWindow = win;

    this.webviewWindow.onCloseRequested(async (event) => {
      // Take over the close: without preventDefault the API auto-destroys the
      // window after this handler, racing the teardown in hide()
      event.preventDefault();
      await this.hide();
      onClose?.();
    });
  }

  async hide(): Promise<void> {
    // Claim the reference up front so a concurrent hide() (close-requested
    // re-entry) is a no-op instead of dereferencing null after the sleep
    const win = this.webviewWindow;
    if (!win) return;
    this.webviewWindow = null;

    try {
      await invoke("navigate_webview", { label: this.label, url: "about:blank" });
    } catch { /* ignore */ }

    await new Promise((resolve) => setTimeout(resolve, 100));

    try { await win.hide(); } catch { /* ignore */ }

    // destroy() (not close()) — close() emits another close-requested event,
    // re-entering the onCloseRequested handler; destroy tears down directly
    try { await win.destroy(); } catch { /* ignore */ }
  }

  isOpen(): boolean { return this.webviewWindow !== null; }
  getLabel(): string { return this.label; }
  getUrl(): string { return this.url; }

  async focus(): Promise<void> {
    await this.webviewWindow?.setFocus();
  }
}
