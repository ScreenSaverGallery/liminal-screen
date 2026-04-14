# Liminal Screen - Issue 1 Implementation: Monitoring Initialization Fix

## Current Status
The `.once("tauri://created", ...)` pattern is confirmed as the correct Tauri v2 approach, as evidenced by its use in the existing codebase.

## Implementation Plan for Issue 1: Monitoring Initialization Fix

### Problem Statement
The screensaver monitoring loop doesn't start until the user opens the options window at least once, because the monitoring initialization is tied to the `DOMContentLoaded` event which never fires for a hidden window.

### Root Cause Analysis
1. The `init()` function is only called in `DOMContentLoaded` event handler
2. Hidden windows don't trigger `DOMContentLoaded` until they become visible
3. Monitoring loop depends on `init()` being called
4. This creates a chicken-and-egg problem where the screensaver never activates

### Solution Approach
We need to ensure that critical application logic (including monitoring initialization) runs regardless of window visibility. This can be achieved by:

1. Separating UI initialization from core application logic initialization
2. Ensuring the main JavaScript context initializes monitoring immediately
3. Using proper Tauri application lifecycle events

### Implementation Steps

#### Step 1: Modify src/main.ts to separate UI from core logic

```typescript
// Add this near the top of src/main.ts, after the existing imports
async function initializeCoreApplication(): Promise<void> {
  console.log("Liminal Screen - Initializing core application logic...");
  
  try {
    // Initialize storage first (critical for app operation)
    await Storage.init();
    console.log("Storage initialized");

    // Initialize options manager
    await OptionsManager.init();
    console.log("Options manager initialized");

    // Load options
    await loadOptions();
    console.log("Options loaded");
    
    // Send current URLs to Rust backend (important for backend synchronization)
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
        console.log("Sent app options to Rust backend");
      } catch (error) {
        console.error("Failed to send options to Rust backend:", error);
      }
    }

    // Set up critical event listeners (needed for app operation)
    setupCriticalEventListeners();
    
    // Start monitoring loop immediately (this is the key fix)
    startMonitoring();
    console.log("Monitoring started immediately");

    console.log("Core application logic initialized successfully");
  } catch (error) {
    console.error("Failed to initialize core application logic:", error);
    // Even if initialization fails, we might still want to start monitoring
    // to give the system a chance to recover
    try {
      startMonitoring();
      console.log("Monitoring started despite initialization errors");
    } catch (monitorError) {
      console.error("Failed to start monitoring:", monitorError);
    }
  }
}

function setupCriticalEventListeners(): void {
  // Listen for saver activity detected (user input in screensaver)
  listen("saver-activity-detected", async () => {
    console.log("Saver activity detected, deactivating screensaver");
    await deactivateScreensaver();
  });
  
  // Other critical listeners that don't depend on UI
  // Note: UI-dependent listeners will be set up separately
}

// Modify the DOMContentLoaded handler
window.addEventListener("DOMContentLoaded", () => {
  console.log("DOM Content Loaded - initializing UI components");
  
  // Cache UI elements (only relevant when UI is visible)
  cacheUIElements();

  // Set up UI button handlers (only relevant when UI is visible)
  setupUIButtonHandlers();
  
  // Set up UI-dependent event listeners
  setupUIEventListeners();
  
  // Load options into form (UI operation)
  loadOptionsIntoForm();
  
  console.log("UI components initialized");
});

function setupUIEventListeners(): void {
  // These listeners are only relevant when the UI is visible
  // Listen for options changes
  listen<RemoteOptions>("options-updated", (event) => {
    console.log("Options updated:", event.payload);
    remoteOptions = event.payload;
    loadOptionsIntoForm(); // Update UI
  });

  // Listen for preview request
  listen("preview-screensaver", () => {
    console.log("Preview screensaver requested");
    previewScreensaver();
  });

  // Listen for open-options event
  listen("open-options-window", () => {
    console.log("Open options window requested");
    openOptionsWindow();
  });

  // Listen for reset-options event
  listen("reset-options", () => {
    console.log("Reset options requested");
    loadOptions(); // Reload from storage
  });
}

// Ensure core application logic starts immediately
// This runs regardless of DOMContentLoaded
console.log("Liminal Screen core bootstrapping initiated");
initializeCoreApplication().catch(error => {
  console.error("Core bootstrap failed:", error);
});
```

#### Step 2: Update the window close handling to be more robust

```typescript
// In the setupEventListeners function, modify the window close handler:
getCurrentWindow().onCloseRequested((event: any) => {
  console.log("Main window close requested - preventing close, hiding instead");
  event.preventDefault();
  // Hide window instead of closing when running in tray
  getCurrentWindow().hide().catch(error => {
    console.error("Failed to hide main window:", error);
  });
});
```

#### Step 3: Add application state tracking to prevent duplicate initialization

```typescript
// Add at the top of src/main.ts
let isCoreInitialized = false;
let isUIInitialized = false;

// Modify initializeCoreApplication to prevent duplicate calls:
async function initializeCoreApplication(): Promise<void> {
  // Prevent duplicate initialization
  if (isCoreInitialized) {
    console.log("Core application already initialized");
    return;
  }
  
  console.log("Liminal Screen - Initializing core application logic...");
  isCoreInitialized = true;
  
  // ... rest of initialization logic
}

// Similarly for UI initialization, modify the DOMContentLoaded handler:
window.addEventListener("DOMContentLoaded", () => {
  if (isUIInitialized) {
    console.log("UI already initialized");
    return;
  }
  
  console.log("DOM Content Loaded - initializing UI components");
  isUIInitialized = true;
  
  // ... rest of UI initialization
});
```

### Testing Approach

1. **Launch without UI activation**:
   - Start the application
   - Don't open the options window
   - Wait and observe console logs for monitoring activity
   - Verify monitoring starts within a few seconds of launch

2. **Verify monitoring behavior**:
   - Check that `checkIdleState()` is called regularly
   - Confirm idle time detection works
   - Validate screensaver activation without manual window opening

3. **Test edge cases**:
   - Launch multiple times to ensure no duplicate initializations
   - Test with window shown/hidden sequences
   - Verify robust error handling

### Expected Outcomes

1. **Immediate Monitoring Start**: Monitoring loop begins as soon as the application launches
2. **Reliable Screensaver Activation**: Screensaver activates based on system idle time without manual intervention
3. **Proper Separation of Concerns**: UI initialization is separate from core logic
4. **Robust Error Handling**: Graceful handling of initialization errors
5. **No Duplicate Processing**: Protection against multiple initializations

### Rollback Plan

If issues occur with this change:

1. Revert to the original single `DOMContentLoaded` handler
2. Add explicit logging to diagnose initialization timing
3. Consider adding a small delay before starting monitoring as a fallback
4. Implement feature flag to toggle between old and new initialization approaches

This implementation addresses the core issue while maintaining backward compatibility and adding robustness to the initialization process.