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

### `resetOptions(): Promise<AppOptions>`

Reset all options to the fork's `.env` defaults. Returns the reset options.

```javascript
const defaults = await liminalAPI.resetOptions();
// defaults.startsIn === env value, etc.
```

### `previewScreensaver(): Promise<void>`

Trigger a preview of the screensaver. No-op in non-Tauri environments.

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

### `destroy(): void`

Remove all event listeners registered via `startAutoSync()`. Call on page unload.

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

In browsers: `getOptions()` returns mock defaults, `setOptions()` logs to console, `ask()`/`showMessage()` fall back to `confirm()`/`alert()`, `previewScreensaver()` is a no-op.

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
      await store.save({
        startsIn: parseFloat(document.getElementById('starts-in').value),
        displayOffIn: opts.displayOffIn,
        requirePassIn: opts.requirePassIn,
        runOnBattery: opts.runOnBattery,
        debug: opts.debug,
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
- Dialog permissions must be explicitly granted in Tauri capability files (`dialog:allow-ask`, `dialog:allow-message`)

## Versioning

This API follows semantic versioning. Breaking changes will result in major version increments.