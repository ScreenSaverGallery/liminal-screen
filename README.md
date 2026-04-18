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
VITE_APP_NAME="Your App Name"
VITE_APP_DESCRIPTION="Your app description here"

# Screensaver URLs
VITE_SAVER_URL="https://your-domain.com/screensaver"
VITE_SAVER_URL_DEBUG="https://your-domain.com/screensaver?debug=true"

# Remote Options (optional)
VITE_OPTIONS_URL="https://your-domain.com/options.html"
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

### 3. Replace the App Icon

  Note: This will be improved in future, as generating the icon set will become part of the build process.

Tauri ships with a default icon set based on its logo — not what you want for your fork. Place a `app-icon.png` (minimum 1024x1024px) in the project root and run:

```bash
bun tauri icon
```

This generates all platform icon files in `src-tauri/icons/` (macOS `.icns`, Windows `.ico`, iOS, Android, and all PNG sizes). See [Tauri Icons docs](https://v2.tauri.app/develop/icons/) for details.

### 4. Edit `src-tauri/tauri.conf.json`

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

### 5. Edit `package.json` (Optional)

Update the package name for your fork:

```json
{
  "name": "your-app-name",
  "version": "1.0.0"
}
```

### 6. Build

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

Factory reset does three things:
1. Clears `options.json` (Tauri store)
2. Resets in-memory state to `.env` defaults
3. Injects cleanup JavaScript into any currently-open remote windows (options window, screensaver windows) to clear `localStorage`, `sessionStorage`, Cache API entries, and unregister service workers

**Limitations of browser storage cleanup:**

- **Closed windows are not cleaned.** Screensaver windows are typically closed when factory reset is triggered (screensaver inactive). Their remote-origin `localStorage`/Cache data persists in the WebView data store on disk until those windows open again.
- **Only the active saver URL domain is cleaned.** All screensaver windows at any given time load either `VITE_SAVER_URL` or `VITE_SAVER_URL_DEBUG` (based on the `debug` flag) — never both simultaneously. If those two URLs are on **different domains**, only the currently-active domain's storage is cleaned. The other domain's storage is unaffected.
- **Main window is intentionally excluded.** It loads a local Tauri asset (`tauri://localhost`) and uses no browser storage APIs.

# Development mode
`bun run tauri dev`

# Production build
`bun run tauri build`

## Architecture

### Frontend (`src/`)

Minimal, reactive UI — no framework. Uses a lightweight `Signal` class for state management and reactive effects.

- `main.ts` — Application entry point: initialization, reactive effects, form handling, app identity (`setIdentity`), dialog interactions via `tauri-plugin-dialog`
- `app/reactive.ts` — Generic `Signal<T>` class with `.derive()` and `.effect()` for reactive data flow
- `app/types.ts` — `AppOptions` TypeScript type mirroring the Rust struct
- `app/preview/preview.ts` — Preview window creation and management
- `app/power-monitor/power-monitory.ts` — Bridge to Rust idle-time detection
- `styles.css` — Application styles

### Backend (`src-tauri/src/`)

The Rust backend is the engine — it handles all screensaver lifecycle, window management, power monitoring, and persistence.

- `main.rs` — App entry, Tauri plugin registration (store, dialog, opener)
- `lib.rs` — Core setup: window creation, system tray with dynamic tooltip (from `VITE_APP_NAME`), options CRUD, screensaver engine orchestration, `factory_reset_options` command
- `screensaver_engine.rs` — Screensaver state machine: monitors idle time, creates/destroys fullscreen windows on activation/deactivation, manages multi-display layout
- `display_manager.rs` — Monitor detection and logical coordinate calculation for multi-monitor fullscreen positioning
- `power_monitor.rs` — Platform-specific idle time detection (macOS IOKit, Windows `GetLastInputInfo`, Linux systemd-inhibit + X11 screensaver queries)
- `autoplay_media.rs` — Per-window autoplay permission configuration for WKWebView (macOS) and WebView2 (Windows)

### Shared Library (`packages/liminal-api/`)

Reusable SDK for fork developers who host their own remote options page. Works via `__TAURI__` globals (no npm install required).

- `src/index.ts` — `LiminalAPI` class: `getOptions`, `setOptions`, `resetOptions`, `previewScreensaver`, `startAutoSync`, `ask`, `showMessage`
- `src/store.ts` — `createOptionsStore` — signal-based reactive state with polling sync
- `src/reactive.ts` — Lightweight `Signal<T>` for remote options page
- `src/security.ts` — Tauri invoke validation and sanitization
- `src/types.ts` — `AppOptions`, `SetOptionsPayload`, `CustomOptions` types
- `examples/remote-options/` — Reference options page (HTML + JS + service worker) ready to deploy

### Build Scripts (`scripts/`)

- `set-identity.ts` — Reads `.env` and patches `tauri.conf.json` with `VITE_APP_NAME` (productName, window title) and `VITE_APP_DESCRIPTION`. Runs automatically via `predev`/`prebuild` lifecycle hooks. Never touches the bundle `identifier`.

### Configuration Layers

| Layer | File | Purpose |
|-------|------|---------|
| Build-time identity | `tauri.conf.json` | App name, bundle ID, metadata — auto-patched from `.env` by `set-identity.ts` |
| Runtime identity | `.env` | `VITE_APP_NAME`, `VITE_APP_DESCRIPTION` — read by Rust backend and forwarded to frontend via `AppOptions` |
| Runtime URLs | `.env` | `VITE_SAVER_URL`, `VITE_SAVER_URL_DEBUG`, `VITE_OPTIONS_URL` |
| Runtime defaults | `.env` | `VITE_DEFAULT_STARTS_IN`, etc. — fallback values for first install |
| User preferences | `options.json` | User's saved timing settings (auto-created, persisted across updates) |

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

Licensed under the [Apache License 2.0](LICENSE).

## Credits

Built with [Tauri v2](https://tauri.app/)

Original project by [tomaszatoo]([https://](https://github.com/tomaszatoo))
