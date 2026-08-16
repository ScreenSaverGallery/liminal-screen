# Liminal Screen API Specification

## Overview

The Liminal Screen API provides a standardized interface for remote options pages to communicate with the Liminal Screen Tauri application. It works via `__TAURI__` globals (no npm runtime dependency) and auto-detects the environment — real IPC inside Tauri, mock data in browsers.

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
</script>
```

Requires `withGlobalTauri: true` in `tauri.conf.json`.

## Types

### `AppOptions`

Full options object returned by `getOptions()` and passed to callbacks:

```typescript
interface AppOptions extends MandatoryOptions {
  /** Production screensaver URL (read-only, from .env) */
  saverUrl: string;
  /** Debug screensaver URL (read-only, from .env) */
  saverUrlDebug: string;
  /** Remote options page URL (read-only, from .env) */
  optionsUrl: string;
  /** Fork display name (read-only, from VITE_APP_NAME) */
  appName: string;
  /** Fork description (read-only, from VITE_APP_DESCRIPTION) */
  appDescription: string;
  /** Fork-defined custom fields */
  customOptions: CustomOptions;
  /** Instance UUID (read-only; regenerated on factory reset) */
  instanceId: string;
  /** User consent for feed notifications — always present in get_options results */
  notificationsEnabled: boolean;
  /** Notification feed URL (read-only, from VITE_NOTIFICATION_URL; empty = disabled) */
  notificationUrl: string;
  /** Notification poll interval in seconds (read-only, from .env) */
  notificationCheckIntervalSecs: number;
  /** Start at login — always present in get_options results (reflects the OS state) */
  autostart: boolean;
}
```

### `MandatoryOptions`

User-configurable timing and behavior fields:

```typescript
interface MandatoryOptions {
  /** Minutes of inactivity before screensaver activates (min 0.1) */
  startsIn: number;
  /** Minutes before display turns off (min 0.5, 0 = disabled) */
  displayOffIn: number;
  /** Minutes before system lock (0 = disabled) */
  requirePassIn: number;
  /** Run screensaver on battery power */
  runOnBattery: boolean;
  /** Enable debug mode (loads saverUrlDebug instead of saverUrl) */
  debug: boolean;
  /**
   * User consent for feed notifications — opt-in, defaults to false.
   * Optional in payloads: setOptions() merges with current options, so
   * omitting it leaves the user's consent unchanged.
   */
  notificationsEnabled?: boolean;
  /**
   * Start Liminal at login. The OS login item is the source of truth — the
   * backend applies the change and reports back what the OS accepted, so the
   * saved value may differ from the requested one. Optional in payloads:
   * omitting it leaves the current state unchanged.
   */
  autostart?: boolean;
}
```

### `SetOptionsPayload`

Payload accepted by `setOptions()`. Identity fields are always preserved by the backend:

```typescript
type SetOptionsPayload = MandatoryOptions & {
  customOptions?: CustomOptions;
};
```

### `CustomOptions`

Fork-defined key/value pairs appended to the screensaver URL as query parameters:

```typescript
type CustomOptions = Record<string, string | number | boolean>;
```

### `OsScreensaverStatus`

Snapshot of the OS-native screensaver configuration, returned by `getOsScreensaverStatus()`:

```typescript
interface OsScreensaverStatus {
  /** Whether the setting could be read on this platform/desktop */
  detected: boolean;
  /** True when the OS screensaver is set to activate on an idle timer */
  enabled: boolean;
  /** Idle seconds before the OS screensaver starts; null when disabled/unknown */
  idleSeconds: number | null;
}
```

Detection is solid on macOS; Windows and Linux (GNOME) are best-effort and report `detected: false` when the setting can't be read.

## `LiminalAPI` Class

### `getOptions(): Promise<AppOptions>`

Retrieve the current application options. In non-Tauri environments, returns mock defaults.

```javascript
const options = await liminalAPI.getOptions();
console.log(options.appName);  // "My Screensaver"
console.log(options.startsIn); // 0.2
```

### `setOptions(payload: SetOptionsPayload): Promise<void>`

Persist user-controlled options to the backend. Read-only identity fields (`saverUrl`, `appName`, etc.) are always preserved — only the fields in `MandatoryOptions` and `customOptions` are updated.

```javascript
await liminalAPI.setOptions({
  startsIn: 5,
  displayOffIn: 10,
  debug: false,
});
```

`notificationsEnabled` and `autostart` are optional — omit them and the current
values are kept. `autostart` is applied to the OS login item, so read the value
back with `getOptions()` if you need to know what the OS accepted.

### `resetOptions(): Promise<AppOptions>`

Reset all options to the fork's `.env` defaults. Returns the reset options.

```javascript
const defaults = await liminalAPI.resetOptions();
// defaults.startsIn === env value, etc.
```

### `previewScreensaver(): Promise<void>`

Open a preview of the screensaver in its own resizable window (800×600, titled
"Screensaver Preview"). Reads the current options and uses `saverUrlDebug` when
`debug` is on, `saverUrl` otherwise.

The preview window is created directly via the backend's `create_preview_window`
command rather than through the main window's event relay, so it works from a
remote options page. The window uses a fixed label (`liminal-preview`) and the
backend is idempotent per label: calling this again while a preview is already
open does nothing.

Throws `LiminalAPIError('No saver URL configured for preview')` when the
resolved URL is empty — e.g. `debug` is on but the fork set no
`VITE_SAVER_URL_DEBUG`. No-op (logs) outside Tauri.

```javascript
try {
  await liminalAPI.previewScreensaver();
} catch (e) {
  await liminalAPI.showMessage(e.message, { title: 'Preview', kind: 'error' });
}
```

Requires an app build with the `create_preview_window` command — Liminal Screen
0.2.0 or newer.

### `openUrl(url: string, openWith?: string): Promise<void>`

Open an external URL in the user's default browser or application, via the Tauri
`opener` plugin. Use this for every outbound link: the options page runs in a
webview, so a plain `<a href>` navigates away from your own page and
`window.open()` is usually blocked outright.

`openWith` optionally names the application to open the URL with; omit it for the
system default.

```javascript
link.addEventListener('click', (e) => {
  e.preventDefault();
  liminalAPI.openUrl('https://example.com/docs');
});

