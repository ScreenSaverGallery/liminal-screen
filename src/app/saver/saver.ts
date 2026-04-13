// Saver Class - Manages individual fullscreen screensaver windows for each display
// Handles window creation, positioning, lifecycle, and inter-window communication

import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { emit, emitTo } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";

/**
 * Saver options interface
 */
export interface SaverOptions {
  /** Debug mode enabled */
  debug: boolean;
  /** Window is muted */
  muted?: boolean;
  /** Additional custom properties */
  [key: string]: unknown;
}

/**
 * Monitor position interface
 */
export interface MonitorPosition {
  x: number;
  y: number;
}

/**
 * Monitor size interface
 */
export interface MonitorSize {
  width: number;
  height: number;
}

/**
 * Default saver options
 */
const DefaultOptions: SaverOptions = {
  debug: false,
};

/**
 * Saver Class - Manages individual fullscreen screensaver windows
 */
export class Saver {
  private webviewWindow: WebviewWindow | null = null;
  private readonly label: string;
  private readonly url: string;
  private readonly monitorPosition?: MonitorPosition;
  private readonly monitorSize?: MonitorSize;
  private readonly options: SaverOptions;

  /**
   * Create a new Saver instance
   * @param url - URL to display in the screensaver
   * @param label - Unique window identifier
   * @param monitorPosition - Monitor top-left position {x, y}
   * @param monitorSize - Window dimensions {width, height}
   * @param options - Configuration options
   */
  constructor(
    url: string,
    label?: string,
    monitorPosition?: MonitorPosition,
    monitorSize?: MonitorSize,
    options?: SaverOptions,
  ) {
    this.url = url;
    this.label = label || `saver-${Date.now()}`;
    this.monitorPosition = monitorPosition;
    this.monitorSize = monitorSize;
    this.options = options ?? DefaultOptions;
  }

  /**
   * Create and show the fullscreen saver window
   */
  async show(): Promise<void> {
    if (this.webviewWindow) {
      console.warn(`Saver window ${this.label} already exists`);
      return;
    }

    try {
      // Create window options for fullscreen saver
      const windowOptions = {
        url: this.url,
        userAgent: `${navigator.userAgent} LiminalSaver/${await getVersion()}`,
        focus: true,
        resizable: false,
        decorations: false, // No title bar, borders for fullscreen
        transparent: false,
        visible: true,
        alwaysOnTop: true, // Stay above other windows
        skipTaskbar: true, // Don't show in taskbar
        title: "saver",
        backgroundColor: "#000000",
        devtools: this.options.debug,
        ...(this.monitorPosition && {
          x: this.monitorPosition.x,
          y: this.monitorPosition.y,
        }),
        ...(this.monitorSize && {
          width: this.monitorSize.width,
          height: this.monitorSize.height,
        }),
      };

      // Create the window using Tauri API
      this.webviewWindow = new WebviewWindow(this.label, windowOptions);

      // Wait for window to be created
      await new Promise<void>((resolve, reject) => {
        let resolved = false;

        if (this.webviewWindow) {
          // Listen for window creation
          this.webviewWindow.once("tauri://created", async () => {
            if (!resolved && this.webviewWindow) {
              resolved = true;

              try {
                // Set fullscreen and maximize
                const isFullscreen = await this.webviewWindow.isFullscreen();
                if (!isFullscreen) {
                  await this.webviewWindow.setFullscreen(true);
                  await this.webviewWindow.maximize();
                }

                // Setup custom navigator properties
                await this.setupCustomNavigator();

                resolve();
              } catch (error) {
                console.error(
                  `Error configuring saver window ${this.label}:`,
                  error,
                );
                resolve(); // Resolve anyway, window is created
              }
            }
          });

          // Listen for window creation errors
          this.webviewWindow.once("tauri://error", (error) => {
            if (!resolved) {
              resolved = true;
              reject(
                new Error(`Failed to create saver window: ${error.payload}`),
              );
            }
          });
        }

        // Timeout after 5 seconds
        setTimeout(() => {
          if (!resolved) {
            resolved = true;
            reject(new Error("Timeout while creating saver window"));
          }
        }, 5000);
      });

      console.log(`Saver window created with label: ${this.label}`);
    } catch (error) {
      console.error("Error creating saver window:", error);
      throw error;
    }
  }

  /**
   * Setup custom navigator properties
   * Injects SaverOptions as navigator properties for the screensaver page
   */
  private async setupCustomNavigator(): Promise<void> {
    if (!this.webviewWindow) return;

    const script = `
      (function() {
        window.__SAVER_OPTIONS__ = ${JSON.stringify(this.options)};
        window.navigator.saver = ${JSON.stringify(this.options)};
      })();
    `;

    try {
      await invoke("evaluate_javascript", {
        label: this.label,
        script: script,
      });
    } catch (error) {
      console.error("Failed to setup custom navigator:", error);
    }
  }

  /**
   * Hide and close the saver window
   * Stops media playback before closing
   */
  async hide(): Promise<void> {
    if (!this.webviewWindow) {
      console.warn(`Saver window ${this.label} does not exist`);
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
        console.warn("Could not navigate to about:blank:", navError);
      }

      // Small delay to ensure navigation completes
      await new Promise((resolve) => setTimeout(resolve, 100));

      // Hide the window
      try {
        await this.webviewWindow.hide();
      } catch (hideError) {
        console.warn("Could not hide webview window:", hideError);
      }

      // Close the window
      await this.webviewWindow.close();
      this.webviewWindow = null;

      console.log(`Saver window ${this.label} closed`);
    } catch (error) {
      console.error(`Error hiding saver window ${this.label}:`, error);
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
   * Focus the saver window
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

  /**
   * Send an IPC event to this specific saver window
   */
  async emit(event: string, payload?: unknown): Promise<void> {
    if (!this.webviewWindow) {
      console.warn(`Cannot emit to closed window ${this.label}`);
      return;
    }

    try {
      await emitTo(this.label, event, payload);
    } catch (error) {
      console.error(`Failed to emit event to ${this.label}:`, error);
    }
  }

  // ============================================
  // Static Methods
  // ============================================

  /**
   * Emit an event to all active saver windows
   */
  static async emitToAll(event: string, payload?: unknown): Promise<void> {
    try {
      await emit(event, payload);
    } catch (error) {
      console.error(`Failed to emit event ${event} to all savers:`, error);
    }
  }
}
