# Rethink: Delegate Reset Responsibility to the Page

**Created:** 2026-04-18

---

## The Problem With the Native Approach

The `PLAN.md` approach (WKWebsiteDataStore, WebView2, webkit2gtk) has platform-specific complexity and gaps — Windows and Linux are hard or impossible to cover cleanly without new dependencies and significant native code per platform.

## Alternative Idea: Let the Page Handle Its Own Cleanup

Instead of the app trying to reach into the WebView's storage from the outside, **flip the responsibility** — tell the loaded page that a reset was requested, and let the page clear its own storage when it next opens.

### How It Could Work

1. When `factory_reset_options` is called, the app sets a **one-shot signal** — something the page will read the next time it loads.
2. The page detects the signal on startup, clears its own `localStorage`, unregisters its service workers, purges its caches — whatever it needs to do.
3. The signal is then consumed (cleared) so it only fires once.

### Where to Put the Signal

**Option A — URL parameter**
Append a query param to the saver/options URL on the next navigation:
`https://saver.example.com?__reset=1`
The page reads `new URLSearchParams(location.search).get('__reset')` and cleans up if truthy.

The app controls the URL, so it can append/remove the param. Store a "pending reset" boolean in `AppState` (not persisted), and inject the param for one navigation only.

**Option B — `navigator` attribute (already have the mechanism)**
We already inject `navigator.id` via `initialization_script`. A similar injection could set `navigator.__pendingReset = true` for a single run. The app tracks in `AppState` whether a reset is pending, injects the flag once, then clears the pending state.

**Option C — Tauri event**
Emit a Tauri event (`__liminal:reset`) to the window once it's open. The page listens for it and handles cleanup. Simpler for pages that use the liminal-api SDK.

### What "One-Shot" Means

- The signal (URL param, navigator flag, or event) must only fire on the **first load after reset**, not on every subsequent load.
- A simple boolean `pending_reset: bool` in `AppState` (non-persisted, so it resets to `false` on restart) is enough. Set it to `true` in `factory_reset_options`, read and clear it when the next window is created or navigated.

### Why This Is Better

- **No platform-specific code** — works identically on macOS, Windows, Linux.
- **No native WebView API archaeology** — the page's own JS cleans up what the page put there.
- **Fork-developer friendly** — the remote options page and screensaver page already use the liminal-api SDK; adding an `onReset` callback to the SDK is a clean extension.
- **More complete** — the page knows exactly what it stored (IndexedDB, custom caches, etc.) and can clean everything, not just what the native API covers.

### Open Questions to Resolve

- Which signal mechanism fits best with the existing `initialization_script` / `navigator.id` pattern?
- Should this be part of the `liminal-api` SDK contract (`LiminalAPI.onReset(callback)`) or a lower-level URL convention?
- How to handle the case where the window is open when reset happens vs. closed? (Open: emit event immediately. Closed: set pending flag, inject on next open.)
- Should the signal be a hard "you must clean up now" or a softer "reset was requested, here's the new `instance_id` — clean up if yours differs"? The new `instance_id` on `navigator.id` after reset could itself serve as the signal, since the page can compare the stored ID with the current one.

### The Instance ID as the Signal

This last point is worth highlighting: **`navigator.id` already changes on factory reset** (a new UUID is generated). If the page stores the last-seen `instance_id` in its own `localStorage`, it can detect a mismatch on load and self-clean:

```javascript
const storedId = localStorage.getItem('__liminal_instance_id');
if (storedId && storedId !== navigator.id) {
    // Instance ID changed → factory reset happened → clean up
    localStorage.clear();
    navigator.serviceWorker?.getRegistrations()
        .then(regs => regs.forEach(r => r.unregister()));
}
localStorage.setItem('__liminal_instance_id', navigator.id);
```

No extra signaling needed — the changed `navigator.id` IS the signal. This is elegant and worth exploring as the primary approach.
