# Liminal Screen

A cross-platform screensaver application built with Tauri v2 that runs in the system tray and activates after a configurable period of system inactivity.

## Features

- **Multi-monitor support** - Displays on all monitors with proper fullscreen handling
- **Autoplay media** - Supports video/audio content without user interaction
- **System tray integration** - Runs silently in the background
- **Configurable timing** - Customize activation delay, display off timing, and more
- **Remote options** - Load custom configuration from a web-based form
- **Cross-platform** - Works on macOS, Windows, and Linux

## For Developers: Forking and Rebranding

Liminal Screen is designed to be forked and rebranded for different organizations. Here's how to customize it:

### 1. Copy the Environment Template

```bash
cp .env.example .env
```

### 2. Edit `.env` with Your Branding

**Required changes:**

```bash
# App Identity
VITE_APP_NAME=Your App Name
VITE_APP_DESCRIPTION=Your app description here

# Screensaver URLs
VITE_SAVER_URL=https://your-domain.com/screensaver
VITE_SAVER_URL_DEBUG=https://your-domain.com/screensaver?debug=true

# Remote Options (optional)
VITE_OPTIONS_URL=https://your-domain.com/options.html
```

**Important:** The Rust backend reads these environment variables at **build time**. Make sure `.env` is sourced before building, or export them explicitly:

```bash
# Option 1: Export before building
export $(cat .env | xargs)
bun run tauri build

# Option 2: Use direnv or similar tool to auto-load .env
```

**Optional: Customize default timing values:**

```bash
VITE_DEFAULT_STARTS_IN=0.5        # Minutes before activation
VITE_DEFAULT_DISPLAY_OFF_IN=2     # Minutes before display off
VITE_DEFAULT_REQUIRE_PASS_IN=0    # Minutes until password required (0 = none)
VITE_DEFAULT_RUN_ON_BATTERY=false # Run on battery power?
VITE_DEFAULT_DEBUG=false          # Enable debug mode?
```

### 3. Edit `src-tauri/tauri.conf.json`

**Critical: Change the bundle identifier to avoid conflicts with other forks:**

```json
{
  "productName": "Your App Name",
  "identifier": "com.yourcompany.your-app-name",
  "app": {
    "windows": [
      {
        "title": "Your App Name"
      }
    ]
  },
  "bundle": {
    "shortDescription": "Your app description",
    "longDescription": "Full description of your screensaver application"
  }
}
```

**Why bundle identifier matters:** If two apps have the same identifier on one system, they'll share preferences, keychain entries, and may crash each other. Each fork MUST use a unique identifier.

### 4. Edit `package.json` (Optional)

Update the package name for your fork:

```json
{
  "name": "your-app-name",
  "version": "1.0.0"
}
```

### 5. Build

```bash
# Install dependencies
bun install

# Build for production (make sure .env is loaded first!)
export $(cat .env | xargs)
bun run tauri build
```

## Configuration Behavior

### Persistent Storage

User preferences (timing values like `startsIn`, `displayOffIn`, etc.) are saved to `options.json` in the app's data directory. These persist across app restarts and updates.

**Priority order:**
1. **User-saved values** from `options.json` (highest priority)
2. **`.env` defaults** (used on first install or after factory reset)
3. **Hardcoded fallbacks** (if `.env` values aren't set)

**What's persisted:** Timing values (`startsIn`, `displayOffIn`, `requirePassIn`), `runOnBattery`, `debug`

**What's NOT persisted:** URLs (`saver_url`, `saver_url_debug`, `options_url`) — these always come from `.env` so forks can update URLs without affecting user preferences.

### Factory Reset

Users can reset to `.env` defaults via the UI (Reset button) or by deleting `options.json` from the app's data directory.

# Development mode
bun run tauri dev

# Production build
bun run tauri build
```

## Architecture

### Frontend (`src/`)

- `main.ts` - Application entry point, initialization
- `app/storage/storage.ts` - Persistent configuration storage
- `app/options/options.ts` - Options window management
- `app/power-monitor/` - System idle time detection
- `app/preview/` - Preview window for testing screensaver

### Backend (`src-tauri/src/`)

- `screensaver_engine.rs` - Core screensaver logic, multi-monitor window management
- `display_manager.rs` - Monitor detection and positioning
- `power_monitor.rs` - Platform-specific idle time detection
- `autoplay_media.rs` - Media autoplay configuration for WKWebView/WebView2

### Configuration Layers

| Layer | File | Purpose |
|-------|------|---------|
| Build-time identity | `tauri.conf.json` | App name, bundle ID, metadata |
| Runtime URLs | `.env` | Screensaver URLs, defaults |
| User preferences | `options.json` | User's timing settings (auto-created) |

## Technical Details

### Multi-Monitor Fullscreen

macOS only allows one fullscreen transition at a time. The app staggers fullscreen calls with 600ms delays to ensure all monitors are covered properly.

### Audio Playback

The app uses a layered approach to stop audio cleanly:
1. JavaScript mute + pause (stops media elements)
2. Platform-native `stopLoading` (kills WebKit pipeline)
3. 500ms delay (CoreAudio drains)
4. Window close (destroys webview)

### Autoplay Configuration

On macOS, autoplay must be configured BEFORE any content loads. The app creates windows with `about:blank`, configures autoplay permissions, then navigates to the real URL.

## License

[Your License Here]

## Credits

Built with [Tauri v2](https://tauri.app/)

Original project by [Your Name/Organization]