await liminalAPI.openUrl('mailto:support@example.com');
```

Requires the `opener:default` (or `opener:allow-open-url`) permission on the
options window's capability, **granted to the page's own remote origin** — Tauri
scopes capabilities to local content by default, so listing the permission is not
enough on its own. Liminal Screen 0.3.0+ registers that grant at runtime from
`VITE_OPTIONS_URL`; see
[INTEGRATION-GUIDE.md](INTEGRATION-GUIDE.md#remote-origins-and-the-acl). The
plugin's default scope permits `http:`, `https:`, `mailto:` and `tel:`.

If the plugin call fails (missing permission, blocked scheme) the error is logged
as a warning and the method falls back to
`window.open(url, '_blank', 'noopener')`, which most webviews ignore — so a
missing permission looks like "nothing happened", not an exception. Check the
webview console when a link seems dead.

### `closeOptions(): Promise<void>`

Close the options window this page is running in — for a "Close" or "Done" button.

```javascript
$('done-btn').addEventListener('click', async () => {
  await store.save(collectForm());
  await liminalAPI.closeOptions();
});
```

Closes the *window*, not just the page, so unsaved form state is discarded — save
first if that matters. A no-op when the window is already closed, and outside
Tauri.

This goes through the app's `close_options` command rather than Tauri's window
API. A remote page calling `getCurrentWindow().close()` would need
`core:window:allow-close` granted to its own origin, and a denied core command
surfaces as nothing happening at all; app-defined commands aren't ACL-gated, so
this path can't fail that way. Requires Liminal Screen 0.3.0+ — on older builds it
rejects with `LiminalAPIError` rather than failing silently.

Not to be confused with [`destroy()`](#destroy-void), which detaches event
listeners and leaves the window open.

### `getVersion(): Promise<string>`

The running application version (e.g. `"0.3.0"`). Reads the injected `navigator.liminalScreen.version` snapshot when present (no IPC), otherwise asks the backend. Returns an empty string outside Tauri.

```javascript
document.getElementById('version').textContent = `v${await liminalAPI.getVersion()}`;
```

### `getOsScreensaverStatus(): Promise<OsScreensaverStatus>`

Read the OS-native screensaver configuration. Liminal is meant to be the *only* screensaver — a system screensaver on an overlapping timer draws over Liminal — so use this to detect a conflict and warn the user. In non-Tauri environments returns `{ detected: false, enabled: false, idleSeconds: null }`.

```javascript
const os = await liminalAPI.getOsScreensaverStatus();
if (os.detected && os.enabled) {
  console.log(`OS screensaver starts after ${os.idleSeconds}s`);
}
```

### `disableOsScreensaver(): Promise<void>`

Disable the OS-native screensaver so it can't appear over Liminal. The current timeout is saved first, so the change can be reversed with `restoreOsScreensaver()`. No-op (logs) outside Tauri.

- **macOS**: `defaults -currentHost write com.apple.screensaver idleTime 0` + `killall cfprefsd`
- **Windows**: `SystemParametersInfoW(SPI_SETSCREENSAVEACTIVE, FALSE)` (best-effort)
- **Linux**: `gsettings set org.gnome.desktop.session idle-delay 0` (GNOME, best-effort)

```javascript
await liminalAPI.disableOsScreensaver();
```

### `restoreOsScreensaver(): Promise<void>`

Restore the OS-native screensaver to the timeout saved by `disableOsScreensaver()`, then clear the saved value. Throws if there's nothing saved to restore.

### `getSavedOsScreensaverIdle(): Promise<number | null>`

The OS screensaver timeout (seconds) Liminal saved when it disabled the screensaver, or `null` if Liminal hasn't disabled it. A non-null value means "Liminal disabled it" — use it to decide whether to show a **Restore** affordance.

```javascript
const saved = await liminalAPI.getSavedOsScreensaverIdle();
if (saved != null) {
  // Liminal disabled the OS screensaver (was `saved` seconds) — offer to restore
}
```

### `isMediaActive(): Promise<boolean>`

`true` when another process — a video player, video call, etc. — is holding a
display-sleep-blocking power assertion. The screensaver engine already treats
this as user activity and won't activate the saver while it's true; use this
to tell the user *why* the saver hasn't started instead of leaving them
thinking it's broken.

macOS only for now: reads IOKit power assertion status
(`IOPMCopyAssertionsStatus`), excluding Liminal's own `caffeinate -d`
inhibitor. Always resolves `false` on Windows/Linux, where this detection
isn't implemented, and outside Tauri. Requires Liminal Screen with the
`is_media_active` command (unreleased as of app `0.2.0`) — rejects with
`LiminalAPIError` on older builds.

On its own this only tells you *that* the saver is blocked, not *why* — genuine
media playback is usually obvious to the user, but a background app holding the
same kind of assertion (a file-sharing tool, a sync client) is not. Pair it with
`getMediaBlockerName()` (below) to name the
process responsible.

```javascript
setInterval(async () => {
  const blocked = await liminalAPI.isMediaActive();
  statusEl.textContent = blocked ? 'Screensaver paused — media is playing' : '';
}, 5000);
```

### `getMediaBlockerName(): Promise<string | null>`

Name of the process holding the display-sleep assertion `isMediaActive()`
detected (e.g. `"LocalSend"`), or `null` if nothing is. Reads `pmset -g
assertions`' per-process listing, since that's the only place macOS exposes
the owning process's name (`IOPMCopyAssertionsStatus`, used by
`isMediaActive()`, only gives system-wide counts) — heavier than
`isMediaActive()`, so call it only once you already know the saver is
blocked, not on every poll tick. Excludes Liminal's own `caffeinate -d`
inhibitor by PID. macOS only for now — always resolves `null` on
Windows/Linux and outside Tauri. Requires Liminal Screen with the
`get_media_blocker_name` command (unreleased as of app `0.2.0`) — rejects
with `LiminalAPIError` on older builds.

```javascript
if (await liminalAPI.isMediaActive()) {
  const who = await liminalAPI.getMediaBlockerName();
  message = who
    ? `${who} is blocking ${appName} from starting.`
    : 'Something is blocking the screensaver from starting.';
}
```

### `ask(message: string, options?: Record<string, unknown>): Promise<boolean>`

Show a confirmation dialog. Uses `tauri-plugin-dialog` inside Tauri (native OS dialog), falls back to `window.confirm()` in browsers.

```javascript
if (!await liminalAPI.ask('Reset all options to defaults?', {
  title: 'Reset',
  kind: 'warning',
  okLabel: 'Reset',
  cancelLabel: 'Cancel',
})) {
  return; // user cancelled
}
```

### `showMessage(message: string, options?: Record<string, unknown>): Promise<void>`

Show a message dialog. Uses `tauri-plugin-dialog` inside Tauri, falls back to `window.alert()` in browsers.

```javascript
await liminalAPI.showMessage('Settings saved!', { title: 'Saved', kind: 'info' });
```

### `startAutoSync(callback: (options: AppOptions) => void): Promise<() => void>`

Subscribe to real-time option updates from the Tauri backend. Also re-dispatches to the window event bus so `onOptionsUpdate()` listeners fire. Returns an unsubscribe function.

```javascript
const unlisten = await liminalAPI.startAutoSync((options) => {
  console.log('Options updated:', options.startsIn);
});
// Later: unlisten();
```

### `onOptionsUpdate(callback: (options: AppOptions) => void): () => void`

Listen for option updates on the window event bus (`liminal:options-updated`). Works without Tauri — useful when `setOptions()` is called locally. Returns an unsubscribe function.

```javascript
const unsub = liminalAPI.onOptionsUpdate((options) => {
  console.log('Options changed:', options);
});
// Later: unsub();
```

### `checkForUpdates(): Promise<UpdateInfo | null>`

Check for an application update. Returns `{ version, notes? }` when an update
is available, `null` otherwise (or outside Tauri). Also causes the backend to
emit `update-available` / `update-not-available` events.

```ts
const update = await liminalAPI.checkForUpdates();
if (update) console.log(`v${update.version} available`);
```

### `installUpdate(): Promise<void>`

Download and install a pending update. The backend emits
`update-download-progress` events while downloading, then restarts the app.

### `onUpdateAvailable(callback: (info: UpdateInfo) => void): () => void`

Subscribe to `update-available` events (fired by both the startup check and
manual checks). Returns an unsubscribe function. No-op outside Tauri.

### `destroy(): void`

Remove all event listeners registered via `startAutoSync()` and
`onUpdateAvailable()`. Call on page unload.

### `isInTauri: boolean`

Read-only property. `true` when running inside a Tauri webview.

## `createOptionsStore(api: LiminalAPI)`

Creates a reactive options store for declarative UI patterns:

```javascript
import { createOptionsStore } from '@liminal-screen/api';

