# Liminal Screen - System Tray Screensaver Application

## Overview

Liminal Screen is a cross-platform screensaver application built with Tauri v2 that runs in the system tray and activates after a configurable period of system inactivity. It displays remote web content in fullscreen chromeless browser windows across all connected displays, with automatic media playback support.

**Key Architecture**: The application starts **without any visible window** - it runs in the background with only a system tray icon. The main window is hidden by default and only serves as a **fallback configuration interface** if the remote options URL is not configured. This design maximizes customizability by allowing the options interface to be hosted remotely.

## Core Features

- **Background Operation**: Starts hidden with no visible window - runs entirely from system tray
- **System Tray Integration**: Runs in the background with a system tray icon providing quick access to controls
- **Remote Configuration**: Options interface can be hosted at a remote URL, enabling maximum customizability
- **Fallback UI**: Built-in main window serves as configuration interface when remote options URL is not defined
- **Idle Detection**: Monitors system inactivity using platform-specific APIs
- **Multi-Monitor Support**: Creates fullscreen windows on all connected displays with proper positioning and scaling
- **Chromeless Browser**: Displays web content without browser UI elements (decorations, borders, toolbars)
- **Auto Media Playback**: Automatically plays videos and audio without requiring user interaction
- **Power Management**: Controls display sleep and system power states
- **Cross-Platform**: Works on Windows, macOS, and Linux (both X11 and Wayland)

## Architecture

### Window Model

The application uses a **hidden window architecture**:

1. **Main Window (Hidden)**:
   - Created but not visible on startup (`visible: false` in config)
   - Runs the monitoring loop and initialization code in the background
   - Only shown as fallback if remote options URL is not configured
   - Contains built-in configuration UI for basic settings

2. **Options Window (Remote/Fallback)**:
   - **Primary**: Opens remote URL defined in `VITE_OPTIONS_URL` environment variable
   - **Fallback**: Shows main window if no remote URL is configured
   - Handles all user configuration through web-based interface

3. **Screensaver Windows (On Demand)**:
   - Created when idle threshold is reached
   - One fullscreen window per connected display
   - Destroyed when user activity is detected

### Process Model

The application follows Tauri's multi-process architecture:

1. **Core Process (Rust)**: Handles system-level operations
   - System tray management and event handling
   - Power monitoring and idle detection via platform-specific APIs
   - Display detection for multi-monitor setups
   - Media autoplay policy enforcement
   - Application lifecycle and window management
   - Routes "Options" to remote URL or fallback main window

2. **Renderer Process (TypeScript/JavaScript)**: Runs in hidden main window
   - Screensaver window creation and positioning
   - Idle state monitoring loop (runs continuously in background)
   - Options UI and persistent storage
   - Inter-window communication via Tauri events

### Custom Tauri Plugins

The application uses three custom Rust plugins that must be implemented:

#### 1. Power Monitor Plugin (`src-tauri/src/power_monitor.rs`)

**Purpose**: Tracks system idle time and manages power states across platforms.

**Key Functions**:

- `get_system_idle_time() -> Result<u64, String>`
  - Returns system idle time in seconds
  - **Windows**: Uses `GetLastInputInfo` API from Win32
  - **macOS**: Parses `HIDIdleTime` from `ioreg -c IOHIDSystem` output (nanoseconds to seconds conversion)
  - **Linux**: Tries `xprintidle` command first (milliseconds to seconds), falls back to `/proc` statistics
  - Must work without requiring root/admin privileges

- `get_system_idle_state(threshold: u64) -> Result<String, String>`
  - Returns "idle" if idle time >= threshold, otherwise "active"

- `is_on_battery_power() -> Result<bool, String>`
  - **Windows**: Checks `SYSTEM_POWER_STATUS.ACLineStatus` (0 = battery, 1 = AC)
  - **macOS**: Uses `pmset -g ps` command, checks for "Battery Power" string
  - **Linux**: Reads `/sys/class/power_supply/AC*/online` or `/sys/class/power_supply/ADP*/online`

- `prevent_display_sleep() -> Result<u32, String>`
  - Prevents system from sleeping while screensaver is active
  - Returns a blocker ID for later release
  - **Windows**: Would use `SetThreadExecutionState` (currently stores dummy value)
  - **macOS**: Would use IOPMAssertionCreate (currently stores dummy value)
  - **Linux**: Would use systemd-inhibit (currently stores dummy value)

