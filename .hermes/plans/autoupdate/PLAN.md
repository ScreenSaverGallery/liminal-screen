# Autoupdate — Frontend Integration

**Created:** 2026-04-18  
**Status:** Implemented — see IMPLEMENTATION_SUMMARY.md

---

## Problem

The Rust backend already checks for updates silently on startup (`updater.rs`), but there is no way for the user to:
- Manually trigger an update check
- Know when an update is available
- Choose when to install it

The tray menu has no "Check for Updates" item, the options window has no update status UI, and the `liminal-api` SDK exposes no update methods to remote options pages.

---

## Current State

| What exists | What is missing |
|-------------|-----------------|
| `tauri-plugin-updater = "2"` in `Cargo.toml` | No `check_for_updates` / `install_update` Tauri commands |
| Full updater config in `tauri.conf.json` (endpoint, key) | No update-related events emitted to frontend |
| `updater.rs` — silent auto-check on startup | No tray menu item |
| `@tauri-apps/plugin-updater` in `package.json` | No options window UI |
| | No liminal-api methods |

---

## Plan

### Phase 1 — Extend `src-tauri/src/updater.rs`

Refactor the existing file into three public functions:

```rust
/// Silent background check run at startup — checks + installs without user interaction.
pub async fn update_silent<R: tauri::Runtime>(app: tauri::AppHandle<R>)
    -> tauri_plugin_updater::Result<()>

/// User-triggered check — emits events so the frontend can react.
/// Emits `update-available` with UpdateInfo payload, or `update-not-available`.
pub async fn check_update<R: tauri::Runtime>(app: tauri::AppHandle<R>)
    -> tauri_plugin_updater::Result<bool>

/// Download + install — emits `update-download-progress` per chunk, then `update-installed`.
/// Calls app.restart() on completion.
pub async fn download_and_install<R: tauri::Runtime>(app: tauri::AppHandle<R>)
    -> tauri_plugin_updater::Result<()>
```

Event payloads (emitted via `app.emit(...)`):

| Event | Payload |
|-------|---------|
| `update-available` | `{ version: String, notes: Option<String> }` |
| `update-not-available` | `{}` |
| `update-download-progress` | `{ downloaded: usize, total: Option<u64> }` |
| `update-installed` | `{}` |

Rename the existing `update()` call site in `lib.rs` `setup_app` to `update_silent`.

### Phase 2 — Add Tauri commands in `src-tauri/src/lib.rs`

```rust
#[tauri::command]
async fn check_for_updates(app: AppHandle<impl Runtime>) -> Result<bool, String> {
    updater::check_update(app).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn install_update(app: AppHandle<impl Runtime>) -> Result<(), String> {
    updater::download_and_install(app).await
        .map_err(|e| e.to_string())
}
```

Register both in `invoke_handler` alongside existing commands.

### Phase 3 — Add tray menu item in `src-tauri/src/lib.rs`

In `create_tray()`, add between the Preview and Quit items:

```rust
let check_updates_i = MenuItem::with_id(app, "check-updates", "Check for Updates", true, None::<&str>)?;
```

Add to `Menu::with_items`: `&[&options_i, &preview_i, &check_updates_i, &quit_i]`

Add handler arm in `on_menu_event`:
```rust
"check-updates" => {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = updater::check_update(handle).await {
            eprintln!("[updater] Manual check failed: {}", e);
        }
    });
}
```

### Phase 4 — Options window UI in `index.html`

Add an update row in the info/about section (before the `#saver-url-display` row):

```html
<div class="info-row" id="update-section">
  <span class="info-label">Updates</span>
  <span id="update-status-text">Up to date</span>
  <button id="check-updates-btn" class="btn-secondary btn-small">Check</button>
  <button id="install-update-btn" class="btn-primary btn-small" hidden>Install</button>
</div>
```

### Phase 5 — `src/main.ts`

Add update state signal and reactive effect:

```typescript
interface UpdateInfo { version: string; notes?: string }
const updateAvailable = new Signal<UpdateInfo | null>(null);
```

Add event listeners in `setupEventListeners()`:
```typescript
listen<UpdateInfo>("update-available", (e) => updateAvailable.set(e.payload));
listen("update-not-available", () => updateAvailable.set(null));
listen("update-installed", () => updateAvailable.set(null));
```

Wire up buttons in `setupUIButtonHandlers()`:
```typescript
document.getElementById("check-updates-btn")
  ?.addEventListener("click", () => invoke("check_for_updates"));
document.getElementById("install-update-btn")
  ?.addEventListener("click", () => invoke("install_update"));
```

Reactive effect:
```typescript
updateAvailable.effect((info) => {
  const statusEl = document.getElementById("update-status-text");
  const installBtn = document.getElementById("install-update-btn") as HTMLButtonElement | null;
  if (statusEl) statusEl.textContent = info ? `v${info.version} available` : "Up to date";
  if (installBtn) installBtn.hidden = !info;
});
```

### Phase 6 — `packages/liminal-api/src/types.ts`

```typescript
export interface UpdateInfo {
  version: string;
  notes?: string;
}
```

### Phase 7 — `packages/liminal-api/src/index.ts`

```typescript
async checkForUpdates(): Promise<UpdateInfo | null> {
  const available = await this._invoke<boolean>("check_for_updates");
  // The Rust command emits `update-available` if true — SDK consumers listen to that event.
  return available ? this._lastUpdateInfo : null;
}

onUpdateAvailable(callback: (info: UpdateInfo) => void): () => void {
  return this._listen<UpdateInfo>("update-available", callback);
}
```

---

## Files Touched

| File | Change |
|------|--------|
| `src-tauri/src/updater.rs` | Add `check_update`, `download_and_install`; rename `update` → `update_silent` |
| `src-tauri/src/lib.rs` | Add `check_for_updates` + `install_update` commands; tray menu item + handler; update `update_silent` call |
| `index.html` | Add `#update-section` row |
| `src/main.ts` | Add `updateAvailable` Signal, event listeners, button handlers, reactive effect |
| `packages/liminal-api/src/types.ts` | Add `UpdateInfo` type |
| `packages/liminal-api/src/index.ts` | Add `checkForUpdates()`, `onUpdateAvailable()` |

No new Cargo or npm dependencies needed — `tauri-plugin-updater` is already present.

---

## Verification

- [ ] `cargo check` passes with no new errors
- [ ] Startup silent auto-check still works (no regressions)
- [ ] Tray menu shows "Check for Updates" between Preview and Quit
- [ ] Clicking "Check for Updates" in tray triggers a check; console shows updater output
- [ ] When up to date: options window shows "Up to date", Install button hidden
- [ ] When update available: `update-available` event fires, options window shows version, Install button appears
- [ ] Clicking Install invokes `install_update`; app restarts after download completes
- [ ] `LiminalAPI.checkForUpdates()` returns `UpdateInfo | null` from remote options page
- [ ] `LiminalAPI.onUpdateAvailable(cb)` fires when update is found
