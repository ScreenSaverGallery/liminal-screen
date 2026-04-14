# Liminal Screen - Key Files and Their Purposes

## Rust Backend (src-tauri/src/)

### lib.rs
**Main Application Library**
- Integrates all plugins, system tray, and event handling
- Manages application state (screensaver active status, active savers, options)
- Sets up system tray with Options, Preview, and Quit menu items
- Implements window management for remote options and main window
- Registers all Tauri commands and plugins

### power_monitor.rs
**Power Management Plugin**
- Tracks system idle time and manages power states across platforms
- Implements platform-specific APIs for idle detection:
  - Windows: Win32 Last Input Info
  - macOS: ioreg HIDIdleTime parsing
  - Linux: xprintidle command
- Controls display sleep prevention and screen locking

### display_manager.rs
**Display Management Plugin**
- Detects connected displays and their properties
- Returns MonitorInfo with position, size, and scale factor
- Used to create correctly positioned screensaver windows

### autoplay_media.rs
**Media Autoplay Plugin**
- Configures webviews to automatically play media
- Platform-specific implementations for WebView2 (Windows), WKWebView (macOS), WebKitGTK (Linux)
- Called when each webview window is created

### main.rs
**Application Entry Point**
- Simple wrapper that calls the library's run() function

## TypeScript Frontend (src/)

### main.ts
**Main Application Entry Point**
- Handles initialization, monitoring loop, and application flow
- Manages idle state detection and screensaver activation/deactivation
- Sets up event listeners for IPC communication
- Manages the hidden main window that runs the background monitoring

### app/power-monitor/power-monitor.ts
**Power Monitor TypeScript Wrapper**
- Provides TypeScript interface to Rust power monitor commands
- Manages power blocker state (prevent/allow display sleep)

### app/saver/saver.ts
**Screensaver Window Manager**
- Creates and manages fullscreen screensaver windows for each display
- Handles window lifecycle (creation, positioning, destruction)
- Stops media playback by navigating to about:blank before closing

### app/storage/storage.ts
**Persistent Configuration Storage**
- Uses Tauri Store plugin for persistent storage
- Manages mandatory options and remote form parameters
- Provides factory reset functionality

### app/options/options.ts
**Options Window Manager**
- Handles loading remote options pages with offline support
- Manages service worker registration for offline capability
- Coordinates between remote options page and main application

### app/remote-options/remote-options.ts
**Remote Options Page Handler**
- Runs in the remote options webpage context
- Communicates with main app via Tauri IPC
- Gracefully degrades in non-Tauri environments for testing

## Configuration Files

### tauri.conf.json
**Tauri Configuration**
- Defines main window as hidden by default
- Configures CSP security policies
- Specifies build and bundle settings

### Cargo.toml
**Rust Dependencies**
- Tauri v2 dependencies and features
- Platform-specific dependencies (windows, cocoa, webkit2gtk, etc.)

### package.json
**Frontend Dependencies**
- Tauri API and plugin dependencies
- Build tools (Vite, TypeScript)

### .env
**Environment Variables**
- Screensaver URLs (SAVER_URL, SAVER_URL_DEBUG)
- Options page URL (OPTIONS_URL) - currently commented out