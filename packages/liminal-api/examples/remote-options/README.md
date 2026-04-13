# Remote Options Example

A self-contained HTML page that demonstrates how to build a remote options interface for Liminal Screen. This page can be served from any HTTPS URL and loaded by the app's options window.

## How It Works

The remote options page operates in two modes:

### Tauri Mode (Production)

When loaded inside a Liminal Screen webview window, the page detects `window.__TAURI__` and communicates with the main app via IPC:

- **`invoke('get_options')`** — Fetches current options from the Rust backend
- **`invoke('set_options', { options })`** — Sends updated options to the Rust backend
- **`invoke('factory_reset_options')`** — Resets options to defaults
- **`listen('options-updated')`** — Receives real-time option updates pushed by the main app
- **`listen('idle-time-update')`** — Receives idle time updates from the Rust-side monitoring loop
- **`emit('preview-screensaver')`** — Triggers a screensaver preview

> **Note:** Tauri v2 auto-converts Rust `snake_case` field names to JavaScript `camelCase`. All JavaScript code uses `camelCase` (`startsIn`, `displayOffIn`, etc.), which Tauri converts to `snake_case` when sending to the Rust backend.

### Browser Mode (Development/Demo)

When opened in a regular browser without Tauri, the page uses mock data with a warning banner. This allows you to develop and test the UI without running the full app.

## Features

- **Real-time sync** — Options are automatically saved 800ms after the last input change (debounced auto-save)
- **Idle time display** — Shows system idle time from the Rust monitoring loop
- **Screensaver status** — Indicates whether the screensaver is currently active
- **Connection badge** — Shows whether the page is connected to Tauri or in demo mode
- **Factory reset** — Resets all options to their defaults
- **Preview** — Triggers a screensaver preview (Tauri mode only)
- **Responsive** — Adapts to narrow viewports (minimum 360px)

## Field Reference

The options form uses these fields (all `camelCase` for Tauri v2 compatibility):

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `startsIn` | number | 0.2 | Minutes of inactivity before screensaver activates |
| `displayOffIn` | number | 1.0 | Minutes before display turns off |
| `requirePassIn` | number | 1.0 | Minutes before password is required (not editable in this UI) |
| `runOnBattery` | boolean | false | Whether to run screensaver on battery power |
| `debug` | boolean | false | Enable debug mode |

The Rust backend also manages these read-only fields that are not exposed in this form:

| Field | Type | Description |
|-------|------|-------------|
| `saverUrl` | string | Main screensaver URL (from `VITE_SAVER_URL`) |
| `saverUrlDebug` | string | Debug screensaver URL (from `VITE_SAVER_URL_DEBUG`) |
| `optionsUrl` | string | Options page URL (from `VITE_OPTIONS_URL`) |

## Deployment

### 1. Host the page

Upload `index.html` to any static hosting service that supports HTTPS. For example:

- GitHub Pages
- Netlify
- Vercel
- AWS S3 + CloudFront

### 2. Configure Liminal Screen

Set the `VITE_OPTIONS_URL` environment variable to point to your hosted page:

```bash
# .env
VITE_OPTIONS_URL=https://your-domain.com/options/index.html
```

Or configure it at runtime via the Rust backend's `set_options` command.

### 3. Test locally

You can test the page in a browser without Tauri:

```bash
# Open directly in a browser
open index.html

# Or serve locally
npx serve .
```

The page will show a "demo mode" banner and use mock data.

## Using the `@liminal-screen/api` Package

If you prefer to use the npm package instead of writing IPC calls directly, install it:

```bash
npm install @liminal-screen/api
```

Then use it in your own page:

```javascript
import { liminalAPI } from '@liminal-screen/api';

await liminalAPI.init();

// Get current options
const options = await liminalAPI.getOptions();
console.log(options.startsIn);   // 0.2
console.log(options.displayOffIn); // 1.0

// Update options (partial update — merges with current)
await liminalAPI.setOptions({ debug: true, startsIn: 0.5 });

// Listen for updates pushed from the main app
const unsubscribe = liminalAPI.onOptionsUpdate((options) => {
  console.log('Options updated:', options);
});

// Preview screensaver
await liminalAPI.previewScreensaver();

// Factory reset
const defaults = await liminalAPI.resetOptions();

// Clean up when done
unsubscribe();
liminalAPI.destroy();
```

> **Important:** The `LiminalOptions` interface uses `camelCase` field names (`startsIn`, `displayOffIn`, etc.) to match Tauri v2's automatic snake_case → camelCase conversion.

## Architecture

```
┌─────────────────────────────────┐
│   Remote Options Page (HTTPS)   │
│                                 │
│  ┌───────────────────────────┐  │
│  │    RemoteOptionsAPI       │  │
│  │  ┌─────────┐ ┌─────────┐ │  │
│  │  │ Tauri   │ │  Mock    │ │  │
│  │  │  IPC    │ │  Data    │ │  │
│  │  └────┬────┘ └────┬────┘ │  │
│  └───────┼────────────┼─────┘  │
│          │            │         │
│          ▼            ▼         │
│    ┌──────────┐   ┌──────┐     │
│    │  Tauri   │   │ Demo │     │
│    │  Window  │   │ Mode │     │
│    └──────────┘   └──────┘     │
└─────────────────────────────────┘
         │
         ▼ IPC (invoke/listen/emit)
┌─────────────────────────────────┐
│     Liminal Screen (Rust)       │
│                                 │
│  ┌──────────┐  ┌──────────────┐ │
│  │  AppState │  │ PowerMonitor │ │
│  │  Options  │  │  Idle Loop   │ │
│  └──────────┘  └──────────────┘ │
└─────────────────────────────────┘
```

## Security Considerations

- The remote options page runs inside a Tauri webview with restricted capabilities
- The `options-capability` file controls which IPC commands the page can invoke
- `script-src` CSP allows `unsafe-inline` and `unsafe-eval` for compatibility with dynamically loaded pages
- The page communicates only with the local Tauri backend — no data is sent to external servers
- Auto-save debouncing (800ms) prevents excessive IPC calls during rapid input

## Troubleshooting

### Options page doesn't load

- Ensure `VITE_OPTIONS_URL` points to a valid HTTPS URL
- Check the Tauri console for CSP violations
- Verify the `options-capability` file includes `core:default` and `core:event:allow-listen`

### Options don't persist

- The Rust backend stores options in `AppState` (in-memory). To persist across restarts, the main app also writes to the Tauri Store plugin.
- Ensure the `store:default` permission is in the `default.json` capability file.

### Idle time shows "--"

- The idle time display requires Tauri IPC. In browser/demo mode, a mock incrementing counter is shown instead.
- If idle time is stuck at 0 in Tauri mode, check the browser console for IPC errors.