- `allow_display_sleep(blocker_id: u32) -> Result<(), String>`
  - Releases the power blocker to allow normal sleep behavior

- `blank_screen() -> Result<(), String>`
  - Turns off the display immediately
  - **Windows**: PowerShell command with `SendMessage` to set `SC_MONITORPOWER` to 2
  - **macOS**: `pmset displaysleepnow` command
  - **Linux**: Tries `xset dpms force off`, `gnome-screensaver-command -a`, or `xdg-screensaver activate`

- `lock_screen() -> Result<(), String>`
  - **Windows**: `rundll32.exe user32.dll,LockWorkStation`
  - **macOS**: `pmset displaysleepnow` (locks when display wakes)
  - **Linux**: Tries `loginctl lock-session`, `gnome-screensaver-command -l`, `xdg-screensaver lock`

**State Management**:
- Uses `PowerSaveBlocker` struct with `Arc<Mutex<Option<...>>>` for thread-safe blocker tracking
- Different types for each platform (u32 for Windows/macOS, String for Linux)

**Plugin Registration**:
```rust
pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("power-monitor")
        .setup(|app, _api| {
            app.manage(PowerSaveBlocker::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_system_idle_time,
            get_system_idle_state,
            is_on_battery_power,
            lock_screen,
            blank_screen,
            prevent_display_sleep,
            allow_display_sleep,
        ])
        .build()
}
```

#### 2. Display Manager Plugin (`src-tauri/src/display_manager.rs`)

**Purpose**: Detects and provides information about all connected displays.

**Data Structures**:
```rust
#[derive(serde::Serialize, Clone)]
pub struct MonitorInfo {
    pub id: u32,              // Zero-based index
    pub name: String,         // Display name or "Unknown"
    pub position: Position,   // x, y coordinates
    pub size: Size,           // width, height in pixels
    pub scale_factor: f64,    // DPI scaling factor (1.0 = 100%, 2.0 = 200%)
}

#[derive(serde::Serialize, Clone)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(serde::Serialize, Clone)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}
```

**Key Function**:
- `get_available_monitors<R: Runtime>(app: AppHandle<R>) -> Result<Vec<MonitorInfo>, String>`
  - Uses Tauri's `app.available_monitors()` to get all displays
  - Enumerates monitors and extracts position, size, and scale factor
  - Position represents the monitor's top-left corner in virtual screen space
  - Scale factor is needed to calculate actual window size (physical_size / scale_factor)

**Usage Example**:
```typescript
const monitors = await invoke<MonitorInfo[]>("get_available_monitors");
// Returns: [{ id: 0, name: "DELL U2718Q", position: { x: 0, y: 0 }, size: { width: 3840, height: 2160 }, scale_factor: 2.0 }]
```

#### 3. AutoPlay Media Plugin (`src-tauri/src/autoplay_media.rs`)

**Purpose**: Configures webview to automatically play media without user interaction.

**Implementation Approach**:

- Uses Tauri's `on_webview_ready` hook to configure each window as it's created
- Platform-specific configuration for each webview backend

**Windows Implementation**:
```rust
// Uses WebView2 API
window.with_webview(|webview| {
    unsafe {
        if let Some(controller) = webview.controller() {
            let core_webview = controller.CoreWebView2().unwrap();
            let settings = core_webview.Settings().unwrap();
            settings.SetIsScriptEnabled(true).ok();
            settings.SetAreDefaultScriptDialogsEnabled(true).ok();
        }
    }
});
```

**macOS Implementation**:
```rust
// Uses WKWebView configuration via Objective-C runtime
window.with_webview(|webview| {
    unsafe {
        let wkwebview: id = webview.inner() as *mut _ as id;
        let config: id = msg_send![wkwebview, configuration];
        
        // Set mediaTypesRequiringUserActionForPlayback to 0 (none)
        // This allows autoplay for both audio and video
        let _: () = msg_send![config, setMediaTypesRequiringUserActionForPlayback: 0];
        
        // Enable JavaScript automation
        let preferences: id = msg_send![config, preferences];
        let _: () = msg_send![preferences, setJavaScriptCanOpenWindowsAutomatically: true];
    }
});
```

