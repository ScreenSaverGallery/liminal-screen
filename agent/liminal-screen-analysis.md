# Liminal Screen - System Tray Screensaver Application Analysis

## Overview

Liminal Screen is a cross-platform system tray screensaver application built with Tauri v2 that displays remote web content in fullscreen chromeless browser windows across all connected displays. The application operates in the background with no visible window when not active.

## Key Architecture Components

### 1. Hidden Window Architecture

The application uses a sophisticated hidden window architecture:

1. **Main Window (Hidden)**
   - Created but not visible on startup (`visible: false` in config)
   - Runs the monitoring loop and initialization code in the background
   - Only shown as fallback if remote options URL is not configured

2. **Options Window**
   - Primary: Opens remote URL defined in `VITE_OPTIONS_URL` environment variable
   - Fallback: Shows main window if no remote URL is configured

3. **Screensaver Windows (On Demand)**
   - Created when idle threshold is reached
   - One fullscreen window per connected display
   - Destroyed when user activity is detected

## Core Plugins

### Power Monitor Plugin (`src-tauri/src/power_monitor.rs`)

Manages system idle time detection and power state control across platforms:

- **Windows**: Uses `GetLastInputInfo` API for idle detection
- **macOS**: Parses `HIDIdleTime` from `ioreg` command output
- **Linux**: Uses `xprintidle` command or `/proc` statistics

Power management functions:
- `prevent_display_sleep()` - Prevents system from sleeping
- `allow_display_sleep()` - Allows normal sleep behavior
- `blank_screen()` - Turns off display immediately
- `lock_screen()` - Locks the system

### Display Manager Plugin (`src-tauri/src/display_manager.rs`)

Detects and manages information about all connected displays:
- Enumerates monitors with position, size, and scale factor
- Provides data for proper window positioning across multi-monitor setups

### AutoPlay Media Plugin (`src-tauri/src/autoplay_media.rs`)

Configures webviews to automatically play media without user interaction:
- **Windows**: Uses WebView2 API configuration
- **macOS**: Uses WKWebView configuration via Objective-C runtime
- **Linux**: Uses WebKitGTK settings

## Configuration System

Two-tier configuration approach:

1. **Mandatory Options** (stored locally):
   - `startsIn`: Minutes before screensaver activates
   - `displayOffIn`: Minutes before display turns off
   - `requirePassIn`: Minutes before password required
   - `runOnBattery`: Whether to run on battery power
   - `debug`: Enable debug mode

2. **Remote Options** (from Options form):
   - Stored in persistent storage when form submitted
   - Passed as query parameters to screensaver URL

## Communication Patterns

### Inter-Window Event Bus

Uses Tauri's event system for communication:
- System Tray ↔ Main Window
- Options Window ↔ Main Process
- Main Process ↔ Screensaver Windows

### System Tray Menu Actions

Provides these menu items:
- **Options**: Open remote/fallback configuration interface
- **Preview**: Immediately preview screensaver
- **Quit**: Exit application

## Implementation Improvements Made

### 1. Power Management Enhancements

Implemented proper system-level power management for Windows:
- Using `SetThreadExecutionState` to prevent/allow display sleep
- Properly storing/restoring previous execution states
- Error handling for power management functions

### 2. Platform-Specific Optimizations

Each platform implements appropriate native APIs:
- **Windows**: Win32 APIs for precise control
- **macOS**: IOKit framework for power assertions (planned enhancement)
- **Linux**: Multiple command-line utilities for compatibility

### 3. Error Handling and Logging

Added comprehensive error handling and logging:
- Detailed error messages for debugging
- Success/failure indicators for all operations
- Console logging for monitoring application behavior

## Technical Details

### Rust Backend Modules

1. `lib.rs` - Application entry point, tray setup, window management
2. `power_monitor.rs` - Idle detection and power management
3. `display_manager.rs` - Multi-monitor detection
4. `autoplay_media.rs` - Media autoplay configuration

### TypeScript Frontend Components

1. `main.ts` - Application initialization, monitoring loop
2. `src/app/saver/saver.ts` - Screensaver window management
3. `src/app/power-monitor/power-monitor.ts` - Power API wrapper
4. `src/app/storage/storage.ts` - Persistent configuration storage
5. `src/app/options/options.ts` - Options window management

## Security Considerations

- Content Security Policy configuration for all windows
- HTTPS requirement for screensaver URLs in production
- Input validation for options forms
- Proper release of power management blockers

## Debugging Capabilities

Debug mode enables:
- Visible windows for inspection
- Developer tools access
- Detailed console logging
- Prevention of auto-close behavior

This comprehensive analysis provides insight into the sophisticated architecture and implementation details of the Liminal Screen application, highlighting its cross-platform capabilities and system-level integration points.