const store = createOptionsStore(liminalAPI);

// Re-render whenever options change
store.signal.effect((opts) => {
  if (!opts) return;
  document.getElementById('starts-in').value = String(opts.startsIn);
  document.getElementById('app-name').textContent = opts.appName;
});

// Save collected form data
await store.save(formData);

// Reset to .env defaults
await store.reset();

// Clean up on page unload
window.addEventListener('beforeunload', () => store.destroy());
```

Returns:

| Property | Type | Description |
|-----------|------|-------------|
| `signal` | `Signal<AppOptions \| null>` | Reactive signal that fires on init and every backend update |
| `save` | `(payload: SetOptionsPayload) => Promise<void>` | Save options and sync signal |
| `reset` | `() => Promise<void>` | Reset to defaults and sync signal |
| `destroy` | `() => void` | Clean up polling and event listeners |

## `Signal<T>`

Lightweight reactive primitive (exported for use in custom reactive patterns):

```typescript
const count = new Signal(0);
const doubled = count.derive(v => v * 2);

count.effect(v => console.log('count:', v));
doubled.effect(v => console.log('doubled:', v));

count.set(5);    // logs: count: 5, doubled: 10
count.update(v => v + 1);  // logs: count: 6, doubled: 12
```

## Error Handling

All API methods may throw `LiminalAPIError`:

```javascript
import { LiminalAPIError } from '@liminal-screen/api';

