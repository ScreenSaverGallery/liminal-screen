# Liminal Screen Application Analysis

## Overview

Liminal Screen is a cross-platform screensaver application built with Tauri v2 that runs in the system tray and activates after a configurable period of system inactivity. It displays remote web content in fullscreen chromeless browser windows across all connected displays, with automatic media playback support.

## Architecture Summary

### Window Model

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

## Key Components Analysis

### 1. Power Monitor Plugin (`src-tauri/src/power_monitor.rs`)

Handles system idle time detection and power management:

- `get_system_idle_time()` - Returns system idle time in seconds
- `get_system_idle_state(threshold)` - Returns "idle" if idle time >= threshold, otherwise "active"
- `is_on_battery_power()` - Checks if system is running on battery power
- `prevent_display_sleep()` - Prevents system from sleeping while screensaver is active
- `allow_display_sleep()` - Releases the power blocker to allow normal sleep behavior
- `blank_screen()` - Turns off the display immediately
- `lock_screen()` - Locks the screen

Implementation varies by platform:
- **Windows**: Uses Win32 APIs
- **macOS**: Uses `ioreg` and `pmset` commands
- **Linux**: Uses `xprintidle` and various system commands

### 2. Display Manager Plugin (`src-tauri/src/display_manager.rs`)

Detects and provides information about all connected displays:

- `get_available_monitors()` - Returns array of MonitorInfo objects containing:
  - id: u32 (Zero-based index)
  - name: String (Display name or "Unknown")
  - position: Position {x: i32, y: i32} (x, y coordinates)
  - size: Size {width: u32, height: u32} (dimensions in pixels)
  - scale_factor: f64 (DPI scaling factor)

### 3. AutoPlay Media Plugin (`src-tauri/src/autoplay_media.rs`)

Configures webview to automatically play media without user interaction:

- Platform-specific configuration for each webview backend:
  - **Windows**: Uses WebView2 API to enable autoplay
  - **macOS**: Uses WKWebView configuration via Objective-C runtime
  - **Linux**: Uses WebKitGTK settings

## TypeScript Frontend Components

### PowerMonitor Class (`src/app/power-monitor/power-monitor.ts`)

TypeScript wrapper for Rust power monitor plugin commands with state management for power blockers.

### Saver Class (`src/app/saver/saver.ts`)

Manages individual fullscreen screensaver windows for each display:

- Creates WebviewWindow with appropriate positioning and sizing
- Sets up chromeless fullscreen windows
- Injects custom navigator properties for communication with screensaver content
- Properly stops media playback by navigating to `about:blank` before closing

### Storage System (`src/app/storage/storage.ts`)

Persistent configuration storage using Tauri Store plugin:

- Handles mandatory options (startsIn, displayOffIn, etc.)
- Stores remote form parameters
- Provides factory reset functionality

### Options Manager (`src/app/options/options.ts`)

Manages the Options configuration window with offline support via service worker.

## Key Issues Identified

### 1. Power Management Implementation Incompleteness

The power management functions in the Rust plugin are partially implemented:

- `prevent_display_sleep()` and `allow_display_sleep()` store dummy values instead of implementing platform-specific power assertion APIs
- On Windows, they should use `SetThreadExecutionState`
- On macOS, they should use IOPMAssertionCreate
- On Linux, they should use systemd-inhibit

### 2. Approach Mismatch Between Tauri v1 and v2

Some parts of the code appear to follow Tauri v1 patterns when the project is configured for Tauri v2:

- Some event names and APIs might need updating
- Plugin registration and initialization may need adaptation

### 3. Remote Options Configuration Disabled

The remote options URL is commented out in `.env` file:
```
# VITE_OPTIONS_URL=http://localhost/dev/projects/ssg/apps/tauri/ssg-tauri-liminal/options/options.html
```

This means the application currently only works with the built-in fallback UI in the main window.

### 4. Service Worker Path Issues

The service worker registration expects files at `/sw.js`, `/options.html`, etc., but the actual implementation might have different paths.

## Recommendations for Improvement

1. **Complete Power Management Implementation**:
   - Implement proper platform-specific power assertion APIs
   - Handle errors appropriately for each platform

2. **Fix Tauri v2 Compatibility Issues**:
   - Review all Tauri API calls to ensure they're v2 compatible
   - Update plugin registration patterns if needed

3. **Improve Error Handling**:
   - Add more robust error handling in Rust plugins
   - Better feedback when system APIs fail

4. **Enhance Testing**:
   - Add unit tests for each component
   - Test on all target platforms (Windows, macOS, Linux)

5. **Documentation Improvements**:
   - Expand inline documentation
   - Add more examples for API usage

This analysis provides a comprehensive overview of the Liminal Screen application and identifies key areas for improvement.