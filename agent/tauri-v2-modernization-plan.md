# Tauri v2 Modernization Plan for Liminal Screen

## Current State Analysis

Based on the analysis of the codebase, the Liminal Screen application is configured for Tauri v2 but implements several patterns that reflect older Tauri v1 approaches or don't leverage the latest v2 improvements. This creates a mismatch that impacts code maintainability, performance, and adherence to current best practices.

## Issues Identified

### 1. Event Handling - emitTo Usage (src/app/saver/saver.ts)

**Current Implementation:**
```typescript
import { emit, emitTo } from "@tauri-apps/api/event";

// In emit method:
async emit(event: string, payload?: unknown): Promise<void> {
  if (!this.webviewWindow) {
    console.warn(`Cannot emit to closed window ${this.label}`);
    return;
  }

  try {
    await emitTo(this.label, event, payload);  // Legacy v1 approach
  } catch (error) {
    console.error(`Failed to emit event to ${this.label}:`, error);
  }
}
```

**Issue:** Using `emitTo` function is a legacy v1 pattern. In Tauri v2, events should be emitted directly on window instances.

**Modern v2 Solution:**
```typescript
async emit(event: string, payload?: unknown): Promise<void> {
  if (!this.webviewWindow) {
    console.warn(`Cannot emit to closed window ${this.label}`);
    return;
  }

  try {
    await this.webviewWindow.emit(event, payload);  // Direct window emission
  } catch (error) {
    console.error(`Failed to emit event to ${this.label}:`, error);
  }
}
```

### 2. Redundant Event Imports

**Issue:** The `emit` function is imported but not effectively used in saver.ts for targeted emissions.

**Solution:** Remove unused imports and simplify the event emission approach.

### 3. Command Registration Patterns (src-tauri/src/lib.rs)

**Current Implementation:**
Extensive use of `tauri::generate_handler!` macro with individual functions rather than grouped command modules.

**Modern v2 Approach:**
Group related commands into modules and use Rust module organization for cleaner separation:

```rust
mod commands {
    mod power {
        use tauri::State;
        
        #[tauri::command]
        pub async fn get_system_idle_time() -> Result<u64, String> {
            // ...
        }
        
        // Other power-related commands grouped together
    }
    
    mod screensaver {
        // Screensaver-specific commands
    }
}

// Register grouped commands
.invoke_handler(tauri::generate_handler![
    commands::power::get_system_idle_time,
    commands::power::get_system_idle_state,
    // ...
])
```

### 4. Window Management Improvements

**Current Issue:** Window lifecycle management uses sequential calls that could be optimized.

**Modern v2 Enhancement:**
Use builder patterns and atomic operations where possible:

```typescript
// Instead of sequential calls
this.webviewWindow = new WebviewWindow(this.label, windowOptions);
await this.webviewWindow.setFullscreen(true);
await this.webviewWindow.maximize();

// Consider configuration options in windowOptions itself
const windowOptions = {
  // ...existing options...
  maximized: true,
  fullscreen: true,
};
this.webviewWindow = new WebviewWindow(this.label, windowOptions);
```

### 5. Store Plugin Usage Optimization

**Current Implementation:**
Basic usage of `@tauri-apps/plugin-store`.

**Modern v2 Enhancement:**
Leverage newer store capabilities and better initialization:

```typescript
// Instead of plain load:
this.store = await load(STORE_FILE, { 
  autoSave: true, 
  defaults: {}, 
  onChanged: (key, value) => {
    // Handle store changes reactively
    console.log(`Store value changed: ${key}`, value);
  } 
});
```

### 6. Event Listener Patterns

**Current Implementation:**
Uses standalone `listen` and `emit` functions.

**Modern v2 Enhancement:**
Utilize more targeted event handling with better typing:

```typescript
// Instead of generic listen calls:
listen<RemoteOptions>("options-updated", (event) => {
  // ...
});

// Consider using event emitters/receivers pattern where applicable
// Or leverage newer Tauri event filtering capabilities
```

## Step-by-Step Modernization

### Step 1: Update Event Emission in Saver Class

**File:** src/app/saver/saver.ts

**Changes Required:**
1. Remove `emitTo` import
2. Update emit method to use `this.webviewWindow.emit()`
3. Simplify imports

**Before:**
```typescript
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { emit, emitTo } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";

// ...

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
```

**After:**
```typescript
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";

// ...

async emit(event: string, payload?: unknown): Promise<void> {
  if (!this.webviewWindow) {
    console.warn(`Cannot emit to closed window ${this.label}`);
    return;
  }

  try {
    await this.webviewWindow.emit(event, payload);
  } catch (error) {
    console.error(`Failed to emit event to ${this.label}:`, error);
  }
}
```

### Step 2: Optimize Command Handler Organization (Rust)

**File:** src-tauri/src/lib.rs

**Changes Required:**
1. Group related commands into modules
2. Use more descriptive command names where beneficial
3. Organize handlers logically