**Linux Implementation**:
```rust
// Uses WebKitGTK settings
window.with_webview(|webview| {
    unsafe {
        let webview = &*(webview.inner() as *mut webkit2gtk::WebView);
        if let Some(settings) = webview.settings() {
            settings.set_enable_media_stream(true);
            settings.set_enable_mediasource(true);
            settings.set_property("media-playback-requires-user-gesture", &false);
        }
    }
});
```

**Plugin Registration**:
```rust
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("autoplay")
        .on_webview_ready(|window| {
            configure_autoplay(window.clone());
        })
        .build()
}
```

## Configuration System

### Mandatory Options (Application-Level)

These options are required for the application to function and are stored in persistent local storage:

```typescript
interface MandatoryOptions {
  startsIn: number;      // Minutes of inactivity before screensaver activates
  displayOffIn: number;  // Minutes before display turns off
  requirePassIn: number; // Minutes after which password is required (0 = no password)
  runOnBattery: boolean; // Whether to run screensaver on battery power
  debug: boolean;        // Enable debug mode (keeps windows visible, shows devtools)
}
```

**Default Values**:
```typescript
{
  startsIn: 0.2,         // 12 seconds for testing
  displayOffIn: 1,       // 1 minute
  requirePassIn: 1,      // 1 minute
  runOnBattery: false,   // Don't run on battery by default
  debug: false           // Production mode
}
```

### Remote Options (Options Window Form)

The Options window is a remote web page that can define additional parameters. These parameters are:

1. **Stored** in persistent storage when the form is submitted
2. **Passed** as query parameters (or custom navigator properties) when opening the screensaver window

**Form Submission Process**:
1. User fills out the form on the remote Options page
2. Form submit triggers `save-options` event with form data
3. Main process stores form data in persistent storage (Tauri Store plugin)
4. When screensaver activates, stored parameters are:
   - Added as query parameters to the screensaver URL: `https://example.com/screensaver?param1=value1&param2=value2`
   - OR injected as custom navigator properties via JavaScript evaluation

**Storage Implementation** (`src/app/storage/storage.ts`):
- Uses `@tauri-apps/plugin-store` for persistent storage
- Stores both mandatory options and remote form parameters
- Provides `options` getter that merges all configuration
- `factoryReset()` clears all stored options to defaults

## Options Window Implementation

### Service Worker for Offline Support

**File**: `src/app/options/sw.js`

The Options window must work offline using a service worker that caches:
- The Options HTML page
- Associated JavaScript and CSS
- Any assets needed for the form

**Service Worker Implementation**:
```javascript
const CACHE_NAME = 'options-cache-v1';
const urlsToCache = ['/options.html', '/options.js', '/options.css'];

// Install event - cache assets
self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_NAME).then((cache) => {
      return cache.addAll(urlsToCache);
    })
  );
});

// Fetch event - network-first strategy with cache fallback
self.addEventListener('fetch', (event) => {
  if (event.request.url.includes('/options')) {
    event.respondWith(
      fetch(event.request).catch(() => caches.match(event.request))
    );
  }
});
```

### Options Window Loading

**URL Configuration**:
```env
VITE_OPTIONS_URL=https://your-options-site.com/options.html
```

**Loading Strategy**:
1. Try to load from remote URL first
2. If offline or network error, serve from service worker cache
3. If cache miss, show error message with retry button

## PowerMonitor Class

**File**: `src/app/power-monitor/power-monitor.ts`

**Purpose**: TypeScript wrapper for Rust power monitor plugin commands.

**Class Structure**:
```typescript
export class PowerMonitor {
  private static blockerId: number | null = null;

  // Get system idle time in seconds
  static async getSystemIdleTime(): Promise<number> {
    return await invoke("get_system_idle_time");
  }

  // Get system idle state ('idle' or 'active')
  static async getSystemIdleState(threshold: number): Promise<string> {
    return await invoke("get_system_idle_state", { threshold });
  }

  // Check if running on battery
  static async isOnBatteryPower(): Promise<boolean> {
    return await invoke("is_on_battery_power");
  }

  // Prevent display sleep
  static async preventDisplaySleep(): Promise<void> {
    if (this.blockerId !== null) {
      console.warn("Display sleep already prevented");
      return;
    }
    this.blockerId = await invoke<number>("prevent_display_sleep");
  }

  // Allow display sleep
  static async allowDisplaySleep(): Promise<void> {
    if (this.blockerId === null) {
      console.warn("Display sleep not currently prevented");
      return;
    }
    await invoke("allow_display_sleep", { blocker_id: this.blockerId });
    this.blockerId = null;
  }

  // Blank/turn off screen
  static async blankScreen(): Promise<void> {
    return await invoke("blank_screen");
  }

  // Lock screen
  static async lockScreen(): Promise<void> {
    return await invoke("lock_screen");
  }
}
```

