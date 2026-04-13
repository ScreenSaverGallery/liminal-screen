// Liminal Screen API Library
// Provides a unified interface for remote options pages to communicate with the main app

/**
 * Liminal Screen Options Interface
 */
export interface LiminalOptions {
  /** Time in minutes before screensaver starts */
  startsIn: number;
  /** Time in minutes before display turns off */
  displayOffIn: number;
  /** Time in minutes before password is required */
  requirePassIn: number;
  /** Whether to run on battery power */
  runOnBattery: boolean;
  /** Debug mode enabled */
  debug: boolean;
  /** Main screensaver URL */
  saverUrl: string;
  /** Debug screensaver URL */
  saverUrlDebug: string;
  /** Options page URL */
  optionsUrl: string;
}

/**
 * Custom error class for Liminal API errors
 */
export class LiminalAPIError extends Error {
  constructor(message: string, cause?: unknown) {
    super(message);
    this.name = "LiminalAPIError";
    if (cause !== undefined) {
      Object.defineProperty(this, "cause", {
        value: cause,
        writable: true,
        enumerable: false,
        configurable: true,
      });
    }
  }
}

/**
 * Liminal Screen API Client
 * Handles communication between remote options pages and the main Tauri application
 */
export class LiminalAPI {
  private isTauri: boolean;
  private isInitialized: boolean;
  private securityConfig: {
    sharedSecret: string | undefined;
    requireAuth: boolean;
  };
  private invokeModule: typeof import("@tauri-apps/api/core") | null = null;
  private eventModule: typeof import("@tauri-apps/api/event") | null = null;
  private unlisteners: (() => void)[] = [];

  constructor() {
    this.isTauri = this.checkTauriEnvironment();
    this.isInitialized = false;
    this.securityConfig = {
      sharedSecret:
        typeof process !== "undefined" && process.env
          ? process.env.LIMINAL_API_SECRET
          : undefined,
      requireAuth: false,
    };
  }

  /**
   * Check if we're running in a Tauri environment
   */
  private checkTauriEnvironment(): boolean {
    try {
      return (
        typeof window !== "undefined" &&
        (window as any)["__TAURI__"] !== undefined
      );
    } catch (e) {
      return false;
    }
  }

  /**
   * Initialize the API client
   */
  async init(): Promise<void> {
    if (this.isInitialized) return;

    if (this.isTauri) {
      this.invokeModule = await import("@tauri-apps/api/core");
      this.eventModule = await import("@tauri-apps/api/event");
      await this.setupTauriIPC();
    }

    this.isInitialized = true;
  }

  /**
   * Destroy the API client and clean up all listeners
   */
  destroy(): void {
    for (const unlisten of this.unlisteners) {
      unlisten();
    }
    this.unlisteners = [];
  }

  /**
   * Setup Tauri IPC listeners
   */
  private async setupTauriIPC(): Promise<void> {
    if (this.isTauri && this.eventModule) {
      const { listen } = this.eventModule;

      // Listen for options updates from main app
      const unlisten1 = await listen("options-updated", (event) => {
        this.handleOptionsUpdate(event.payload as LiminalOptions);
      });

      // Note: security-challenge is reserved for future server-side validation
      const unlisten2 = await listen("security-challenge", (event) => {
        this.handleSecurityChallenge(event.payload);
      });

      this.unlisteners.push(unlisten1, unlisten2);
    }
  }

  /**
   * Handle options update from main app
   */
  private handleOptionsUpdate(payload: LiminalOptions): void {
    // Dispatch custom event for remote options pages to listen to
    if (typeof window !== "undefined") {
      const event = new CustomEvent("liminal-options-update", {
        detail: payload,
      });
      window.dispatchEvent(event);
    }
  }

  /**
   * Handle security challenge from main app
   */
  private handleSecurityChallenge(payload: any): void {
    // Dispatch custom event for remote options pages to listen to
    if (typeof window !== "undefined") {
      const event = new CustomEvent("liminal-security-challenge", {
        detail: payload,
      });
      window.dispatchEvent(event);
    }
  }