**Example Pattern:**
```rust
mod app_commands {
    use tauri::{AppHandle, State, Runtime};
    
    mod screensaver {
        use super::*;
        
        #[tauri::command]
        pub fn is_screensaver_active(
            state: tauri::State<'_, crate::AppState>
        ) -> Result<bool, String> {
            let active = state.is_screensaver_active.lock().unwrap();
            Ok(*active)
        }
        
        #[tauri::command]
        pub fn get_active_savers(
            state: tauri::State<'_, crate::AppState>
        ) -> Result<Vec<String>, String> {
            let savers = state.active_savers.lock().unwrap();
            Ok(savers.clone())
        }
    }
    
    mod options {
        use super::*;
        
        #[tauri::command]
        pub fn get_options(state: tauri::State<'_, crate::AppState>) -> Result<crate::AppOptions, String> {
            let options = state.options.lock().unwrap();
            Ok(options.clone())
        }
        
        #[tauri::command] 
        pub fn set_options(
            state: tauri::State<'_, crate::AppState>, 
            options: crate::AppOptions
        ) -> Result<(), String> {
            let mut current = state.options.lock().unwrap();
            *current = options;
            Ok(())
        }
    }
}

// Then register with:
.invoke_handler(tauri::generate_handler![
    app_commands::screensaver::is_screensaver_active,
    app_commands::screensaver::get_active_savers,
    app_commands::options::get_options,
    app_commands::options::set_options,
    // ... other grouped commands
])
```

### Step 3: Enhance Window Creation Patterns

**File:** src/app/saver/saver.ts

**Changes Required:**
1. Optimize window creation by minimizing sequential API calls
2. Consider batching configuration when possible

**Enhanced Pattern:**
```typescript
async show(): Promise<void> {
  if (this.webviewWindow) {
    console.warn(`Saver window ${this.label} already exists`);
    return;
  }

  try {
    // More comprehensive window options
    const windowOptions = {
      url: this.url,
      userAgent: `${navigator.userAgent} LiminalSaver/${await getVersion()}`,
      focus: true,
      resizable: false,
      decorations: false,
      transparent: false,
      visible: true,
      alwaysOnTop: true,
      skipTaskbar: true,
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

    // Create window with comprehensive options
    this.webviewWindow = new WebviewWindow(this.label, windowOptions);

    // Only additional configuration if needed beyond initial options
    await new Promise<void>((resolve, reject) => {
      let resolved = false;
      
      if (this.webviewWindow) {
        // Listen for window creation  
        this.webviewWindow.once("tauri://created", async () => {
          if (!resolved && this.webviewWindow) {
            resolved = true;
            
            try {
              // Additional setup if needed
              await this.setupCustomNavigator();
              resolve();
            } catch (error) {
              console.error(`Error configuring saver window ${this.label}:`, error);
              resolve(); // Still resolve as window is created
            }
          }
        });
        
        // Error handling
        this.webviewWindow.once("tauri://error", (error) => {
          if (!resolved) {
            resolved = true;
            reject(new Error(`Failed to create saver window: ${error.payload}`));
          }
        });
      }
      
      // Timeout handling
      setTimeout(() => {
        if (!resolved) {
          resolved = true;
          reject(new Error("Timeout while creating saver window"));
        }
      }, 5000);
    });
  } catch (error) {
    console.error("Error creating saver window:", error);
    throw error;
  }
}
```

### Step 4: Enhance Store Usage with Reactive Capabilities

**File:** src/app/storage/storage.ts

**Enhanced Pattern:**
```typescript
static async init(): Promise<void> {
  if (this.initialized) return;

  // Enhanced store initialization with reactive capabilities
  this.store = await load(STORE_FILE, { 
    autoSave: true, 
    defaults: {},
    onChanged: (key, value) => {
      // Optional: Handle changes as they happen
      console.log(`Storage key "${key}" changed to:`, value);
    }
  });
  
  this.initialized = true;
  await this.setDefaults();
  console.log("Storage initialized with enhanced capabilities");
}
```

## Benefits of Modernization

1. **Improved Maintainability:** Grouped commands and optimized event handling make the codebase easier to understand and modify.

2. **Better Performance:** Reduced complexity in event emissions and window management leads to more efficient operations.

3. **Enhanced Type Safety:** Modern patterns generally provide better TypeScript support and compile-time checking.

4. **Future Compatibility:** Aligning with v2 patterns ensures better forward compatibility with future Tauri updates.

5. **Cleaner Architecture:** Organized command modules and optimized event handling create a more professional codebase.

## Risk Mitigation

1. **Gradual Implementation:** Apply changes incrementally to minimize risk of breaking existing functionality.

2. **Comprehensive Testing:** Verify all user flows work correctly after each change.

3. **Backward Compatibility:** Ensure changes don't break existing plugin interfaces or IPC contracts.

4. **Documentation Updates:** Update comments and documentation to reflect the new patterns.

## Testing Approach

1. **Unit Tests:** Verify each updated component individually.
2. **Integration Tests:** Ensure event handling and command interfaces work seamlessly.
3. **End-to-End Tests:** Test the complete user flow including screensaver activation/deactivation.
4. **Cross-Platform Testing:** Verify the updates work on all supported platforms (Windows, macOS, Linux).

## Conclusion

The Tauri v2 modernization primarily focuses on eliminating legacy v1 patterns that remain in the codebase despite using v2 dependencies. The most impactful immediate improvement is upgrading the event emission system in the Saver class to use direct window emission instead of the deprecated emitTo function, along with organizing command handlers logically for better maintainability.