**Usage Pattern**:
```typescript
// Check idle time
const idleTime = await PowerMonitor.getSystemIdleTime();
console.log(`System idle for ${idleTime} seconds`);

// Prevent sleep during screensaver
await PowerMonitor.preventDisplaySleep();

// After screensaver closes
await PowerMonitor.allowDisplaySleep();
```

## Saver Class

**File**: `src/app/saver/saver.ts`

**Purpose**: Manages individual fullscreen screensaver windows for each display.

**Class Structure**:
```typescript
export class Saver {
  private webviewWindow: WebviewWindow | null = null;
  private readonly label: string;              // Window identifier
  private readonly url: string;                // Screensaver content URL
  private readonly monitorPosition?: { x: number; y: number };  // Monitor top-left position
  private readonly monitorSize?: { width: number; height: number }; // Window dimensions
  private readonly options: SaverOptions;       // Configuration options

  constructor(
    url: string,
    label?: string,
    monitorPosition?: { x: number; y: number },
    monitorSize?: { width: number; height: number },
    options?: SaverOptions
  ) { /* ... */ }
}
```

**Window Creation Process** (`show()` method):

1. **Create Window Options**:
   ```typescript
   const windowOptions = {
     url: this.url,
     userAgent: `${navigator.userAgent} LiminalSaver/${version}`,
     focus: true,
     resizable: false,
     decorations: false,    // No title bar, borders
     transparent: false,
     visible: true,
     alwaysOnTop: true,     // Stay above other windows
     skipTaskbar: true,     // Don't show in taskbar
     title: "saver",
     backgroundColor: "#000000",
     devtools: options.debug,
     x: monitorPosition.x,
     y: monitorPosition.y,
     width: monitorSize.width / scale_factor,  // Adjust for DPI
     height: monitorSize.height / scale_factor
   };
   ```

2. **Create WebviewWindow**:
   ```typescript
   this.webviewWindow = new WebviewWindow(this.label, windowOptions);
   ```

3. **Wait for Creation**:
   - Listen for `tauri://created` event
   - Set up 5-second timeout
   - Handle `tauri://error` event

4. **Configure Window**:
   ```typescript
   // Set fullscreen and maximize
   await webviewWindow.setFullscreen(true);
   await webviewWindow.maximize();
   
   // Inject custom navigator properties
   await this.setupCustomNavigator();
   ```

**Custom Navigator Setup**:
- Injects SaverOptions as navigator properties for the screensaver page
- Example: `window.navigator.muted = true`
- Uses `evaluate_javascript` Rust command

**Window Cleanup** (`hide()` method):

1. **Stop Media Playback**:
   ```typescript
   await invoke("navigate_webview", {
     label: this.label,
     url: "about:blank"
   });
   ```
   - Navigating to `about:blank` stops all media

2. **Delay**:
   ```typescript
   await new Promise(resolve => setTimeout(resolve, 100));
   ```
   - 100ms delay ensures navigation completes

3. **Hide and Close**:
   ```typescript
   await webviewWindow.hide();
   await webviewWindow.close();
   this.webviewWindow = null;
   ```

**IPC Communication**:
- `emit(event, payload)`: Send event to this specific window
- `listen(event, handler)`: Listen for events from this window
- `Saver.emitToAll(event, payload)`: Broadcast to all saver windows

**Static Methods**:
- `closeAll()`: Closes all saver windows by navigating to `about:blank`, hiding, then closing

## Communication System

### Inter-Window Event Bus

The application uses Tauri's event system for communication between:

1. **System Tray → Main Window**
   - Events: `show-window`, `hide-window`, `quit-app`

