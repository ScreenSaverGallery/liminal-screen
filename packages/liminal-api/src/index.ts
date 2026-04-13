// Liminal Screen API Library
// Provides a unified interface for remote options pages to communicate with the main app

/**
 * Liminal Screen API Client
 * Handles communication between remote options pages and the main Tauri application
 */
export class LiminalAPI {
  private isTauri: boolean;
  private isInitialized: boolean;
  private securityConfig: any;

  constructor() {
    this.isTauri = this.checkTauriEnvironment();
    this.isInitialized = false;
    this.securityConfig = {
      sharedSecret:
        typeof process !== "undefined"
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
      // In Tauri environment, we can use real IPC
      await this.setupTauriIPC();
    }

    this.isInitialized = true;
  }

  /**
   * Setup Tauri IPC listeners
   */
  private async setupTauriIPC(): Promise<void> {
    // Import Tauri modules dynamically to avoid issues in non-Tauri environments
    if (this.isTauri) {
      const { listen } = await import("@tauri-apps/api/event");

      // Listen for options updates from main app
      listen("options-updated", (event) => {
        this.handleOptionsUpdate(event.payload);
      });

      // Listen for security challenges (if security is enabled)
      listen("security-challenge", (event) => {
        this.handleSecurityChallenge(event.payload);
      });
    }
  }

  /**
   * Handle options update from main app
   */
  private handleOptionsUpdate(payload: any): void {
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
    if (this.isTauri) {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        // Include auth token if provided
        const options = await invoke<LiminalOptions>("get_options", {
          token: authToken,
        });
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
    if (this.isTauri) {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        // Include auth token if provided
        await invoke("set_options", {
          options,
          token: authToken,
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
    if (this.isTauri) {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        // Include auth token if provided
        const options = await invoke<LiminalOptions>("factory_reset_options", {
          token: authToken,
        });
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
    if (this.isTauri) {
      const { invoke } = await import("@tauri-apps/api/core");
      try {
        // Include auth token if provided
        await invoke("preview_screensaver", {
          token: authToken,
        });
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
      starts_in: 0.2,
      display_off_in: 1.0,
      require_pass_in: 1.0,
      run_on_battery: false,
      debug: false,
      saver_url: "https://save.screensaver.gallery",
      saver_url_debug: "https://save.screensaver.gallery/debug",
      options_url: "",
    };
  }

  /**
   * Listen for options updates
   */
  onOptionsUpdate(callback: (options: LiminalOptions) => void): () => void {
    if (typeof window !== "undefined") {
      const handler = (event: Event) => {
        callback((event as CustomEvent).detail);
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

    // Simple token generation (in production, use proper crypto)
    const timestamp = Date.now().toString();
    const nonce = Math.random().toString(36).substring(2, 15);
    const data = `${timestamp}:${nonce}:${this.securityConfig.sharedSecret}`;

    // Simple hash (replace with proper crypto in production)
    let hash = 0;
    for (let i = 0; i < data.length; i++) {
      hash = ((hash << 5) - hash + data.charCodeAt(i)) | 0;
    }

    return `${timestamp}.${nonce}.${hash.toString(16)}`;
  }
}

/**
 * Liminal Screen Options Interface
 */
export interface LiminalOptions {
  /** Time in minutes before screensaver starts */
  starts_in: number;

  /** Time in minutes before display turns off */
  display_off_in: number;

  /** Time in minutes before password is required */
  require_pass_in: number;

  /** Whether to run on battery power */
  run_on_battery: boolean;

  /** Debug mode enabled */
  debug: boolean;

  /** Main screensaver URL */
  saver_url: string;

  /** Debug screensaver URL */
  saver_url_debug: string;

  /** Options page URL */
  options_url: string;
}

/**
 * Custom error class for Liminal API errors
 */
export class LiminalAPIError extends Error {
  constructor(
    message: string,
    public cause?: any,
  ) {
    super(message);
    this.name = "LiminalAPIError";
  }
}

// Export singleton instance
export const liminalAPI = new LiminalAPI();