  /**
   * Get current app options
   */
  async getOptions(authToken?: string): Promise<LiminalOptions> {
    if (this.isTauri && this.invokeModule) {
      try {
        const options = await this.invokeModule.invoke<LiminalOptions>(
          "get_options",
          {
            token: authToken,
          },
        );
        return options;
      } catch (error) {
        throw new LiminalAPIError("Failed to get options", error);
      }
    } else {
      // Return mock options for browser environment
      return this.getDefaultOptions();
    }
  }

  /**
   * Set app options
   */
  async setOptions(
    options: Partial<LiminalOptions>,
    authToken?: string,
  ): Promise<void> {
    if (this.isTauri && this.invokeModule) {
      try {
        // Get current options and merge with partial update
        const currentOptions = await this.getOptions();
        const mergedOptions: LiminalOptions = { ...currentOptions, ...options };
        await this.invokeModule.invoke("set_options", {
          options: mergedOptions,
        });
      } catch (error) {
        throw new LiminalAPIError("Failed to set options", error);
      }
    } else {
      // Mock implementation for browser environment
      console.log("Mock: Setting options", options);
    }
  }

  /**
   * Reset options to factory defaults
   */
  async resetOptions(authToken?: string): Promise<LiminalOptions> {
    if (this.isTauri && this.invokeModule) {
      try {
        const options = await this.invokeModule.invoke<LiminalOptions>(
          "factory_reset_options",
          {
            token: authToken,
          },
        );
        return options;
      } catch (error) {
        throw new LiminalAPIError("Failed to reset options", error);
      }
    } else {
      // Return default options for browser environment
      return this.getDefaultOptions();
    }
  }

  /**
   * Preview the screensaver
   */
  async previewScreensaver(authToken?: string): Promise<void> {
    if (this.isTauri && this.eventModule) {
      try {
        await this.eventModule.emit("preview-screensaver", {});
      } catch (error) {
        throw new LiminalAPIError("Failed to preview screensaver", error);
      }
    } else {
      // Mock implementation for browser environment
      console.log("Mock: Previewing screensaver");
    }
  }

  /**
   * Get default options
   */
  private getDefaultOptions(): LiminalOptions {
    return {
      startsIn: 0.2,
      displayOffIn: 1.0,
      requirePassIn: 1.0,
      runOnBattery: false,
      debug: false,
      saverUrl: "https://save.screensaver.gallery",
      saverUrlDebug: "https://save.screensaver.gallery/debug",
      optionsUrl: "",
    };
  }

  /**
   * Listen for options updates
   */
  onOptionsUpdate(callback: (options: LiminalOptions) => void): () => void {
    if (typeof window !== "undefined") {
      const handler = (event: Event) => {
        callback((event as CustomEvent).detail as LiminalOptions);
      };

      window.addEventListener("liminal-options-update", handler);

      // Return unsubscribe function
      return () => {
        window.removeEventListener("liminal-options-update", handler);
      };
    }

    // Return noop unsubscribe function for non-browser environments
    return () => {};
  }

  /**
   * Check if running in Tauri environment
   */
  isInTauri(): boolean {
    return this.isTauri;
  }

  /**
   * Configure security settings
   */
  configureSecurity(config: {
    sharedSecret?: string;
    requireAuth?: boolean;
  }): void {
    if (config.sharedSecret !== undefined) {
      this.securityConfig.sharedSecret = config.sharedSecret;
    }
    if (config.requireAuth !== undefined) {
      this.securityConfig.requireAuth = config.requireAuth;
    }
  }

  /**
   * Generate authentication token for secure communication
   */
  generateAuthToken(): string | null {
    if (!this.securityConfig.sharedSecret || !this.securityConfig.requireAuth) {
      return null;
    }

    const timestamp = Date.now().toString();
    const nonceArray = new Uint8Array(12);
    crypto.getRandomValues(nonceArray);
    const nonce = Array.from(nonceArray, (b) =>
      b.toString(16).padStart(2, "0"),
    ).join("");
    const data = `${timestamp}:${nonce}:${this.securityConfig.sharedSecret}`;

    // Simple hash (replace with proper crypto in production)
    let hash = 0;
    for (let i = 0; i < data.length; i++) {
      hash = ((hash << 5) - hash + data.charCodeAt(i)) | 0;
    }

    return `${timestamp}.${nonce}.${hash.toString(16)}`;
  }
}

// Export singleton instance
export const liminalAPI = new LiminalAPI();