2. **Options Window ↔ Main Process**
   - `save-options`: Save form parameters to storage
   - `request-current-options`: Get current options from main process
   - `current-options`: Response with current options
   - `reset-options`: Reset to default values
   - `options-updated`: Broadcast when options change

3. **Main Process ↔ Screensaver Windows**
   - `screensaver-started`: Notify window it's active
   - `screensaver-ending`: Warn window before close
   - Custom events for media control

### Communication Channel API

**Common Operations**:

```typescript
// Preview Screensaver
await emit('preview-screensaver', { url: 'https://...' });

// Reset to Defaults
await emit('reset-options');

// Open Link in Browser
await emit('open-external-link', { url: 'https://...' });

// Save Options (form submit)
await emit('save-options', formData);
```

**Event Listeners**:
```typescript
// Listen for options updates
await listen('options-updated', (event) => {
  console.log('Options changed:', event.payload);
});

// Listen for preview request
await listen('preview-screensaver', async (event) => {
  const { url } = event.payload;
  // Open temporary fullscreen window with URL
});
```

### System Tray Menu Actions

The tray icon provides these menu items:

1. **Options**: Open the Options configuration window
   - If `VITE_OPTIONS_URL` is configured: Opens remote options page in new window
   - If not configured: Shows the hidden main window as fallback
2. **Preview**: Preview the screensaver immediately
3. **Quit**: Exit the application completely

**Note**: "Show Window" and "Hide Window" are intentionally omitted since the app runs in the background by design.

**Implementation** (Rust side in `lib.rs`):
- Use `TrayIconBuilder` to create tray
- `MenuItem::with_id()` for each menu item
- `on_menu_event()` handler for menu clicks
- "Options" checks if remote URL is configured and routes accordingly
- Left-click on tray icon opens Options (same as menu item)

**Implementation** (TypeScript side in `main.ts`):
- Listen for events from tray menu actions
- Handle preview, options, and quit events
- Main window remains hidden unless opened as fallback

## Application Flow

### Initialization Sequence

1. **App Starts (Hidden)**:
   - Rust `setup()` creates tray icon
   - Main window is created with `visible: false`
   - Main window loads `main.ts` in background

2. **Background Process Starts**:
   - Initialize storage and load mandatory options
   - System tray is already created
   - Setup event listeners
   - **No window is shown** - app runs from tray

3. **Monitoring Loop (Background)**:
   ```typescript
   setInterval(async () => {
     const idleTime = await PowerMonitor.getSystemIdleTime();
     
     if (idleTime >= startsIn * 60 && !isScreensaverActive) {
       await createSavers();
     } else if (idleTime < startsIn * 60 && isScreensaverActive) {
       await cleanupSavers();
     } else if (idleTime >= displayOffIn * 60 && isScreensaverActive) {
       await PowerMonitor.blankScreen();
     }
   }, 1000); // Check every second
   ```

4. **User Interaction via Tray**:
   - Click "Options" → Opens remote URL (if configured) or shows main window (fallback)
   - Click "Preview" → Immediately activates screensaver
   - Click "Quit" → Exits application

### Screensaver Activation Flow

1. **Idle Threshold Reached**:
   - Call `PowerMonitor.preventDisplaySleep()` to block screen sleep
   - Get all monitors via `invoke("get_available_monitors")`
   
2. **Create Windows for Each Display**:
   ```typescript
   for (const display of monitors) {
     const saver = new Saver(
       options.debug ? debugUrl : saverUrl,
       `saver-display-${display.id}`,
       { x: display.position.x, y: display.position.y },
       { 
         width: display.size.width / display.scale_factor,
         height: display.size.height / display.scale_factor
       },
       options
     );
     await saver.show();
     activeSavers.push(saver);
   }
   ```

3. **Mark as Active**:
   ```typescript
   isScreensaverActive = true;
   ```

### Screensaver Deactivation Flow

1. **User Activity Detected** (idle time < threshold):
   - Call `cleanupSavers()` for all active windows

2. **Cleanup Each Window**:
   ```typescript
   for (const saver of activeSavers) {
     await saver.hide(); // Navigates to about:blank, stops media
   }
   ```

3. **Allow Sleep**:
   ```typescript
   await PowerMonitor.allowDisplaySleep();
   isScreensaverActive = false;
   ```

### Options Change Flow

1. **User Submits Options Form**:
   - Form data collected from remote Options page
   - `save-options` event emitted with form data

