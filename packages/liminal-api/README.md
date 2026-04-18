# Liminal Screen API

IPC bridge for Liminal Screen remote options pages. Works as an npm package or a CDN-loaded script — no `@tauri-apps/api` dependency required.

## Overview

The Liminal Screen API lets remote options pages communicate with the Tauri backend via `__TAURI__` globals (requires `withGlobalTauri: true` in `tauri.conf.json`). It auto-detects whether it's running inside a Tauri window or a regular browser and falls back to mock data when outside Tauri.

## Features

- **Cross-environment**: Works in Tauri webviews and plain browsers (mock mode)
- **TypeScript**: Full types for `AppOptions`, `SetOptionsPayload`, `CustomOptions`
- **Reactive store**: `createOptionsStore()` provides a `Signal`-based reactive state with auto-polling sync
- **Native dialogs**: `ask()` and `showMessage()` use Tauri's dialog plugin when available, fall back to `confirm()`/`alert()`
- **Event sync**: `startAutoSync()` pushes real-time option updates from the backend
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

## API Reference

### `liminalAPI` — singleton instance

| Method | Returns | Description |
|--------|---------|-------------|
| `getOptions()` | `Promise<AppOptions>` | Get current options from backend |
| `setOptions(payload)` | `Promise<void>` | Save user options (identity fields preserved) |
| `resetOptions()` | `Promise<AppOptions>` | Reset to `.env` defaults |
| `previewScreensaver()` | `Promise<void>` | Trigger a screensaver preview |
| `ask(message, options?)` | `Promise<boolean>` | Confirmation dialog (falls back to `confirm()`) |
| `showMessage(message, options?)` | `Promise<void>` | Message dialog (falls back to `alert()`) |
| `startAutoSync(callback)` | `Promise<() => void>` | Subscribe to real-time option updates |
| `onOptionsUpdate(callback)` | `() => void` | Listen on window event bus (works outside Tauri) |
| `destroy()` | `void` | Clean up all listeners |
| `isInTauri` | `boolean` | `true` when running inside Tauri |

### `createOptionsStore(api)` — reactive store

Returns `{ signal, save, reset, destroy }` where `signal` is a `Signal<AppOptions | null>`.

### `AppOptions` type

```typescript
interface AppOptions extends MandatoryOptions {
  saverUrl: string;          // Production screensaver URL (read-only)
  saverUrlDebug: string;     // Debug screensaver URL (read-only)
  optionsUrl: string;         // Remote options URL (read-only)
  appName: string;            // Fork display name (read-only)
  appDescription: string;    // Fork description (read-only)
  customOptions: CustomOptions; // Fork-defined key/value pairs
}

interface MandatoryOptions {
  startsIn: number;           // Minutes before activation
  displayOffIn: number;       // Minutes before display off
  requirePassIn: number;      // Minutes before lock (0 = disabled)
  runOnBattery: boolean;      // Run on battery power
  debug: boolean;             // Use debug URL
}

type CustomOptions = Record<string, string | number | boolean>;
```

## Documentation

- **[API.md](docs/API.md)** — Full API specification
- **[INTEGRATION-GUIDE.md](docs/INTEGRATION-GUIDE.md)** — Step-by-step integration guide
- **[SECURITY.md](docs/SECURITY.md)** — Security model and best practices

## Reference Implementation

See `examples/remote-options/` for a complete options page with form handling, reactive store, native dialogs, and service worker.

## Development

```bash
# Build (ESM + IIFE + types)
bun run build

# Typecheck
bun run typecheck
```

## License

MIT