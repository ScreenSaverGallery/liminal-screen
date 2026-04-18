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

  async show(): Promise<void> {
    if (this.webviewWindow) return;

    this.webviewWindow = new WebviewWindow(this.label, {
      url: this.url,
      title: "Screensaver Preview",
      width: 800,
      height: 600,
      resizable: true,
      decorations: true,
      visible: true,
      alwaysOnTop: false,
      skipTaskbar: false,
    });

    await new Promise<void>((resolve, reject) => {
      let resolved = false;

      this.webviewWindow!.once("tauri://created", () => {
        if (!resolved) { resolved = true; resolve(); }
      });
      this.webviewWindow!.once("tauri://error", (error) => {
        if (!resolved) { resolved = true; reject(new Error(`Failed to create preview window: ${error.payload}`)); }
      });
      setTimeout(() => {
        if (!resolved) { resolved = true; reject(new Error("Timeout while creating preview window")); }
      }, 5000);
    });

    this.webviewWindow.onCloseRequested(async () => { await this.hide(); });
  }

  async hide(): Promise<void> {
    if (!this.webviewWindow) return;

    try {
      await invoke("navigate_webview", { label: this.label, url: "about:blank" });
    } catch { /* ignore */ }

    await new Promise((resolve) => setTimeout(resolve, 100));

    try { await this.webviewWindow.hide(); } catch { /* ignore */ }

    try {
      await this.webviewWindow.close();
    } finally {
      this.webviewWindow = null;
    }
  }

  isOpen(): boolean { return this.webviewWindow !== null; }
  getLabel(): string { return this.label; }
  getUrl(): string { return this.url; }

  async focus(): Promise<void> {
    await this.webviewWindow?.setFocus();
  }
}