try {
  await liminalAPI.setOptions({ startsIn: 0.5 });
} catch (error) {
  if (error instanceof LiminalAPIError) {
    console.error('API Error:', error.message, error.cause);
  }
}
```

## Environment Detection

The API automatically detects whether it's running inside a Tauri webview:

```javascript
if (liminalAPI.isInTauri) {
  console.log('Running in Liminal Screen — real IPC');
} else {
  console.log('Running in browser — mock mode');
}
```

In Tauri: all operations use real IPC via `window.__TAURI__.core.invoke`.

In browsers: `getOptions()` returns mock defaults, `setOptions()` logs to console, `ask()`/`showMessage()` fall back to `confirm()`/`alert()`, `openUrl()` falls back to `window.open()`, and `previewScreensaver()`/`closeOptions()` are no-ops.

## Integration Guide

### Minimal HTML Page

```html
<!DOCTYPE html>
<html>
<head>
  <title>My Options</title>
  <script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
</head>
<body>
  <h1 id="app-name">Options</h1>
  <form id="options-form">
    <label>Start After (min): <input type="number" id="starts-in" min="0.1" step="0.1"></label>
    <button type="button" id="save-btn">Save</button>
    <button type="button" id="reset-btn">Reset</button>
  </form>

  <script>
    const { liminalAPI, createOptionsStore } = LiminalAPI;
    const store = createOptionsStore(liminalAPI);

    store.signal.effect((opts) => {
      if (!opts) return;
      document.getElementById('starts-in').value = opts.startsIn;
      document.getElementById('app-name').textContent = opts.appName;
    });

    document.getElementById('save-btn').addEventListener('click', async () => {
      const opts = store.signal.get();
      if (!opts) return;
      await store.save({
        ...opts,
        startsIn: parseFloat(document.getElementById('starts-in').value),
      });
    });

    document.getElementById('reset-btn').addEventListener('click', async () => {
      if (!await liminalAPI.ask('Reset to defaults?', { title: 'Reset', kind: 'warning' })) return;
      await store.reset();
    });
  </script>
</body>
</html>
```

### Configuring Liminal Screen

Set the `VITE_OPTIONS_URL` environment variable to point to your hosted page:

```bash
VITE_OPTIONS_URL="https://your-domain.com/options.html"
```

## Security

- All IPC communication is sandboxed by Tauri
- Remote options pages cannot access sensitive system APIs directly
- Options updates are validated by the Rust backend
- Identity fields (`saverUrl`, `appName`, etc.) are read-only — user submissions are ignored
- Plugin permissions must be explicitly granted in Tauri capability files — `dialog:allow-ask` and `dialog:allow-message` for dialogs, `opener:default` for `openUrl()`
- `openUrl()` is scoped by the opener plugin (`http:`, `https:`, `mailto:`, `tel:` by default) — narrow the scope in your capability file if your page only needs specific hosts

## Versioning

This API follows semantic versioning. Breaking changes will result in major version increments.