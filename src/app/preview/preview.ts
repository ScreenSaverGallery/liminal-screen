// Preview Class - Manages screensaver preview windows
// Handles window creation, lifecycle, and inter-window communication for previews

import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";

/**
 * Preview options interface
 */
export interface PreviewOptions {
  /** Debug mode enabled */
  debug: boolean;
  /** Additional custom properties */
  [key: string]: unknown;
}

/**
 * Default preview options
 */
const DefaultOptions: PreviewOptions = {
  debug: false,
};

/**
 * Preview Class - Manages screensaver preview windows
 */
export class Preview {
  private webviewWindow: WebviewWindow | null = null;
  private readonly label: string;
  private readonly url: string;
  private readonly options: PreviewOptions;

  /**
   * Create a new Preview instance
   * @param url - URL to display in the preview
   * @param label - Unique window identifier
   * @param options - Configuration options (unused but kept for API compatibility)
   */
  constructor(
    url: string,
    label?: string,
  ) {
    this.url = url;
    this.label = label || `preview-${Date.now()}`;
    this.options = DefaultOptions; // Use default options always
  }

  /**
   * Create and show the preview window
   */
  async show(): Promise<void> {
    if (this.webviewWindow) {
      console.warn(`Preview window ${this.label} already exists`);
      return;
    }

    try {
      // Create window options for preview
      const windowOptions = {
        url: this.url,
        title: "Screensaver Preview",
        width: 800,
        height: 600,
        resizable: true,
        decorations: true,
        visible: true,
        alwaysOnTop: false,
        skipTaskbar: false,
      };

      console.log(
        `Creating Preview WebviewWindow with label: ${this.label}`,
        windowOptions,
      );
      // Create the window using Tauri API
      this.webviewWindow = new WebviewWindow(this.label, windowOptions);

      console.log(`Waiting for preview window ${this.label} to be created...`);
      // Wait for window to be created
      await new Promise<void>((resolve, reject) => {
        let resolved = false;

        if (this.webviewWindow) {
          // Listen for window creation
          this.webviewWindow.once("tauri://created", async () => {
            console.log(`Preview window ${this.label} created successfully`);
            if (!resolved) {
              resolved = true;
              resolve();
            }
          });

          // Listen for window creation errors
          this.webviewWindow.once("tauri://error", (error) => {
            console.log(`Error creating preview window ${this.label}:`, error);
            if (!resolved) {
              resolved = true;
              reject(
                new Error(`Failed to create preview window: ${error.payload}`),
              );
            }
          });
        }

        // Timeout after 5 seconds
        setTimeout(() => {
          if (!resolved) {
            resolved = true;
            reject(new Error("Timeout while creating preview window"));
          }
        }, 5000);
      });

      // Set up cleanup handler when window is closed
      this.webviewWindow.onCloseRequested(async () => {
        console.log("Preview window close requested, cleaning up media...");
        await this.hide();
      });

      console.log(`Preview window created with label: ${this.label}`);
    } catch (error) {
      console.error("Error creating preview window:", error);
      throw error;
    }
  }

  /**
   * Hide and close the preview window
   * Stops media playback before closing
   */
  async hide(): Promise<void> {
    if (!this.webviewWindow) {
      console.warn(`Preview window ${this.label} does not exist`);
      return;
    }

    try {
      // Navigate to about:blank to stop all media playback
      try {
        await invoke("navigate_webview", {
          label: this.label,
          url: "about:blank",
        });
      } catch (navError) {
        console.warn("Could not navigate preview window to about:blank:", navError);
      }

      // Small delay to ensure navigation completes
      await new Promise((resolve) => setTimeout(resolve, 100));

      // Hide the window
      try {
        await this.webviewWindow.hide();
      } catch (hideError) {
        console.warn("Could not hide preview window:", hideError);
      }

      // Close the window
      await this.webviewWindow.close();
      this.webviewWindow = null;

      console.log(`Preview window ${this.label} closed`);
    } catch (error) {
      console.error(`Error hiding preview window ${this.label}:`, error);
      // Ensure we still null the reference even if close fails
      this.webviewWindow = null;
      throw error;
    }
  }

  /**
   * Check if the window is currently open
   */
  isOpen(): boolean {
    return this.webviewWindow !== null;
  }

  /**
   * Check if the window is currently visible/active
   */
  isActive(): boolean {
    return this.webviewWindow !== null;
  }

  /**
   * Get the window label
   */
  getLabel(): string {
    return this.label;
  }

  /**
   * Get the URL being displayed
   */
  getUrl(): string {
    return this.url;
  }

  /**
   * Focus the preview window
   */
  async focus(): Promise<void> {
    if (this.webviewWindow) {
      await this.webviewWindow.setFocus();
    }
  }

  /**
   * Listen for window events
   */
  async listen<T>(
    event: string,
    handler: (event: { payload: T }) => void,
  ): Promise<() => void> {
    if (!this.webviewWindow) {
      throw new Error("Window not created yet. Call show() first.");
    }

    return await this.webviewWindow.listen<T>(event, handler);
  }

  /**
   * Listen for window events only once
   */
  async once<T>(
    event: string,
    handler: (event: { payload: T }) => void,
  ): Promise<() => void> {
    if (!this.webviewWindow) {
      throw new Error("Window not created yet. Call show() first.");
    }

    return await this.webviewWindow.once<T>(event, handler);
  }
}