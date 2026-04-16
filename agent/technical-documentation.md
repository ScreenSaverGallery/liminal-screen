# Liminal Screen Technical Documentation

## Architecture Overview

Liminal Screen implements a dual-layer architecture where:
- **Rust Layer**: Handles core system operations, monitoring, and window management
- **TypeScript Layer**: Manages UI presentation and user interactions

This separation ensures reliability and performance while maintaining flexibility for customization.

## Core Components

### 1. ScreensaverEngine (Rust)
Location: `src-tauri/src/screensaver_engine.rs`

#### Responsibilities:
- Continuous system idle time monitoring
- Automatic screensaver activation/deactivation
- Multi-monitor window management
- Power state control (sleep prevention)
- User activity detection

#### Key Methods:
- `start_engine()`: Initializes autonomous monitoring
- `stop_engine()`: Gracefully shuts down monitoring
- `check_idle_state()`: Determines screensaver activation state
- `create_saver_windows()`: Creates fullscreen windows on all displays
- `destroy_saver_windows()`: Cleans up screensaver windows

#### Threading Model:
```rust
// Background monitoring thread
thread::spawn(move || {
    loop {
        if engine_handle.should_stop.load(Ordering::Relaxed) {
            break;
        }
        
        // Check idle state and manage screensaver
        engine_handle.check_idle_state();
        
        // Sleep to avoid excessive CPU usage
        thread::sleep(Duration::from_millis(MONITORING_INTERVAL_MS));
    }
});
```

### 2. PowerMonitor (Rust)
Location: `src-tauri/src/power_monitor.rs`

#### Platform-Specific Implementations:
- **Windows**: Win32 API (`GetLastInputInfo`)
- **macOS**: `ioreg` command parsing
- **Linux**: `xprintidle` command with `/proc` fallback

#### Key Methods:
- `get_system_idle_time()`: Returns seconds of inactivity
- `is_on_battery_power()`: Power source detection
- `prevent_display_sleep()`: Power state management
- `allow_display_sleep()`: Release power management

### 3. DisplayManager (Rust)
Location: `src-tauri/src/display_manager.rs`

#### Monitor Information:
- Position (x, y coordinates)
- Size (width, height in pixels)
- Scale factor (DPI scaling)
- Unique identification

#### Multi-Monitor Support:
```rust
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub position: Position,
    pub size: Size,
    pub scale_factor: f64,
}
```

### 4. AutoplayMedia (Rust)
Location: `src-tauri/src/autoplay_media.rs`

#### Platform-Specific Configuration:
- **Windows**: WebView2 API settings
- **macOS**: WKWebView configuration via Objective-C
- **Linux**: WebKitGTK settings

#### Media Handling:
- Automatic video/audio playback
- JavaScript automation enablement
- User gesture requirement removal

### 5. Preview (TypeScript)
Location: `src/app/preview/preview.ts`

#### Manual Control:
- Immediate screensaver activation for testing
- Independent window management
- Preview-specific configuration

#### Lifecycle Management:
```typescript
class Preview {
  async show(): Promise<void>  // Create and show preview window
  async hide(): Promise<void>   // Close preview window
  async isOpen(): Promise<boolean>  // Check if window exists
}
```

## Communication Patterns

### Rust to TypeScript (Events)
```rust
// Emit events to JavaScript UI
tauri::emit_to_window("screensaver-started", Payload::empty());
tauri::emit_to_window("screensaver-ended", Payload::empty());
tauri::emit_to_window("options-updated", options_payload);
```

### TypeScript to Rust (Commands)
```typescript
// Invoke Rust commands from JavaScript
await invoke("set_options", { options: appOptions });
await invoke("get_screensaver_status");
await invoke("deactivate_screensaver_command");
```

### Inter-Window Communication
```typescript
// Broadcast to all screensaver windows
await emit("screensaver-ending");
await emit("screensaver-started");
```

## Configuration Management

### Mandatory Options (Persistent Storage)
```typescript
interface MandatoryOptions {
  startsIn: number;      // Minutes before activation
  displayOffIn: number;  // Minutes before display off
  requirePassIn: number; // Minutes before password required
  runOnBattery: boolean; // Battery power activation
  debug: boolean;        // Debug mode enablement
}
```

### Remote Options (Web-based Configuration)
Handled through web-based options interface with service worker caching.

## Build Process

### Rust Compilation
```bash
cd src-tauri
cargo build
```

### TypeScript Compilation
```bash
npm run build
# or
bun run build
```

### Tauri Bundle
```bash
npm run tauri build
```

## Error Handling

### Rust Error Propagation
```rust
fn critical_operation() -> Result<(), String> {
    // Operation that might fail
    operation_that_might_fail()
        .map_err(|e| format!("Critical operation failed: {}", e))?;
    
    Ok(())
}
```

### TypeScript Error Boundaries
```typescript
try {
    await invoke("critical_rust_command");
} catch (error) {
    console.error("Operation failed:", error);
    // User-friendly error handling
}
```

## Platform Compatibility

### Windows
- Visual Studio Build Tools requirement
- WebView2 runtime dependency
- Win32 API for power management

### macOS
- Xcode Command Line Tools requirement
- WKWebView (WebKit) rendering engine
- Objective-C runtime for autoplay configuration

### Linux
- webkit2gtk development packages
- X11/Wayland display server support
- Multiple command-line tool fallbacks

## Performance Considerations

### Monitoring Efficiency
- Background thread with configurable interval
- Minimal CPU usage during idle periods
- Efficient system API calls

### Memory Management
- Proper resource cleanup on window destruction
- Reference counting for shared resources
- Leak prevention in window management

### Startup Optimization
- Immediate Rust engine initialization
- Deferred JavaScript loading for hidden windows
- Asynchronous resource loading where possible

## Security Model

### Content Security
- HTTPS requirement for production URLs
- CSP configuration for web content
- Sandboxed webview environments

### System Integration
- Minimal privilege requirements
- Secure command invocation
- Protected configuration storage

## Testing Approach

### Unit Testing (Rust)
```rust
#[test]
fn test_idle_time_detection() {
    let idle_time = get_system_idle_time().unwrap();
    assert!(idle_time >= 0);
}
```

### Integration Testing (TypeScript)
```typescript
test('Preview window creation', async () => {
    const preview = new Preview(testUrl);
    await preview.show();
    expect(await preview.isOpen()).toBe(true);
    await preview.hide();
});
```

### Manual Verification
- Cross-platform behavior consistency
- Multi-monitor window positioning
- User activity detection responsiveness
- Power management integration

## Extensibility Points

### Custom Screensaver Content
- Remote URL configuration
- Local HTML file support
- Custom JavaScript integration

### Plugin Architecture
- Additional Tauri plugins for extended features
- Custom event handlers for special requirements
- Third-party service integrations

### Configuration Extensions
- Additional mandatory options
- Extended remote options schema
- Environment-specific overrides

## Maintenance Guidelines

### Dependency Updates
- Regular Tauri version upgrades
- Security patch application
- Deprecated API migration

### Code Quality
- Consistent error handling patterns
- Clear separation of concerns
- Comprehensive documentation

### Performance Monitoring
- Resource usage tracking
- Startup time optimization
- Memory leak prevention