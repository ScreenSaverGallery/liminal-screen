# Factory Reset — Browser Storage Cleanup

**Created:** 2026-04-18  
**Status:** Draft

---

## Problem

`factory_reset_options` currently clears only the Tauri-side `options.json` store and resets in-memory state. Remote URLs loaded by screensaver windows (`VITE_SAVER_URL`) and the options window (`VITE_OPTIONS_URL`) may have written their own data to the WebView's browser storage (localStorage, sessionStorage, Cache API, service worker registrations). A factory reset should clean those too so the fork's remote pages start fresh.

---

## Current State

| What gets cleared today | What does NOT get cleared |
|-------------------------|---------------------------|
| `options.json` (Tauri store) | Remote page `localStorage` |
| In-memory `AppState.options` | Remote page `sessionStorage` |
| | Cache API entries |
| | Registered service workers |

---

## Plan

### Phase 1 — Add constants

**Location:** `src-tauri/src/lib.rs`, after line 34 (after `MAIN_WINDOW_LABEL`)

```rust
/// Window label prefix for all screensaver display windows
const SAVER_WINDOW_PREFIX: &str = "saver-display-";
```

```rust
/// JavaScript injected into open windows during factory reset.
/// Clears localStorage, sessionStorage, Cache API entries, and
/// unregisters all service workers. Each block is isolated in
/// its own try/catch — a failure in one does not abort the others.
/// Uses ES5 callbacks (not async/await) for broadest WebKit compatibility.
const BROWSER_STORAGE_CLEANUP_JS: &str = r#"
(function() {
    try { localStorage.clear(); } catch(e) {}
    try { sessionStorage.clear(); } catch(e) {}
    try {
        if ('caches' in self) {
            caches.keys().then(function(keys) {
                keys.forEach(function(key) { caches.delete(key); });
            });
        }
    } catch(e) {}
    try {
        if (navigator.serviceWorker) {
            navigator.serviceWorker.getRegistrations().then(function(regs) {
                regs.forEach(function(reg) { reg.unregister(); });
            });
        }
    } catch(e) {}
})();
"#;
```

### Phase 2 — Add `clean_browser_storage` helper

Insert before `factory_reset_options` in `src-tauri/src/lib.rs`:

```rust
/// Injects browser-storage cleanup JS into all currently-open relevant windows.
/// Best-effort: silently skips windows that are not open. Never propagates errors.
///
/// Windows cleaned:
/// - `OPTIONS_LABEL` ("options") — if open
/// - All `saver-display-*` windows — if any are open
///
/// See LIMITATIONS below.
fn clean_browser_storage<R: Runtime>(app: &AppHandle<R>) {
    // Options window
    if let Some(window) = app.get_webview_window(OPTIONS_LABEL) {
        if let Err(e) = window.eval(BROWSER_STORAGE_CLEANUP_JS) {
            eprintln!("[factory_reset] browser cleanup failed for '{}': {}", OPTIONS_LABEL, e);
        }
    }

    // All open saver-display-* windows
    for (label, window) in app.webview_windows() {
        if label.starts_with(SAVER_WINDOW_PREFIX) {
            if let Err(e) = window.eval(BROWSER_STORAGE_CLEANUP_JS) {
                eprintln!("[factory_reset] browser cleanup failed for '{}': {}", label, e);
            }
        }
    }
}
```

No new imports needed — `Manager` (which provides `get_webview_window` + `webview_windows`) and `Runtime` are already in scope.

### Phase 3 — Update `factory_reset_options`

```rust
#[tauri::command]
fn factory_reset_options<R: Runtime>(app: AppHandle<R>, state: tauri::State<AppState>) -> Result<AppOptions, String> {
    // Clear and persist the options store
    let store = app.store("options.json").map_err(|e| format!("Failed to open store: {}", e))?;
    store.clear();
    store.save().map_err(|e| format!("Failed to save reset: {}", e))?;

    // Reset in-memory state to defaults
    let default_options = AppOptions::default();
    {
        let mut current = state.options.lock().unwrap();
        *current = default_options.clone();
    } // Lock released here — must be before eval to avoid deadlock

    // Clean browser-side storage in any currently-open windows (best effort)
    clean_browser_storage(&app);

    Ok(default_options)
}
```

The explicit scope block on the mutex guard is important: `clean_browser_storage` calls `app.webview_windows()` and `window.eval()` — code triggered by eval completions may eventually re-read `AppState.options`. Holding the lock across eval calls creates a deadlock risk. The guard is dropped before the cleanup call.

---

## Files Touched

| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Add `SAVER_WINDOW_PREFIX` constant, `BROWSER_STORAGE_CLEANUP_JS` constant, `clean_browser_storage` helper, update `factory_reset_options` body |

No other files are modified. No new Cargo dependencies.

---

## Limitations

**1. Closed windows are not cleaned.**
`window.eval()` only works on open, loaded `WebviewWindow`s. Screensaver windows (`saver-display-*`) are typically closed when factory reset is triggered (screensaver inactive). Their remote-origin localStorage/Cache data persists in the WKWebView on-disk data store until those windows are opened again. A complete purge would require calling the native `WKWebsiteDataStore` API (macOS) directly — not exposed by Tauri's public surface.

**2. Service worker unregistration is fire-and-forget.**
`navigator.serviceWorker.getRegistrations()` returns a Promise. `factory_reset_options` returns before those promises resolve. If the window closes before resolution, the service worker may not be unregistered in that session.

**3. Preview windows excluded.**
`preview-{timestamp}` windows are transient and user-initiated. They are not covered. If needed, add `|| label.starts_with("preview-")` to the loop condition.

**4. Main window excluded intentionally.**
The `"main"` window loads a local Tauri asset — no localStorage or service workers. Cleaning it is a no-op.

---

## Verification

- [ ] `cargo check` passes with no new warnings
- [ ] Factory reset still clears `options.json` and returns `AppOptions::default()`
- [ ] When options window is **closed** at reset time: no panic, eval skipped cleanly
- [ ] When options window is **open** at reset time: open its devtools → call `factory_reset_options` → confirm `localStorage` is empty
- [ ] When no saver windows are open: loop finds no `saver-display-*` labels, no errors logged
- [ ] When saver windows are open: confirm they receive the eval and storage is cleaned
- [ ] No double-eval on the options window: `"options"` does not match `SAVER_WINDOW_PREFIX`
- [ ] `eprintln!` errors appear in Tauri console when eval fails; never cause the command to return `Err`
