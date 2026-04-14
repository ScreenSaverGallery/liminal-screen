# Liminal Screen Technical Specification

## System Architecture

### Overview
Liminal Screen is a system tray screensaver application that displays web content across all connected displays after a period of system inactivity. Built with Tauri v2, it combines Rust system integration with web-based interfaces.

### Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                    System Tray Process                      │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │   Power Monitor │  │ Display Manager │  │ AutoPlay    │ │
│  │     Plugin      │  │    Plugin       │  │ Media Plugin│ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
│                                                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                    Main Process                         ││
│  │  ┌─────────────┐  ┌──────────────┐  ┌────────────────┐ ││
│  │  │ Event       │  │ Configuration│  │ Communication  │ ││
│  │  │ Monitoring  │  │ Management   │  │ Handler        │ ││
│  │  │ Loop        │  │              │  │                │ ││
│  │  └─────────────┘  └──────────────┘  └────────────────┘ ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘

┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│   Options GUI   │  │  Screensaver    │  │  Screensaver    │
│   (Web-based)   │  │   Windows       │  │   Windows       │
│                 │  │ (Per Display)   │  │ (Per Display)   │
└─────────────────┘  └─────────────────┘  └─────────────────┘
```

## Core Plugins

### Power Monitor Plugin

#### Purpose
Tracks system idle time and manages power states across all supported platforms.

#### Platform Implementations

**Windows Implementation**
- Uses `GetLastInputInfo` Win32 API for idle detection
- `SetThreadExecutionState` for power management control
- Registry queries for power status information

**macOS Implementation**
- `ioreg` command parsing for idle time detection
- `pmset` command for power management operations
- Potential IOKit integration for enhanced control

**Linux Implementation**
- `xprintidle` command for X11 idle detection
- `/proc` statistics as fallback mechanism
- Various utility commands for power operations

#### Key Functions

1. `get_system_idle_time()` → u64 (seconds)
2. `get_system_idle_state(threshold)` → String ("idle"/"active")
3. `is_on_battery_power()` → bool
4. `prevent_display_sleep()` → Result<(), String>
5. `allow_display_sleep()` → Result<(), String>
6. `blank_screen()` → Result<(), String>
7. `lock_screen()` → Result<(), String>

### Display Manager Plugin

#### Purpose
Detects and provides information about all connected displays for proper window positioning.

#### Data Structures

```rust
#[derive(serde::Serialize, Clone)]
pub struct MonitorInfo {
    pub id: u32,              // Zero-based index
    pub name: String,         // Display name or "Unknown"
    pub position: Position,   // x, y coordinates
    pub size: Size,           // width, height in pixels
    pub scale_factor: f64,    // DPI scaling factor
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

### AutoPlay Media Plugin

#### Purpose
Configures webviews to automatically play media without user interaction.

#### Platform Implementations

**Windows**
- WebView2 API configuration
- Settings adjustment for script execution
- Automation permission configuration

**macOS**
- WKWebView configuration via Objective-C runtime
- Media playback policy adjustments
- JavaScript automation enabling

**Linux**
- WebKitGTK settings configuration
- Media stream enabling
- User gesture requirement disabling

## Configuration Management

### Mandatory Options (Local Storage)

```typescript
interface MandatoryOptions {
  startsIn: number;      // Minutes before screensaver activates
  displayOffIn: number;  // Minutes before display turns off
  requirePassIn: number; // Minutes before password required
  runOnBattery: boolean; // Run on battery power
  debug: boolean;        // Debug mode enabled
}
```

### Remote Options (Persistent Storage)

Form-based configuration stored in Tauri Store plugin:
- Passed as query parameters to screensaver URLs
- Available as navigator properties in screensaver context
- Updated via inter-process communication events

## Communication Protocol

### Event-Based Messaging

#### System Tray Events
- `show-window`: Display options interface
- `hide-window`: Hide options interface (unused)
- `quit-app`: Terminate application
- `preview-screensaver`: Immediate activation

#### Options Management Events
- `save-options`: Store updated configuration
- `request-current-options`: Retrieve current settings
- `reset-options`: Factory reset to defaults
- `options-updated`: Broadcast configuration changes

#### Screensaver Control Events
- `screensaver-started`: Notification of activation
- `screensaver-ending`: Warning of deactivation
- Custom media control events (implementation dependent)

## Window Management

### Window Types

1. **Main Window**
   - Always exists but hidden by default
   - Runs background monitoring loop
   - Fallback configuration interface

2. **Options Window**
   - Opens remote URL when configured
   - Falls back to main window when not configured
   - Service worker for offline capability

3. **Screensaver Windows**
   - Created per display during activation
   - Destroyed upon user activity detection
   - Chromeless full-screen presentation

### Window Properties

**Screensaver Windows Configuration:**
- `url`: Content URL (remote or local)
- `userAgent`: Custom identifier with version
- `focus`: Initially focused
- `resizable`: Fixed size
- `decorations`: None (chromeless)
- `transparent`: False
- `visible`: True when active
- `alwaysOnTop`: Above other windows
- `skipTaskbar`: Hidden from taskbar
- `title`: "saver"
- `backgroundColor`: Black (#000000)
- `devtools`: Enabled in debug mode
- Position: Per-monitor coordinates
- Size: Adjusted for DPI scaling

## Power Management State Machine

```
Idle Detection Loop:
┌─────────────┐
│   Active    │
│(User Activity)◄──────────────┐
└──────┬──────┘               │
       │                      │
       ▼                      │
┌─────────────┐    Timeout   │
│   Monitor   ├──────────────►
│   Idle Time │              │
└──────┬──────┘              │
       │                      │
       ▼                      │
┌─────────────┐    Activate  │
│   Threshold │◄─────────────┤
│   Reached   │              │
└──────┬──────┘              │
       │                      │
       ▼                      │
┌─────────────┐              │
│Screensaver  │              │
│  Active     │              │
│             │              │
└──────┬──────┘              │
       │                      │
       ▼                      │
┌─────────────┐    Activity  │
│  Deactivate │◄─────────────┤
│Screensaver  │              │
└─────────────┘              │
       │                      │
       ▼                      │
       └──────────────────────┘
```

## Security Model

### Content Security
- HTTPS required for remote URLs in production
- CSP policies configurable per window type
- Input sanitization for all user-provided data

### System Integration
- Minimal required permissions
- Explicit power management control
- Secure inter-process communication

## Performance Considerations

### Resource Usage
- Minimal background footprint
- Efficient idle detection polling
- Proper resource cleanup on deactivation
- Memory management for multiple windows

### Optimization Strategies
- Lazy initialization of components
- Event-driven rather than polling where possible
- Platform-native APIs for system integration
- Efficient serialization for inter-process communication

## Testing Approach

### Unit Testing
- Platform-specific API mocking
- Configuration validation
- Error condition handling

### Integration Testing
- Cross-platform idle detection
- Multi-monitor window placement
- Power management functionality
- Remote content loading reliability

### End-to-End Testing
- Full activation/deactivation cycles
- Configuration persistence
- Offline operation for options interface
- Recovery from system events

## Deployment Considerations

### Build Process
- Tauri v2 bundling for all platforms
- Code signing for distribution
- Automatic update mechanisms
- Platform-specific packaging

### Distribution Channels
- Direct download packages
- Package managers (Homebrew, AUR, etc.)
- App store distribution (where applicable)

### Compatibility Matrix
- Windows 10/11 with WebView2
- macOS 10.15+ with Safari support
- Linux with webkit2gtk development packages
- Multiple display configurations (extended/mirrored)

This technical specification provides a comprehensive overview of the Liminal Screen application architecture and implementation details, serving as a reference for ongoing development and maintenance activities.