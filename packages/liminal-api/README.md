# Liminal Screen API

IPC bridge for [Liminal Screen](https://github.com/tomaszatoo/liminal-screen) remote options pages. Works as an npm package or a CDN-loaded script — no `@tauri-apps/api` dependency required.

## Overview

The Liminal Screen API lets remote options pages communicate with the Tauri backend via `__TAURI__` globals (requires `withGlobalTauri: true` in `tauri.conf.json`). It auto-detects whether it's running inside a Tauri window or a regular browser and falls back to mock data when outside Tauri.

## Features

- **Cross-environment**: Works in Tauri webviews and plain browsers (mock mode)
- **TypeScript**: Full types for `AppOptions`, `SetOptionsPayload`, `CustomOptions`, `UpdateInfo`
- **Reactive store**: `createOptionsStore()` provides a `Signal`-based reactive state kept in sync with the backend
- **Native dialogs**: `ask()` and `showMessage()` use Tauri's dialog plugin when available, fall back to `confirm()`/`alert()`
- **External links**: `openUrl()` opens links in the user's real browser instead of hijacking the options window
- **Screensaver preview**: `previewScreensaver()` opens a windowed preview of the configured saver URL
- **Event sync**: `startAutoSync()` pushes real-time option updates from the backend
- **System screensaver control**: detect a conflicting OS screensaver and disable/restore it so Liminal is the only screensaver
- **App updates**: `checkForUpdates()`, `installUpdate()` and an `update-available` subscription
- **App version**: `getVersion()` returns the running application version
- **Zero dependencies**: No `@tauri-apps/api` needed — uses `window.__TAURI__` globals directly

## Installation

### npm

```bash
npm install @liminal-screen/api
```

```javascript
import { liminalAPI, createOptionsStore } from '@liminal-screen/api';
```

### CDN (no build step)

```html
<script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
<script>
  const { liminalAPI, createOptionsStore } = LiminalAPI;
  // ...
</script>
```

Pin an exact version in production — e.g. `@liminal-screen/api@0.3.0`.

## Quick Start

### Basic — imperative API

```javascript
const options = await liminalAPI.getOptions();
console.log(options.appName, options.startsIn);

await liminalAPI.setOptions({ startsIn: 5, debug: true });

const defaults = await liminalAPI.resetOptions();
```

### Reactive — with options store

```javascript
import { createOptionsStore } from '@liminal-screen/api';

const store = createOptionsStore(liminalAPI);

// Re-render whenever options change
store.signal.effect((opts) => {
  if (!opts) return;
  document.getElementById('starts-in').value = opts.startsIn;
  document.getElementById('app-name').textContent = opts.appName;
});

// Save form data
await store.save(collectedFormData);

// Reset to defaults
await store.reset();

// Clean up on unload
window.addEventListener('beforeunload', () => store.destroy());
```

### Dialogs

```javascript
// Confirm before resetting
if (!await liminalAPI.ask('Reset all options to defaults?', { title: 'Reset', kind: 'warning' })) {
  return;
}
await liminalAPI.resetOptions();

// Show a success message
await liminalAPI.showMessage('Settings saved!', { title: 'Saved', kind: 'info' });
```

### External links and preview

A remote options page is loaded in a webview, so a plain `<a href>` or
`window.open()` either replaces your options page or is silently blocked. Use
`openUrl()` to hand the link to the user's real browser:

```javascript
document.getElementById('docs-link').addEventListener('click', (e) => {
  e.preventDefault();
  liminalAPI.openUrl('https://example.com/docs');
});

// Optionally pick the application to open it with
await liminalAPI.openUrl('mailto:support@example.com');
```

`previewScreensaver()` opens the configured saver URL (`saverUrlDebug` when
`debug` is on) in its own resizable window, so users can see the effect of their
settings without waiting for the idle timer:

```javascript
document.getElementById('preview-btn').addEventListener('click', async () => {
  try {
    await liminalAPI.previewScreensaver();
  } catch (e) {
    await liminalAPI.showMessage(e.message, { kind: 'error' });
  }
});
```

### System screensaver conflict

Liminal is meant to be the *only* screensaver — a system screensaver on an overlapping timer draws over Liminal. Detect one and offer to disable it (the prior timeout is saved so it can be restored):

```javascript
const os = await liminalAPI.getOsScreensaverStatus();
if (os.detected && os.enabled) {
  // e.g. os.idleSeconds === 60 → the OS screensaver starts after 1 minute
  if (await liminalAPI.ask('Your system screensaver may appear over Liminal. Disable it?')) {
    await liminalAPI.disableOsScreensaver();
  }
}

// Offer to undo it later — non-null means Liminal disabled it:
const saved = await liminalAPI.getSavedOsScreensaverIdle();
if (saved != null) {
  await liminalAPI.restoreOsScreensaver();
}
```

### App updates

```javascript
// React to the startup check as well as manual checks
liminalAPI.onUpdateAvailable((info) => {
  banner.textContent = `Version ${info.version} is available`;
});

// Manual check, gated behind a user action
const update = await liminalAPI.checkForUpdates();
if (update && await liminalAPI.ask(`Install v${update.version} now? The app will restart.`)) {
  await liminalAPI.installUpdate();
}
```

### App version

```javascript
document.getElementById('version').textContent = `v${await liminalAPI.getVersion()}`;
```

## API Reference

### `liminalAPI` — singleton instance

| Method | Returns | Description |
|--------|---------|-------------|
| `getOptions()` | `Promise<AppOptions>` | Get current options from backend |
| `setOptions(payload)` | `Promise<void>` | Save user options (identity fields preserved) |
| `resetOptions()` | `Promise<AppOptions>` | Reset to `.env` defaults |
| `previewScreensaver()` | `Promise<void>` | Open a preview window for the configured saver URL |
| `openUrl(url, openWith?)` | `Promise<void>` | Open an external URL in the user's default browser/app |
| `getVersion()` | `Promise<string>` | Running app version (e.g. `"0.3.0"`) |
| `getOsScreensaverStatus()` | `Promise<OsScreensaverStatus>` | Read the OS-native screensaver config (conflict detection) |
| `disableOsScreensaver()` | `Promise<void>` | Disable the OS screensaver so it can't cover Liminal (prior value saved) |
| `restoreOsScreensaver()` | `Promise<void>` | Restore the OS screensaver to the saved value |
| `getSavedOsScreensaverIdle()` | `Promise<number \| null>` | Saved OS timeout (seconds) if Liminal disabled it, else `null` |
| `ask(message, options?)` | `Promise<boolean>` | Confirmation dialog (falls back to `confirm()`) |
| `showMessage(message, options?)` | `Promise<void>` | Message dialog (falls back to `alert()`) |
| `checkForUpdates()` | `Promise<UpdateInfo \| null>` | Check for an app update; `null` when none (or outside Tauri) |
| `installUpdate()` | `Promise<void>` | Download and install a pending update, then restart |
| `onUpdateAvailable(callback)` | `() => void` | Subscribe to `update-available` events |
| `startAutoSync(callback)` | `Promise<() => void>` | Subscribe to real-time option updates |
| `onOptionsUpdate(callback)` | `() => void` | Listen on window event bus (works outside Tauri) |
| `destroy()` | `void` | Clean up all listeners |
| `isInTauri` | `boolean` | `true` when running inside Tauri |

Also exported: `createOptionsStore`, `Signal`, `LiminalAPIError`, and the `LiminalAPI` class itself for multi-instance setups.

### `createOptionsStore(api)` — reactive store

Returns `{ signal, save, reset, destroy }` where `signal` is a `Signal<AppOptions | null>`.

### `AppOptions` type

```typescript
interface AppOptions extends MandatoryOptions {
  saverUrl: string;           // Production screensaver URL (read-only)
  saverUrlDebug: string;      // Debug screensaver URL (read-only)
  optionsUrl: string;         // Remote options URL (read-only)
  appName: string;            // Fork display name (read-only)
  appDescription: string;     // Fork description (read-only)
  customOptions: CustomOptions;      // Fork-defined key/value pairs
  instanceId: string;                // Instance UUID (read-only, reset on factory reset)
  notificationsEnabled: boolean;     // User consent for feed notifications
  notificationUrl: string;           // Notification feed URL (read-only; empty = disabled)
  notificationCheckIntervalSecs: number; // Poll interval (read-only)
  autostart: boolean;                // Start at login (reflects the OS login item)
}

interface MandatoryOptions {
  startsIn: number;            // Minutes before activation
  displayOffIn: number;        // Minutes before display off
  requirePassIn: number;       // Minutes before lock (0 = disabled)
  runOnBattery: boolean;       // Run on battery power
  debug: boolean;              // Use debug URL
  notificationsEnabled?: boolean; // Opt-in; omit to keep current consent
  autostart?: boolean;         // Omit to keep the current login-item state
}

type CustomOptions = Record<string, string | number | boolean>;
```

`setOptions()` merges the payload over the current options, so the optional fields can be omitted safely. Read-only fields are re-applied by the backend and cannot be changed from the options page.

### `OsScreensaverStatus` type

```typescript
interface OsScreensaverStatus {
  detected: boolean;          // Could the setting be read on this platform/desktop?
  enabled: boolean;           // Is the OS screensaver set to activate on a timer?
  idleSeconds: number | null; // Idle seconds before it starts; null if disabled/unknown
}
```

### `UpdateInfo` type

```typescript
interface UpdateInfo {
  version: string;            // Version of the available update
  notes?: string;             // Release notes, when the release provides them
}
```

## App Compatibility

This package version is independent of the Liminal Screen app version — it
tracks its own JavaScript API surface. But each method calls into the app's
backend, so a few need a recent enough app build. Outside Tauri everything falls
back to mock behaviour, so this only matters for the installed app your fork
ships.

| Package | Requires app | Notes |
|---------|--------------|-------|
| `0.3.0` | `0.2.0`+ | `previewScreensaver()` uses the `create_preview_window` command |
| `0.3.0` | `0.3.0`+ | `openUrl()` needs the `opener:default` permission on the options window; without it the call falls back to `window.open()`, which webviews usually block |
| `0.2.0` | `0.2.0`+ | Updater, notification and autostart fields |

Fork developers: if you've customised `src-tauri/capabilities/options.json`, add
`opener:default` (or `opener:allow-open-url`) to use `openUrl()`.

## Documentation

- **[API.md](docs/API.md)** — Full API specification
- **[INTEGRATION-GUIDE.md](docs/INTEGRATION-GUIDE.md)** — Step-by-step integration guide
- **[SECURITY.md](docs/SECURITY.md)** — Trust model and deployment recommendations

## Reference Implementation

See [`examples/remote-options/`](https://github.com/tomaszatoo/liminal-screen/tree/main/packages/liminal-api/examples/remote-options) for a complete options page with form handling, reactive store, native dialogs, and service worker.

## Development

```bash
# Build (ESM + IIFE + types)
bun run build

# Typecheck
bun run typecheck

# Tests (run from the repository root)
bun run test
```

## License

MIT — see [LICENSE](LICENSE).
