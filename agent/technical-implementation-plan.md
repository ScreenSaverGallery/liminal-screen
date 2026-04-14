# Liminal Screen - Technical Implementation Plan

## Issue 1: Screensaver Monitoring Doesn't Start Until Options Window Activated

### Current Problem
The monitoring loop in `src/main.ts` only starts when `init()` is called, which happens in `DOMContentLoaded` event. Since the main window starts hidden, this event never fires until the window is shown.

### Solution Approach
We need to ensure the monitoring loop starts when the application launches, not when the window becomes visible.

### Implementation Steps

1. **Modify the application initialization to start monitoring immediately**

```typescript
// In src/main.ts, modify the window load event handlers

// Replace the current DOMContentLoaded handler with a more robust approach
window.addEventListener("DOMContentLoaded", () => {
  // Cache UI elements (only if window is actually visible)
  if (document.visibilityState === "visible") {
    cacheUIElements();
    setupUIButtonHandlers();
  }
  
  // Always initialize application logic even if window is hidden
  init();
});

// Additionally, ensure init runs even if DOMContentLoaded doesn't fire
if (document.readyState === "loading") {
  // Document is still loading, use DOMContentLoaded
  window.addEventListener("DOMContentLoaded", initAppLogic);
} else {
  // Document already loaded, initialize immediately
  initAppLogic();
}

async function initAppLogic() {
  console.log("Initializing app logic...");
  try {
    // Initialize storage first
    await Storage.init();

    // Initialize options manager
    await OptionsManager.init();

    // Load options
    await loadOptions();
    
    // Send current URLs to Rust backend
    if (options) {
      const appOptions = {
        saver_url:
          import.meta.env.VITE_SAVER_URL || "https://save.screensaver.gallery",
        saver_url_debug:
          import.meta.env.VITE_SAVER_URL_DEBUG ||
          "https://save.screensaver.gallery/debug",
        options_url: import.meta.env.VITE_OPTIONS_URL || "",
        starts_in: options.startsIn,
        display_off_in: options.displayOffIn,
        require_pass_in: options.requirePassIn || 1.0,
        run_on_battery: options.runOnBattery,
        debug: options.debug,
      };

      try {
        await invoke("set_options", { options: appOptions });
        console.log("Sent app options to Rust backend:", appOptions);
      } catch (error) {
        console.error("Failed to send options to Rust backend:", error);
      }
    }

    // Register service worker for offline support
    await registerServiceWorker();

    // Set up event listeners
    setupEventListeners();

    // Start monitoring loop (this should now happen even for hidden windows)
    startMonitoring();

    console.log("Liminal Screen - App logic initialized successfully");
  } catch (error) {
    console.error("Failed to initialize app logic:", error);
  }
}
```

2. **Ensure window JavaScript context runs even when hidden**

This may require changes in Tauri configuration to ensure the renderer process starts immediately.

## Issue 2: Preview Window Media Persistence

### Current Problem
Preview window only hides instead of closing properly, causing media to continue playing.

### Solution Approach
Implement proper close handling with media cleanup before window destruction.

### Implementation Steps

1. **Fix the onCloseRequested handler in previewScreensaver function**

```typescript
/**
 * Preview the screensaver (immediate activation)
 */
async function previewScreensaver(): Promise<void> {
  if (isScreensaverActive) {
    console.warn("Screensaver already active");
    return;
  }

  // Close existing preview window if open
  if (previewWindow) {
    try {
      // Navigate to blank first to stop media
      try {
        await invoke("navigate_webview", {
          label: "preview",
          url: "about:blank"
        });
        // Small delay for navigation
        await new Promise(resolve => setTimeout(resolve, 100));
      } catch (error) {
        console.warn("Could not navigate preview to blank:", error);
      }
      
      await previewWindow.close();
    } catch (error) {
      console.warn("Could not close existing preview window:", error);
    }
    previewWindow = null;
  }

  try {
    // Create a simple preview window like the main window
    previewWindow = new WebviewWindow("preview", {
      url: options?.debug
        ? import.meta.env.VITE_SAVER_URL_DEBUG
        : import.meta.env.VITE_SAVER_URL,
      title: "Screensaver Preview",
      width: 800,
      height: 600,
      resizable: true,
      decorations: true,
      visible: true,
      alwaysOnTop: false,
      skipTaskbar: false,
    });

    console.log("Preview window created");

    // Proper close event handling
    previewWindow.onCloseRequested(async (event) => {
      console.log("Preview window close requested");
      event.preventDefault(); // Prevent default close behavior
      
      if (previewWindow) {
        // Navigate to about:blank to stop all media playback
        try {
          await invoke("navigate_webview", {
            label: "preview",
            url: "about:blank"
          });
          // Small delay to ensure navigation completes
          await new Promise(resolve => setTimeout(resolve, 100));
        } catch (navError) {
          console.warn("Could not navigate preview to blank:", navError);
        }
        
        // Actually close the window
        try {
          await previewWindow.close();
          console.log("Preview window closed successfully");
        } catch (closeError) {
          console.warn("Error closing preview window:", closeError);
        }
        
        previewWindow = null;
      }
    });

    console.log(
      "Preview window created with proper close handling."
    );
  } catch (error) {
    console.error("Failed to create preview window:", error);
  }
}
```

