# Reset to Defaults — Bug Analysis

**Date:** 2026-04-18  
**Symptom:** Clicking "Reset to Defaults" does nothing — no confirm dialog, no form changes.  
**Context:** Save works correctly. No console errors.

---

## Symptoms (as reported)

1. Form fields don't change at all after clicking Reset
2. The confirm dialog does NOT appear
3. No console errors
4. Defaults are expected to come from `.env`
5. Save button works correctly

## Code Paths Involved

Two separate UIs can show the Reset button:

### A. Local main window (`src/main.ts` + `index.html`)

```js
// src/main.ts lines 123-133
document.getElementById("reset-btn")?.addEventListener("click", async () => {
    if (!confirm("Reset all options to defaults?")) return;
    try {
      await invoke("factory_reset_options");
      options.set(await invoke<AppOptions>("get_options"));
    } catch (error) {
      console.error("Failed to reset options:", error);
      alert("Failed to reset options. Please try again.");
    }
  });
```

### B. Remote options page (`packages/liminal-api/examples/remote-options/main.ts`)

```ts
// remote-options/main.ts lines 192-201
async function reset(): Promise<void> {
  if (!confirm('Reset all options to defaults?')) return;
  try {
    await store.reset();
    setStatus(api.isInTauri, 'Reset to defaults');
  } catch (e) {
    alert(`Failed to reset: ${e}`);
  }
}
```

---

## Root Cause Analysis

### Primary Suspect: `<button>` without `type="button"`

Both `index.html` and the remote options `index.html` have `<button>` elements **without an explicit `type` attribute**:

```html
<!-- index.html line 116 -->
<button id="reset-btn" class="btn btn-danger">Reset to Defaults</button>

<!-- remote-options/index.html line 144 -->
<button class="btn-danger" id="reset-btn">Reset to defaults</button>
```

By HTML spec, `<button>` defaults to `type="submit"`. While these buttons are placed **outside** the `<form id="options-form">`, some browser engines (notably WebKit/WKWebView used by Tauri on macOS) can still associate buttons with nearby forms through heuristics or parent-scope form association.

**If the browser treats the button as a form submit button**, clicking it would:
- Trigger form submission (page navigation/reload)
- The JavaScript click handler **never completes** — `confirm()` is either skipped or its dialog is destroyed by the navigation
- The page reloads to its initial state, so form fields appear unchanged

**Why Save still "works":** The `change` event listener on inputs auto-saves values via `saveOptions(true)` (silent mode). When a user edits a field and clicks away, `change` fires, persisting the value. The user sees their changes saved and assumes the Save button works — but it's the **auto-save on blur** doing the work, not the explicit Save button click.

### Secondary Issue: Double `init()` in `src/main.ts`

```js
window.addEventListener("DOMContentLoaded", () => {
  // ... setup ...
  init();
});

// Also init immediately — runs BEFORE DOMContentLoaded
try { init().catch(console.error); } catch (error) { ... }
```

This causes:
- `setupEventListeners()` registers duplicate Tauri event listeners
- The immediate `init()` sets `options` before DOM elements are cached, so the first effect run is a no-op
- Not the direct cause of the reset bug, but creates confusing double-fire behavior

### Rust Backend: `factory_reset_options` (verified correct)

```rust
// src-tauri/src/lib.rs lines 335-347
fn factory_reset_options<R: Runtime>(app: AppHandle<R>, state: tauri::State<AppState>) -> Result<AppOptions, String> {
    let store = app.store("options.json").map_err(|e| format!("Failed to open store: {}", e))?;
    store.clear();
    store.save().map_err(|e| format!("Failed to save reset: {}", e))?;
    
    let default_options = AppOptions::default();
    let mut current = state.options.lock().unwrap();
    *current = default_options.clone();
    
    Ok(default_options)
}
```

This is correct: clears persistent store, resets in-memory state to `.env` defaults, returns the new defaults.

---

## Proposed Fix — 3 Changes

### 1. Add `type="button"` to ALL buttons in both HTML files

**`index.html`:**
```html
<button id="save-btn" type="button" class="btn btn-primary">Save Settings</button>
<button id="preview-btn" type="button" class="btn btn-secondary">Preview Screensaver</button>
<button id="reset-btn" type="button" class="btn btn-danger">Reset to Defaults</button>
```

**`packages/liminal-api/examples/remote-options/index.html`:**
```html
<button class="btn-primary" id="save-btn" type="button">Save</button>
<button class="btn-secondary" id="preview-btn" type="button">Preview</button>
<button class="btn-danger" id="reset-btn" type="button">Reset to defaults</button>
```

### 2. Remove duplicate `init()` call in `src/main.ts`

Delete the bottom-of-file immediate invocation:
```js
// REMOVE these lines from the end of main.ts:
try {
  init().catch(console.error);
} catch (error) {
  console.error("Immediate init threw error:", error);
}
```

Add a guard inside `init()` to prevent double-registration if needed:
```js
let initialized = false;

async function init(): Promise<void> {
  if (initialized) return;
  initialized = true;
  // ... rest of init
}
```

### 3. Verify: after the fix, test these scenarios

- [ ] Reset button shows confirm dialog
- [ ] Confirming reset updates all form fields to `.env` defaults
- [ ] Cancelling reset leaves form unchanged
- [ ] Save button still works (both explicit click and auto-save on change)
- [ ] Reopening the options window shows reset values (persistence works)
- [ ] Works in both local main window AND remote options window

---

## If Bug Persists After Fix

If `type="button"` doesn't resolve it, next steps:

1. **Add diagnostic `console.log`** as the first line of the click handler:
   ```js
   document.getElementById("reset-btn")?.addEventListener("click", async () => {
     console.log("Reset button clicked");  // DIAGNOSTIC
     if (!confirm("Reset all options to defaults?")) return;
     ...
   ```

2. **Check if `confirm()` is blocked** by Content Security Policy or Tauri webview config — some setups suppress modal dialogs silently.

3. **Check if the element is found** — add `console.log("reset-btn element:", document.getElementById("reset-btn"))` before the listener.

4. **Check if auto-save on `change` races with reset** — the remote options page auto-saves on every `change` event. If a field is focused when Reset is clicked, `change` fires first, auto-saving stale values that could overwrite the reset.