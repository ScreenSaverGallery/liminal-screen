# Liminal Screen Integration Guide

## Overview

This guide explains how to create a custom remote options page for a Liminal Screen fork. The options page is a regular HTML page hosted at a URL you control. Liminal Screen loads it in a webview and communicates via the `@liminal-screen/api` library.

## Prerequisites

- A web server or static hosting for your options page
- `withGlobalTauri: true` set in `tauri.conf.json` (already the default in Liminal Screen)

## Getting Started

### Option A: npm package

```bash
npm install @liminal-screen/api
```

```javascript
import { liminalAPI, createOptionsStore } from '@liminal-screen/api';
```

### Option B: CDN (no build step)

```html
<script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
<script>
  const { liminalAPI, createOptionsStore } = LiminalAPI;
</script>
```

## Reactive Pattern (Recommended)

Use `createOptionsStore()` for a declarative UI that auto-updates when options change:

```javascript
const store = createOptionsStore(liminalAPI);

// Re-render whenever options change (fires immediately + on every update)
store.signal.effect((opts) => {
  if (!opts) return;
  document.getElementById('starts-in').value = String(opts.startsIn);
  document.getElementById('app-name').textContent = opts.appName;
  document.getElementById('app-description').textContent = opts.appDescription;
});

// Save collected form data
document.getElementById('save-btn').addEventListener('click', () => {
  store.save(collectForm());
});

// Reset to .env defaults (with confirmation)
document.getElementById('reset-btn').addEventListener('click', async () => {
  if (!await liminalAPI.ask('Reset all options to defaults?', { title: 'Reset', kind: 'warning' })) return;
  await store.reset();
});

// Clean up on page unload
window.addEventListener('beforeunload', () => store.destroy());
```

## Imperative Pattern

If you prefer manual control:

```javascript
// Load options
const opts = await liminalAPI.getOptions();
document.getElementById('starts-in').value = String(opts.startsIn);

// Save
await liminalAPI.setOptions({ startsIn: 5, displayOffIn: 10, /* ... */ });

// Reset
if (await liminalAPI.ask('Reset?')) {
  await liminalAPI.resetOptions();
}

// Listen for external updates
await liminalAPI.startAutoSync((updatedOpts) => {
  document.getElementById('starts-in').value = String(updatedOpts.startsIn);
});
```

## App Identity Fields

`AppOptions` includes read-only identity fields that come from the fork's `.env`:

| Field | Source | Purpose |
|-------|--------|---------|
| `appName` | `VITE_APP_NAME` | Display in page heading, title |
| `appDescription` | `VITE_APP_DESCRIPTION` | Page subtitle |
| `saverUrl` | `VITE_SAVER_URL` | Screensaver URL (production) |
| `saverUrlDebug` | `VITE_SAVER_URL_DEBUG` | Screensaver URL (debug mode) |
| `optionsUrl` | `VITE_OPTIONS_URL` | This options page URL |

These are set by the backend from `.env` and **cannot be changed** by `setOptions()`. Use them for branding:

```javascript
store.signal.effect((opts) => {
  if (!opts) return;
  document.getElementById('app-name').textContent = opts.appName;
  document.title = `${opts.appName} Options`;
});
```

## Custom Options

Forks can define custom key/value fields that get appended to the screensaver URL as query parameters:

```javascript
// In your options page logic
const CUSTOM_FIELDS = [
  { key: 'theme', label: 'Theme', type: 'text', defaultValue: 'dark' },
  { key: 'speed', label: 'Speed', type: 'number', defaultValue: 1.0, min: 0.1, max: 5, step: 0.1 },
];

await liminalAPI.setOptions({
  // ...mandatory fields...
  customOptions: { theme: 'dark', speed: 1.5 },
});
```

The screensaver URL will include `?theme=dark&speed=1.5`.

## Optional User Fields

Two fields are user-settable but optional in `setOptions()` payloads — omit them
and the current value is kept:

| Field | Notes |
|-------|-------|
| `notificationsEnabled` | Opt-in consent for feed notifications. Only meaningful when the fork sets a notification feed URL (`opts.notificationUrl` is non-empty); hide the control otherwise. Nothing is ever shown while this is `false`. |
| `autostart` | Start Liminal at login. The OS login item is the source of truth — the backend applies the change and reports back what the OS accepted, so read the value back with `getOptions()` rather than assuming the write stuck. |

```javascript
// Only offer the notification toggle when the fork configured a feed
store.signal.effect((opts) => {
  if (!opts) return;
  notificationRow.hidden = !opts.notificationUrl;
  notificationToggle.checked = opts.notificationsEnabled;
  autostartToggle.checked = opts.autostart;
});
```

## App Updates

```javascript
liminalAPI.onUpdateAvailable((info) => {
  updateBanner.textContent = `Version ${info.version} is available`;
  updateBanner.hidden = false;
});

checkBtn.addEventListener('click', async () => {
  const update = await liminalAPI.checkForUpdates();
  if (!update) return liminalAPI.showMessage('You are up to date.');
  if (await liminalAPI.ask(`Install v${update.version}? The app will restart.`)) {
    await liminalAPI.installUpdate();
  }
});
```

## Dialogs

Inside Tauri, `ask()` and `showMessage()` use native OS dialogs via `tauri-plugin-dialog`. In browser fallback mode, they use `confirm()` and `alert()`.

