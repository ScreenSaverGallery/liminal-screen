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
VITE_APP_VERSION="1.0.0"                              # semver
VITE_APP_IDENTIFIER="com.yourcompany.your-app-name"  # MUST be unique per fork

# Screensaver URLs
VITE_SAVER_URL="https://your-domain.com/screensaver"
VITE_SAVER_URL_DEBUG="https://your-domain.com/screensaver?debug=true"

# Remote Options (optional)
VITE_OPTIONS_URL="https://your-domain.com/options.html"

# Updater (REQUIRED if using the Tauri updater plugin)
VITE_UPDATER_PUBKEY="-----BEGIN PUBLIC KEY-----
 paste your public key here
-----END PUBLIC KEY-----"
VITE_UPDATER_ENDPOINT="https://your-domain.com/releases/latest/download/latest.json"
```

**Important:** These values are read at **build time** and substituted into `tauri.conf.json` via Tauri v2's native `${{ env.VAR }}` template syntax — no patching script is needed. `tauri.conf.json` itself never needs to be edited by forks.

**Loading `.env`:** The Tauri CLI auto-loads `.env` from the project root. For production builds outside the CLI (e.g. CI), use a loader that preserves newlines — `VITE_UPDATER_PUBKEY` is multi-line:

```bash
# Preferred (preserves multi-line values)
set -a; source .env; set +a
bun run tauri build

# Or via Bun's built-in env loader
bun --env-file=.env run tauri build
```

> Avoid `export $(cat .env | xargs)` — it breaks on multi-line values like the updater PEM.

**Why bundle identifier matters:** If two apps have the same identifier on one system, they'll share preferences, keychain entries, and may crash each other. Each fork MUST use a unique `VITE_APP_IDENTIFIER`.

**Optional: Customize default timing values:**

```bash
VITE_DEFAULT_STARTS_IN=0.5        # Minutes before activation
VITE_DEFAULT_DISPLAY_OFF_IN=2     # Minutes before display off
VITE_DEFAULT_REQUIRE_PASS_IN=0    # Minutes until password required (0 = none)
VITE_DEFAULT_RUN_ON_BATTERY=false # Run on battery power?
VITE_DEFAULT_DEBUG=false          # Enable debug mode?
```

### 4. Replace the App Icon

  Note: This will be improved in future, as generating the icon set will become part of the build process.

Tauri ships with a default icon set based on its logo — not what you want for your fork. Place a `app-icon.png` (minimum 1024x1024px) in the project root and run:

```bash
bun tauri icon
```

This generates all platform icon files in `src-tauri/icons/` (macOS `.icns`, Windows `.ico`, iOS, Android, and all PNG sizes). See [Tauri Icons docs](https://v2.tauri.app/develop/icons/) for details.

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

# Build for production (preserves multi-line env values like VITE_UPDATER_PUBKEY)
set -a; source .env; set +a
bun run tauri build
```

## Configuration Behavior

### Persistent Storage

User preferences (timing values like `startsIn`, `displayOffIn`, etc.) are saved to `options.json` in the app's data directory. These persist across app restarts and updates.