## Issue 3: Create Preview Class

### Solution Approach
Encapsulate preview functionality in a dedicated class similar to Saver class.

### Implementation Steps

1. **Create new Preview class file**

```typescript
// src/app/preview/preview.ts

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
 * Preview Class - Manages preview window
 */
export class Preview {
  private webviewWindow: WebviewWindow | null = null;
  private readonly label: string = "preview";
  private readonly url: string;
  private readonly options: PreviewOptions;

  /**
   * Create a new Preview instance
   * @param url - URL to display in the preview
   * @param options - Configuration options
   */
  constructor(
    url: string,
    options?: PreviewOptions,
  ) {
    this.url = url;
    this.options = options ?? DefaultOptions;
  }

  /**
   * Create and show the preview window
   */
  async show(): Promise<void> {
    if (this.webviewWindow) {
      console.warn(`Preview window already exists`);
      // Focus existing window instead
      try {
        await this.webviewWindow.setFocus();
        await this.webviewWindow.show();
        return;
      } catch (error) {
        console.warn("Could not focus existing preview window:", error);
      }
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
        devtools: this.options.debug,
      };

      console.log(`Creating Preview WebviewWindow`, windowOptions);
      // Create the window using Tauri API
      this.webviewWindow = new WebviewWindow(this.label, windowOptions);

      console.log(`Waiting for preview window to be created...`);
      // Wait for window to be created
      await new Promise<void>((resolve, reject) => {
        let resolved = false;

        if (this.webviewWindow) {
          // Listen for window creation
          this.webviewWindow.once("tauri://created", async () => {
            console.log(`Preview window created successfully`);
            if (!resolved) {
              resolved = true;
              resolve();
            }
          });

          // Listen for window creation errors
          this.webviewWindow.once("tauri://error", (error) => {
            console.log(`Error creating preview window:`, error);
            if (!resolved) {
              resolved = true;
              reject(
                new Error(`Failed to create preview window: ${error.payload}`)
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

      // Setup close event handling
      if (this.webviewWindow) {
        this.webviewWindow.onCloseRequested(async (event) => {
          console.log("Preview window close requested");
          event.preventDefault(); // Prevent default close behavior
          
          await this.hide();
        });
      }

      console.log(`Preview window created successfully`);
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
      console.warn(`Preview window does not exist`);
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
        console.warn("Could not navigate preview to blank:", navError);
      }

      // Small delay to ensure navigation completes
      await new Promise((resolve) => setTimeout(resolve, 100));

      // Close the window
      await this.webviewWindow.close();
      this.webviewWindow = null;

      console.log(`Preview window closed`);
    } catch (error) {
      console.error(`Error hiding preview window:`, error);
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
   * Focus the preview window
   */
  async focus(): Promise<void> {
    if (this.webviewWindow) {
      await this.webviewWindow.setFocus();
    }
  }
}
```

2. **Update main.ts to use Preview class**

```typescript
// In src/main.ts, add import
import { Preview } from "./app/preview/preview";

// Update previewScreensaver function
let previewInstance: Preview | null = null;

/**
 * Preview the screensaver (immediate activation)
 */
async function previewScreensaver(): Promise<void> {
  if (isScreensaverActive) {
    console.warn("Screensaver already active");
    return;
  }

  try {
    // Get the URL for preview
    const previewUrl = options?.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG
      : import.meta.env.VITE_SAVER_URL;
      
    // Create or reuse preview instance
    if (!previewInstance) {
      previewInstance = new Preview(previewUrl, {
        debug: options?.debug || false,
      });
    }
    
    // Show the preview
    await previewInstance.show();
    
    console.log("Preview window shown using Preview class");
  } catch (error) {
    console.error("Failed to create preview window:", error);
  }
}
```

## Issue 4: Multi-Monitor Fullscreen Issue

### Current Problem
On dual monitor setups, one window doesn't properly fullscreen.

### Solution Approach
Add proper timing delays and verification for fullscreen operations.

### Implementation Steps

1. **Modify Saver.show() method with improved timing and verification**

