# "Reset to Defaults" Button Bug — Full Debug Summary

**Date:** 2026-04-18  
**Symptom:** Clicking "Reset to Defaults" does nothing — no confirm dialog, no form changes.  
**Context:** Save works correctly. No console errors.

---

## Diagnosis

Three interacting bugs caused the Reset button to be completely non-functional:

### Bug 1: `<button>` missing `type="button"` → form submission on click

Both `index.html` and the remote options HTML had `<button>` elements **without an explicit `type` attribute**:

```html
<!-- BEFORE — defaults to type="submit" -->
<button id="reset-btn" class="btn btn-danger">Reset to Defaults</button>
```

By HTML spec, `<button>` defaults to `type="submit"`. Even though these buttons sat outside the `<form>`, WebKit/WKWebView (which Tauri uses on macOS) can associate nearby buttons with forms. Clicking Reset triggered a form submission/page reload instead of running the JavaScript click handler.

```html
<!-- AFTER -->
<button id="reset-btn" type="button" class="btn btn-danger">Reset to Defaults</button>
```

**Why Save "worked":** The `change` event listener on inputs auto-saves on blur (`saveOptions(true)` in silent mode). Users editing a value and clicking away triggered the save. The explicit Save button was just as broken as Reset if clicked directly — but nobody noticed because auto-save masked it.

### Bug 2: Tauri v2 WKWebView silently suppresses `confirm()` and `alert()`

Even after fixing Bug 1, the Reset handler calls `confirm("Reset all options to defaults?")`. Tauri v2's WKWebView on macOS **silently swallows native JavaScript `confirm()` and `alert()` calls** by default. The dialog never appears, the function returns `undefined` (falsy), and the Reset logic short-circuits at the `if (!confirm(...))` guard.

This is by design in Tauri v2 — native dialogs are blocked for security/UX reasons. The proper solution is `tauri-plugin-dialog`, which provides native OS-level dialogs through Tauri's IPC.

### Bug 3: Double `init()` in `src/main.ts`

```js
window.addEventListener("DOMContentLoaded", () => {
  // ... setup ...
  init();
});

// ALSO runs immediately, BEFORE DOMContentLoaded:
try { init().catch(console.error); } catch (error) { ... }
```

This caused duplicate Tauri event listeners and the first `options.effect()` firing before DOM elements were cached (no-op). Not the direct cause of the reset bug, but created confusing double-fire behavior.

---

## Fix Applied

### 1. Added `type="button"` to all `<button>` elements (both HTML files)

Prevents form submission behavior on click.

### 2. Installed `tauri-plugin-dialog` and replaced native dialogs

**Rust side:**
- Added `tauri-plugin-dialog = "2"` to `Cargo.toml`
- Registered `.plugin(tauri_plugin_dialog::init())` in `lib.rs`
- Added permissions to both capability files

**Local main window (`src/main.ts`):**
```ts
import { ask, message } from "@tauri-apps/plugin-dialog";

// Reset handler:
const confirmed = await ask("Reset all options to defaults?", {
  title: "Reset", kind: "warning", okLabel: "Reset", cancelLabel: "Cancel"
});

// Save handler:
await message("Settings saved successfully!", { title: "Saved", kind: "info" });
```

**Remote options window (`liminal-api`):**
```ts
// Uses __TAURI__.dialog (withGlobalTauri: true — no npm import needed)
const tauriDialog = () => (window as any).__TAURI__.dialog;

async ask(message: string, options?: Record<string, unknown>): Promise<boolean> {
  return tauriDialog().ask(message, options ?? { title: "Confirm", kind: "warning" });
}

async showMessage(message: string, options?: Record<string, unknown>): Promise<void> {
  return tauriDialog().message(message, options ?? { title: "Info", kind: "info" });
}
```

### 3. Added `initialized` guard to `init()` in `src/main.ts`

```ts
let initialized = false;

async function init(): Promise<void> {
  if (initialized) return;
  initialized = true;
  // ... rest of init
}
```

### 4. Fixed invalid capability permission

Initially added `dialog:allow-ok` which doesn't exist in Tauri v2's dialog plugin. The valid permissions are:

| Permission | Purpose |
|---|---|
| `dialog:allow-ask` | Yes/No confirmation dialog |
| `dialog:allow-confirm` | Confirm dialog |
| `dialog:allow-message` | Alert/message dialog |
| `dialog:allow-open` | File open dialog |
| `dialog:allow-save` | File save dialog |
| `dialog:default` | All of the above |

Only `dialog:allow-ask` and `dialog:allow-message` were needed. Removed `dialog:allow-ok` from both `default.json` and `options.json`.

---

## Files Modified

| File | Change |
|---|---|
| `index.html` | `type="button"` on save, preview, reset buttons |
| `packages/liminal-api/examples/remote-options/index.html` | `type="button"` on save, preview, reset buttons |
| `src/main.ts` | Import `ask`/`message` from dialog plugin, replace `confirm()` → `ask()`, `alert()` → `message()`, add `initialized` guard |
| `package.json` / `bun.lock` | Added `@tauri-apps/plugin-dialog` |
| `src-tauri/Cargo.toml` | Added `tauri-plugin-dialog = "2"` |
| `src-tauri/src/lib.rs` | Added `.plugin(tauri_plugin_dialog::init())` |
| `src-tauri/capabilities/default.json` | Added `dialog:allow-ask`, `dialog:allow-message` |
| `src-tauri/capabilities/options.json` | Added `dialog:allow-ask`, `dialog:allow-message` |
| `packages/liminal-api/src/index.ts` | Added `tauriDialog()` helper, `ask()`, `showMessage()` methods |
| `packages/liminal-api/examples/remote-options/main.ts` | Replaced `confirm()` → `api.ask()`, `alert()` → `api.showMessage()` |

---

## Key Takeaway

When a button does nothing in a Tauri v2 app — no errors, no console output — check two things:

1. **Does the button have `type="button"`?** Without it, `<button>` defaults to `type="submit"` and clicks trigger form submission instead of your handler.
2. **Does the handler use `confirm()` or `alert()`?** Tauri v2's WKWebView silently suppresses native JS dialogs. Use `tauri-plugin-dialog` and add the appropriate capability permissions (`dialog:allow-ask`, `dialog:allow-message`, etc.).

Both bugs were silent — no error, no console output, no visible feedback. The button click either (a) submitted the form and reloaded the page, or (b) had its confirm dialog swallowed by the webview. Either way: zero visible effect.