**Priority order:**
1. **User-saved values** from `options.json` (highest priority)
2. **`.env` defaults** (used on first install or after factory reset)
3. **Hardcoded fallbacks** (if `.env` values aren't set)

**What's persisted:** Timing values (`startsIn`, `displayOffIn`, `requirePassIn`), `runOnBattery`, `debug`, `instanceId`

**What's NOT persisted:** URLs (`saver_url`, `saver_url_debug`, `options_url`) — these always come from `.env` so forks can update URLs without affecting user preferences.

### Factory Reset

Users can reset to `.env` defaults via the UI (Reset button) or by deleting `options.json` from the app's data directory.

Factory reset does two things:
1. Clears `options.json` (Tauri store) and regenerates `instanceId`
2. Resets in-memory state to `.env` defaults

**Browser storage:** Remote pages (screensaver, options) may have written data to `localStorage`, the Cache API, or registered service workers. These are not cleared from the native side. Instead, every remote window has `navigator.id` injected at document-start (set to the current `instanceId`). A page that stores the last-seen ID can detect a mismatch on load and self-clean — the changed `navigator.id` after reset is the signal. See `.hermes/plans/native-storage-cleanup/RETHINK.md` for the concept and implementation notes.

# Development mode
`bun run tauri dev`

# Production build (preserves multi-line env values like VITE_UPDATER_PUBKEY)
set -a; source .env; set +a
bun run tauri build

## Architecture

### Frontend (`src/`)

Minimal, reactive UI — no framework. Uses a lightweight `Signal` class for state management and reactive effects.

- `main.ts` — Application entry point: initialization, reactive effects, form handling, app identity (`setIdentity`), dialog interactions via `tauri-plugin-dialog`
- `app/reactive.ts` — Generic `Signal<T>` class with `.derive()` and `.effect()` for reactive data flow
- `app/types.ts` — `AppOptions` TypeScript type mirroring the Rust struct
- `app/preview/preview.ts` — Preview window creation and management
- `app/power-monitor/power-monitor.ts` — Bridge to Rust idle-time detection
- `styles.css` — Application styles

### Backend (`src-tauri/src/`)

The Rust backend is the engine — it handles all screensaver lifecycle, window management, power monitoring, and persistence.

- `main.rs` — App entry, Tauri plugin registration (store, dialog, opener)
- `lib.rs` — Core setup: window creation, system tray with dynamic tooltip (from `VITE_APP_NAME`), options CRUD, screensaver engine orchestration, `factory_reset_options` command, `build_init_script` (injects `navigator.id` and `navigator.userAgent` suffix into all remote windows at document-start)
- `screensaver_engine.rs` — Screensaver state machine: monitors idle time, creates/destroys fullscreen windows on activation/deactivation, manages multi-display layout
- `display_manager.rs` — Monitor detection and logical coordinate calculation for multi-monitor fullscreen positioning
- `power_monitor.rs` — Platform-specific idle time detection (macOS IOKit, Windows `GetLastInputInfo`, Linux systemd-inhibit + X11 screensaver queries)
- `autoplay_media.rs` — Per-window autoplay permission configuration for WKWebView (macOS) and WebView2 (Windows)
- `speech.rs` + `speech_polyfill.js` — `speechSynthesis` fallback for Linux (WebKitGTK ships no Web Speech API): a JS shim injected into saver/preview windows forwards `speak`/`cancel` to `spd-say` via Tauri commands; inert on macOS/Windows where the native API exists

### Shared Library (`packages/liminal-api/`)

Reusable SDK for fork developers who host their own remote options page. Works via `__TAURI__` globals (no npm install required).

- `src/index.ts` — `LiminalAPI` class: `getOptions`, `setOptions`, `resetOptions`, `previewScreensaver`, `startAutoSync`, `ask`, `showMessage`
- `src/store.ts` — `createOptionsStore` — signal-based reactive state with polling sync
- `src/reactive.ts` — Lightweight `Signal<T>` for remote options page
- `src/security.ts` — Tauri invoke validation and sanitization
- `src/types.ts` — `AppOptions`, `SetOptionsPayload`, `CustomOptions` types
- `examples/remote-options/` — Reference options page (HTML + JS + service worker) ready to deploy

### Configuration Layers

| Layer | File | Purpose |
|-------|------|---------|
| Build-time identity | `tauri.conf.json` | App name, version, bundle ID, descriptions, updater config — uses Tauri's `${{ env.VAR }}` template syntax, substituted from `.env` at build/dev time |
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

### Speech Synthesis on Linux

WebKitGTK does not implement `window.speechSynthesis`, so saver content that speaks text would be silent on Linux. The app injects a Web Speech API polyfill into saver and preview windows that forwards utterances to `spd-say`. **Linux users need `speech-dispatcher` installed** (provides the `spd-say` binary; preinstalled on many desktop distributions):

```bash
# Debian/Ubuntu
sudo apt install speech-dispatcher
# Fedora
sudo dnf install speech-dispatcher-utils
# Arch
sudo pacman -S speech-dispatcher
```

Without it, speech is skipped gracefully (utterances fire `error` events). macOS and Windows use their webviews' native speech synthesis — the polyfill steps aside there.

## License

Licensed under the [Apache License 2.0](LICENSE).

## Credits

Built with [Tauri v2](https://tauri.app/)

Original project by [tomaszatoo](https://github.com/tomaszatoo)