```javascript
// Confirmation dialog
const confirmed = await liminalAPI.ask('Are you sure?', {
  title: 'Confirm',
  kind: 'warning',
  okLabel: 'Yes',
  cancelLabel: 'No',
});

// Message dialog
await liminalAPI.showMessage('Settings saved!', {
  title: 'Success',
  kind: 'info',
});
```

**Important:** Always use these methods instead of native `confirm()` / `alert()`. Tauri v2's WKWebView silently suppresses native JavaScript dialogs.

## Links Out of the Options Page

Your page runs inside a webview, so a normal link replaces the options UI and
there's no back button to return. Route every outbound link through `openUrl()`:

```javascript
document.querySelectorAll('a[href^="http"]').forEach((a) => {
  a.addEventListener('click', (e) => {
    e.preventDefault();
    liminalAPI.openUrl(a.href);
  });
});
```

This needs the `opener:default` permission on the options window. Liminal Screen
0.3.0 and newer ship it; if you maintain your own
`src-tauri/capabilities/options.json`, add it there:

```json
{
  "identifier": "options-capability",
  "windows": ["options"],
  "permissions": ["dialog:allow-ask", "dialog:allow-message", "opener:default"]
}
```

Without the permission the call falls back to `window.open()`, which the webview
ignores — the link will simply appear dead, with a warning in the console rather
than a thrown error.

### Remote origins and the ACL

Granting `opener:default` is necessary but *not sufficient*. Tauri scopes every
capability to **local** content unless it declares remote URLs, so a page served
from `http://localhost:4200` or `https://your-domain.com` is denied even when the
permission is listed:

```
opener.open_url not allowed on window "options", webview "options",
URL: http://localhost:4200/

allowed on: [windows: "main", URL: local], [windows: "options", URL: local]
```

This affects every core and plugin command the page uses — `openUrl()`,
`ask()`, `showMessage()` and the event subscription behind `startAutoSync()`.
App-defined commands (`getOptions`, `setOptions`, `previewScreensaver`, …) are
not ACL-gated, which is why they keep working and the failure looks selective.

Liminal Screen 0.3.0+ handles this for you: because `VITE_OPTIONS_URL` is only
known at runtime, the app registers a capability for your options page's origin
when it opens the window. If you maintain your own fork of the Rust code, either
do the same or declare the origin statically:

```json
{
  "identifier": "options-capability",
  "windows": ["options"],
  "remote": { "urls": ["https://your-domain.com"] },
  "permissions": ["dialog:allow-ask", "dialog:allow-message", "opener:default"]
}
```

The grant is matched per origin (scheme + host + port), and paths, query strings
and fragments are wildcarded — so SPA routing on the same origin keeps working.
A static list can't adapt to a `.env`-configured URL, which is why the runtime
grant exists.

## Screensaver Preview

```javascript
$('preview-btn').addEventListener('click', async () => {
  try {
    await liminalAPI.previewScreensaver();
  } catch (e) {
    // e.g. debug mode is on but the fork set no VITE_SAVER_URL_DEBUG
    await liminalAPI.showMessage(e.message, { title: 'Preview', kind: 'error' });
  }
});
```

The preview opens the saver URL for the *current* `debug` setting in an 800×600
window. Save the form before previewing if you want the user's unsaved changes
reflected — the backend reads persisted options, not your form state.

## Closing the Options Window

The window has its own close button, but a "Done" action on the page is often
better UX — especially with auto-save, where the user has no other signal that
they're finished:

```javascript
$('done-btn').addEventListener('click', async () => {
  await store.save(collectForm());   // unsaved state dies with the window
  await liminalAPI.closeOptions();
});
```

Don't reach for `window.close()` or `getCurrentWindow().close()`: the former is
ignored in a webview, and the latter needs `core:window:allow-close` granted to
your page's origin. `closeOptions()` goes through an app command, which isn't
ACL-gated. Requires Liminal Screen 0.3.0+.

## Environment Detection

```javascript
if (liminalAPI.isInTauri) {
  // Real Tauri IPC — full functionality
} else {
  // Browser — mock data, console logging, no preview
}
```

## Error Handling

```javascript
import { LiminalAPIError } from '@liminal-screen/api';

try {
  await liminalAPI.setOptions(options);
} catch (error) {
  if (error instanceof LiminalAPIError) {
    await liminalAPI.showMessage(`Failed to save: ${error.message}`, { kind: 'error' });
  }
}
```

## Deployment

1. Build and host your options page at a public HTTPS URL
2. Set `VITE_OPTIONS_URL` in your `.env`:

```bash
VITE_OPTIONS_URL="https://your-domain.com/options.html"
```

3. Rebuild the Liminal Screen app — it will load your options page in the options window

The backend appends `appName` and `appDescription` as query parameters to your URL, so you can also read them from `URLSearchParams` if needed (though using the API is simpler).

## Reference Implementation

See `examples/remote-options/` in the `@liminal-screen/api` package for a complete, production-ready options page with:

- Reactive store integration
- Form validation
- Native dialog confirmation for reset
- Custom fields support
- Service worker for offline caching
- Identity branding from `opts.appName` / `opts.appDescription`

## Security

- All IPC is sandboxed by Tauri
- Remote options pages cannot access sensitive system APIs directly
- Identity fields are read-only — backend ignores user-submitted values for `saverUrl`, `appName`, etc.
- Dialog permissions (`dialog:allow-ask`, `dialog:allow-message`) must be granted in Tauri capability files