```typescript
// In src/app/saver/saver.ts, update the window creation event handler

this.webviewWindow.once("tauri://created", async () => {
  console.log(`Window ${this.label} created successfully`);
  if (!resolved && this.webviewWindow) {
    resolved = true;

    try {
      // Add a small delay to ensure window is properly positioned before fullscreen
      await new Promise(resolve => setTimeout(resolve, 150));
      
      // Set fullscreen with error handling and verification
      try {
        await this.webviewWindow.setFullscreen(true);
        
        // Verify fullscreen was applied
        const isFullscreen = await this.webviewWindow.isFullscreen();
        if (!isFullscreen) {
          console.warn(`Window ${this.label} failed to go fullscreen, retrying...`);
          // Retry with a delay
          await new Promise(resolve => setTimeout(resolve, 50));
          await this.webviewWindow.setFullscreen(true);
          
          // Final verification
          const isReallyFullscreen = await this.webviewWindow.isFullscreen();
          if (!isReallyFullscreen) {
            console.error(`Window ${this.label} still failed to go fullscreen after retry`);
          }
        }
      } catch (fullscreenError) {
        console.error(`Error setting fullscreen for ${this.label}:`, fullscreenError);
        // Continue anyway as window is created
      }
      
      // Add another small delay before maximizing
      await new Promise(resolve => setTimeout(resolve, 50));
      
      // Maximize window (additional insurance for fullscreen)
      try {
        await this.webviewWindow.maximize();
      } catch (maximizeError) {
        console.warn(`Could not maximize window ${this.label}:`, maximizeError);
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
```

2. **Consider adding monitor-specific delays or sequential creation**

```typescript
// In src/main.ts, modify activateScreensaver to create windows sequentially

/**
 * Activate the screensaver on all displays
 */
async function activateScreensaver(): Promise<void> {
  if (isScreensaverActive) return;

  console.log("Activating screensaver...");

  try {
    console.log("activateScreensaver called, getting monitors...");
    // Prevent display sleep
    await PowerMonitor.preventDisplaySleep();

    // Get all monitors - command registered directly without namespace
    console.log("Getting available monitors...");
    const monitors = await invoke<MonitorInfo[]>("get_available_monitors");
    console.log("Got monitors:", monitors);

    if (monitors.length === 0) {
      console.warn("No monitors found");
      return;
    }

    // Create saver for each display sequentially to avoid timing issues
    const saverOptions: SaverOptions = {
      debug: options?.debug || false,
      ...remoteOptions,
    };
    console.log("Using saver options:", saverOptions);

    // Build URL with query parameters
    const baseUrl = options?.debug
      ? import.meta.env.VITE_SAVER_URL_DEBUG
      : import.meta.env.VITE_SAVER_URL;
    console.log("Base URL for saver:", baseUrl);
    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(remoteOptions)) {
      if (value !== undefined && value !== null) {
        params.append(key, String(value));
      }
    }
    const url = params.toString()
      ? `${baseUrl}?${params.toString()}`
      : baseUrl || "about:blank";

    // Create savers sequentially with delays
    for (let i = 0; i < monitors.length; i++) {
      const monitor = monitors[i];
      console.log(`Creating saver for monitor ${i}:`, monitor);
      
      const saver = new Saver(
        url,
        `saver-display-${monitor.id}`,
        { x: monitor.position.x, y: monitor.position.y },
        {
          width: monitor.size.width,
          height: monitor.size.height,
        },
        saverOptions,
      );
      
      console.log("Saver created, calling show...");
      await saver.show();
      activeSavers.push(saver);

      // Register with Rust side
      await invoke("add_active_saver", {
        label: `saver-display-${monitor.id}`,
      });
      
      // Add delay between window creations for multi-monitor setups
      if (i < monitors.length - 1) {
        await new Promise(resolve => setTimeout(resolve, 200));
      }
    }

    isScreensaverActive = true;
    console.log(`Screensaver activated on ${monitors.length} display(s)`);

    // Emit event
    await emit("screensaver-started");
  } catch (error) {
    console.error("Failed to activate screensaver:", error);
    await deactivateScreensaver();
  }
}
```

## Testing Validation Plan

### For Monitoring Initialization
1. Launch application without opening options window
2. Check console for monitoring logs
3. Wait for idle time to accumulate
4. Verify screensaver activates automatically

### For Preview Window Fix
1. Open preview window
2. Navigate to content with autoplay media
3. Close preview window using window controls
4. Verify media stops playing completely

### For Preview Class Implementation
1. Verify Preview class loads correctly
2. Check that preview window behaves consistently
3. Confirm proper cleanup on window close

### For Multi-Monitor Fix
1. Test on dual monitor setup
2. Verify both windows go fullscreen properly
3. Check window positioning accuracy
4. Confirm no timing-related issues

This comprehensive technical implementation plan provides specific code changes to address all four priority issues while maintaining application stability and user experience.