2. **Main Process Saves**:
   ```typescript
   await listen('save-options', async (event) => {
     const formData = event.payload;
     // Store in persistent storage
     await storage.set('remoteOptions', formData);
     // Update running configuration
     await emit('options-updated', formData);
   });
   ```

3. **Apply to Screensaver**:
   - Next screensaver activation uses new parameters
   - Parameters added to URL as query string or navigator properties

## Environment Configuration

Create `.env` file with:

```env
VITE_SAVER_URL=https://your-screensaver-content.com/content
VITE_SAVER_URL_DEBUG=https://your-screensaver-content.com/debug
VITE_OPTIONS_URL=https://your-options-site.com/options
```

**URL Requirements**:
- `VITE_SAVER_URL`: Main screensaver content (must support autoplay media)
- `VITE_SAVER_URL_DEBUG`: Debug version with development tools
- `VITE_OPTIONS_URL`: Options configuration form (optional)
  - If configured: Opens in separate window when user clicks "Options"
  - If NOT configured: Main window is shown as fallback configuration UI
  - Should work offline via service worker for reliability

## Platform-Specific Considerations

### Windows
- Requires Visual Studio Build Tools for Rust compilation
- WebView2 runtime must be installed (usually bundled)
- Power monitoring uses Win32 API
- Autoplay configured via WebView2 settings

### macOS
- Requires Xcode Command Line Tools
- Uses WKWebView (WebKit)
- Power monitoring via `ioreg` and `pmset` commands
- Autoplay configured via Objective-C runtime

### Linux
- Requires `webkit2gtk` development packages
- Supports both X11 and Wayland
- Power monitoring via `xprintidle` (X11) or systemd
- Multiple lock screen command compatibility

## Key Implementation Files

**Rust Backend**:
- `src-tauri/src/lib.rs` - Application entry, tray setup, window management
- `src-tauri/src/power_monitor.rs` - Idle detection and power management
- `src-tauri/src/display_manager.rs` - Multi-monitor detection
- `src-tauri/src/autoplay_media.rs` - Media autoplay configuration

**TypeScript Frontend**:
- `src/main.ts` - Application initialization, monitoring loop, event handling
- `src/app/saver/saver.ts` - Screensaver window management
- `src/app/power-monitor/power-monitor.ts` - Power API wrapper
- `src/app/storage/storage.ts` - Persistent configuration storage
- `src/app/options/options.ts` - Options window management
- `src/app/options/sw.js` - Service worker for offline support

**Configuration**:
- `src-tauri/tauri.conf.json` - Tauri configuration (window, tray, security)
- `src-tauri/Cargo.toml` - Rust dependencies and features
- `.env` - Environment variables for URLs

## Debugging

Enable debug mode in mandatory options:
- `debug: true`
- Keeps windows visible
- Shows developer tools
- Logs detailed event information
- Prevents auto-closing of screensaver windows

**Console Access**:
- Main window devtools accessible via tray or config
- Saver windows devtools accessible when debug mode enabled
- All events logged to console with timestamps

## Security Considerations

- Content Security Policy should be configured for all windows
- Screensaver URL must be HTTPS in production
- Options page should validate form inputs
- Power management blockers must be properly released
- Window close events should prevent data loss

## Summary for Agent Implementation

1. **Create three Rust plugins**: power_monitor, display_manager, autoplay_media
2. **Implement PowerMonitor TypeScript class**: Wrapper for Rust commands
3. **Implement Saver TypeScript class**: Window creation and lifecycle
4. **Create monitoring loop**: Check idle time every second, activate/deactivate (runs in hidden window)
5. **Setup system tray**: Menu with Options, Preview, Quit (no Show/Hide Window)
6. **Implement Options routing**: Check if remote URL configured, open remote or show main window
7. **Configure hidden main window**: Set `visible: false` in tauri.conf.json
8. **Implement communication bus**: Events for save-options, preview, reset
9. **Create Options window loader**: Load remote URL with service worker fallback
10. **Handle mandatory options**: Storage, defaults, and runtime application
11. **Test multi-monitor**: Verify positioning, scaling, and fullscreen on all displays
12. **Test media autoplay**: Ensure video/audio plays automatically on all platforms
13. **Test background operation**: Verify app starts hidden and monitoring works without visible window
