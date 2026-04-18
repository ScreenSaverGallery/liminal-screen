# Remote Options Window — Implementation Summary

**Commit:** `50a8e3fb9a34b0f19e1f219ef4be2cda3ead39cd`  
**Date:** 2026-04-18  
**Status:** ✅ Implemented

---

## What Was Implemented

### 1. Backend (`src-tauri/src/lib.rs`)

**AppOptions struct extended:**
- Added `app_name: String` — read from `VITE_APP_NAME` env var
- Added `app_description: String` — read from `VITE_APP_DESCRIPTION` env var
- Added `custom_options: serde_json::Value` — persisted as JSON blob in `options.json`

**Persistence layer:**
- `load_persisted_options()` now loads `customOptions` from store
- `set_options()` persists `customOptions` JSON key
- `factory_reset_options()` clears `customOptions`
- Identity fields (`app_name`, `app_description`, URLs) are NEVER persisted — always from `.env`

**Window opening:**
- `open_options_window()` appends app identity to URL:
  ```
  https://example.com/options?appName=MyApp&appDescription=My%20Description
  ```

**Dependencies:**
- Added `urlencoding` crate to `Cargo.toml`

---

### 2. Screensaver Engine (`src-tauri/src/screensaver_engine.rs`)

**`get_saver_url()` updated:**
- Appends `custom_options` as URL query params to the screensaver URL
- Example: `https://saver.example.com?theme=dark&location=lobby`
- Handles empty custom_options gracefully (no params appended)

---

### 3. Frontend Storage (`src/app/storage/storage.ts`)

**Type updates:**
- `RemoteOptions` → `CustomOptions` (semantic clarity)
- `AppOptions` interface now includes:
  - `appName`, `appDescription` (read-only identity)
  - `saverUrl`, `saverUrlDebug`, `optionsUrl` (read-only URLs)
  - `customOptions: CustomOptions` (user-settable fork-defined fields)

---

### 4. liminal-api (`packages/liminal-api/`)

**Redesigned API surface:**
- New types in `src/types.ts`: `AppInfo`, `MandatoryOptions`, `CustomOptions`, `LiminalOptions`, `OptionsUpdate`
- Removed `authToken` from all public method signatures
- `LiminalAPI.getOptions()` returns full `LiminalOptions` structure
- `LiminalAPI.setOptions(update: OptionsUpdate)` accepts separated mandatory/custom updates
- `SecurityManager` moved to separate `security.ts` (opt-in only)

**Build updates:**
- UMD bundle target added for CDN/script-tag usage
- `dist/sw.js` — general-purpose service worker template

---

### 5. Reference Options Page (`options/`)

**New example implementation:**
- `options/index.html` — minimal HTML page using liminal-api
- `options/main.ts` — demonstrates all 5 mandatory fields + custom section
- `options/sw.js` — service worker for offline support
- `options/package.json` — standalone build config

**Features:**
- Reads `?appName` from URL for immediate page title
- Uses `liminalAPI.init()` + `getOptions()` for IPC
- Auto-sync on change (debounced) + `beforeunload` final save
- Reset and Preview buttons
- Works in Tauri webview AND regular browser (mock mode)

---

### 6. Service Worker (`options/sw.js`)

**Stale-while-revalidate strategy:**
- Caches options page assets on install
- Serves from cache immediately, refreshes in background
- Clears old caches on version bump
- Fork developers copy `dist/sw.js` and customize `PRECACHE_URLS` + version

---

## Architecture Flow

```
Remote Options Page (webview)
         │
         │ liminalAPI.getOptions()
         ▼
Tauri Backend (lib.rs)
  └─> Returns: AppInfo + MandatoryOptions + CustomOptions
         │
         │ User changes value
         ▼
Tauri Backend (set_options)
  └─> Persists: mandatory fields + customOptions JSON
         │
         │ Screensaver activates
         ▼
Screensaver Engine
  └─> get_saver_url() → base URL + ?custom=params
```

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Identity fields from `.env` only | Forks control branding; users shouldn't override app name |
| Custom options as JSON blob | Flexible schema — fork devs define their own fields |
| Custom options as URL params (no prefix) | Fork devs control field names; no namespace pollution |
| Auto-save on change + beforeunload | Matches "open/close saves" UX expectation |
| Service worker owned by fork dev | Tauri can't inject SW into remote origins |
| liminal-api as UMD + ESM | Works for both bundler users and simple `<script>` tag |

---

## Files Modified/Created

| Category | Files |
|----------|-------|
| Backend | `src-tauri/src/lib.rs`, `src-tauri/src/screensaver_engine.rs`, `src-tauri/Cargo.toml` |
| Frontend Storage | `src/app/storage/storage.ts`, `src/app/options/options.ts` |
| liminal-api | `packages/liminal-api/src/index.ts`, `src/types.ts`, `src/security.ts`, `package.json` |
| Reference Page | `options/index.html`, `options/main.ts`, `options/sw.js`, `options/package.json` |
| App Entry | `src/main.ts` (updated to use new storage types) |

---

## Validation

**Backend validation added:**
```rust
fn validate_options(options: &AppOptions) -> Result<(), String> {
    // starts_in: 0.1–1440 min
    // display_off_in: 0.5–1440 min
    // require_pass_in: 0–1440 min
}
```

---

## Success Criteria Met

- ✅ Fork developer sets `VITE_OPTIONS_URL` → page loads in webview
- ✅ Options page reads options via `liminalAPI.getOptions()`
- ✅ Options page writes back; changes take effect without restart
- ✅ Custom options appear as URL query params on screensaver
- ✅ Options page works offline after first load (SW cached)
- ✅ `appName`/`appDescription` accessible via URL params + IPC
- ✅ liminal-api works as `<script>` tag (UMD bundle)
- ✅ Reference implementation demonstrates all mandatory fields + custom section

---

## Next Steps (Optional Enhancements)

1. Add visual feedback for sync status (saving/saved/error states)
2. Implement security module (auth tokens) if multi-tenant deployment needed
3. Add unit tests for `get_saver_url()` param encoding
4. Document fork developer workflow in README
