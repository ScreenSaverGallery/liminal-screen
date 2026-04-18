# Remote Options Window

**Created:** 2026-04-17  
**Status:** Draft — Pending Review  
**Priority:** High (core user-facing feature)

---

## Executive Summary

A fork developer hosts a settings page at `VITE_OPTIONS_URL`. The Tauri app opens it in a webview window. That page uses the `liminal-api` library to read/write options via IPC. Mandatory options drive engine behaviour; custom options (fork-defined) become URL query params appended to the screensaver URL(s) automatically. The page caches itself via service worker for offline use.

**Three moving parts:**

| Part | Who owns it | Where it lives |
|------|-------------|----------------|
| IPC bridge (`liminal-api`) | This repo | `packages/liminal-api/` |
| Options page | Fork developer | External URL (`VITE_OPTIONS_URL`) |
| Service worker | Fork developer (we supply template) | Same origin as options page |

---

## Current State vs Target

### What exists

- `lib.rs` — `get_options`, `set_options`, `factory_reset_options` commands ✓
- `storage.ts` — `MandatoryOptions`, `RemoteOptions` (freeform), `AppOptions` ✓
- `options.ts` — `getSaverUrl()` that appends remoteOptions as query params ✓
- `remote-options.ts` — rough reference implementation (incomplete: missing `requirePassIn`, no custom fields)
- `sw.js` — basic service worker draft (hardcoded paths, incomplete)
- `liminal-api/` — prototype (API design issues: URLs leak into settable interface, `authToken` noise throughout, no standalone bundle)

### What is missing

1. `app_name` / `app_description` not in `get_options` response
2. `custom_options` not stored in backend `AppOptions` — frontend `Storage.remoteOptions` exists but backend doesn't know about it
3. Screensaver engine doesn't append custom options as URL params
4. `liminal-api` interface is wrong (`setOptions` leaks URLs, mandatory/custom not separated)
5. No standalone/CDN-ready bundle for liminal-api
6. Service worker is a hardcoded stub — not general-purpose
7. The options window URL isn't passed app identity (name/description) for immediate use before IPC

---

## Data Model

### Backend: `AppOptions` additions (Rust)

```rust
pub struct AppOptions {
    // === Existing — fork identity, NOT persisted ===
    pub saver_url: String,
    pub saver_url_debug: String,
    pub options_url: String,

    // === NEW — app identity, NOT persisted (env only) ===
    pub app_name: String,         // VITE_APP_NAME
    pub app_description: String,  // VITE_APP_DESCRIPTION

    // === Existing — mandatory, persisted ===
    pub starts_in: f64,
    pub display_off_in: f64,
    pub require_pass_in: f64,
    pub run_on_battery: bool,
    pub debug: bool,

    // === NEW — custom, persisted as JSON blob ===
    pub custom_options: serde_json::Value,  // {}  on first install
}
```

**Persistence rules:**
- `app_name`, `app_description`, URLs — always from `.env`, never persisted
- `starts_in`, `display_off_in`, `require_pass_in`, `run_on_battery`, `debug` — persisted as individual keys (existing behaviour)
- `custom_options` — persisted as `"customOptions"` key (JSON string)

### Frontend: `storage.ts` (TypeScript)

`Storage` already has the right shape. Rename `RemoteOptions` to `CustomOptions` for clarity:

```typescript
export type CustomOptions = Record<string, string | number | boolean>;
```

The `AppOptions` frontend type becomes:

```typescript
export interface AppOptions {
  // App identity (read-only, from env via backend)
  appName: string;
  appDescription: string;
  // URLs (read-only)
  saverUrl: string;
  saverUrlDebug: string;
  optionsUrl: string;
  // Mandatory (user-settable)
  startsIn: number;
  displayOffIn: number;
  requirePassIn: number;
  runOnBattery: boolean;
  debug: boolean;
  // Custom (fork-defined, user-settable)
  customOptions: CustomOptions;
}
```

---

## Architecture

### Communication flow

```
┌─────────────────────────────────────────────────────────────────────┐
│  REMOTE OPTIONS PAGE (external URL, loaded in Tauri webview)        │
│                                                                     │
│   1. page load → liminalAPI.init()                                 │
│   2. liminalAPI.getOptions()  ─────────────────────────┐           │
│   3. populate form (mandatory + custom fields)         │ Tauri IPC │
│                                                        │           │
│   4. user changes value → auto-sync debounced          │           │
│   5. liminalAPI.setOptions(mandatory, custom) ─────────┤           │
│   6. beforeunload → final sync                         │           │
│                                                        ▼           │
│                              ┌─────────────────────────────────┐   │
│                              │   Tauri Backend (lib.rs)         │   │
│                              │   set_options()                  │   │
│                              │   ├── update AppState (memory)  │   │
│                              │   ├── persist mandatory fields   │   │
│                              │   └── persist customOptions JSON │   │
│                              └─────────────┬───────────────────┘   │
└────────────────────────────────────────────┼───────────────────────┘
                                             │
                              ┌──────────────▼──────────────┐
                              │  Screensaver Engine (Rust)  │
                              │  get_saver_url()            │
                              │  ├── base URL from AppState │
                              │  └── append customOptions   │
                              │      as URL query params    │
                              │                             │
                              │  e.g.                       │
                              │  https://saver.example.com  │
                              │  ?theme=dark&location=lobby │
                              └─────────────────────────────┘
```

### Service worker flow

```
First visit (online):
  Browser → fetches options page from VITE_OPTIONS_URL
  Options page → registers /sw.js at same origin
  SW → caches the full page (HTML + JS + CSS + fonts)

Subsequent visits (offline capable):
  Browser → hits SW
  SW → stale-while-revalidate: serve from cache immediately,
       attempt network refresh in background
  Options page → loads, connects to Tauri IPC normally

Cache invalidation:
  SW version string in sw.js → bump to force re-cache on deploy
```

---

## liminal-api Redesign

### Interface redesign

```typescript
// ── What the page reads from the app ─────────────────────────────────

export interface AppInfo {
  appName: string;
  appDescription: string;
  saverUrl: string;        // read-only, for display only
  saverUrlDebug: string;   // read-only, for display only
  optionsUrl: string;      // read-only
}

export interface MandatoryOptions {
  startsIn: number;        // minutes
  displayOffIn: number;    // minutes
  requirePassIn: number;   // minutes (0 = disabled)
  runOnBattery: boolean;
  debug: boolean;
}

export type CustomOptions = Record<string, string | number | boolean>;

export interface LiminalOptions {
  appInfo: AppInfo;
  mandatory: MandatoryOptions;
  custom: CustomOptions;
}

// ── What the page writes back ─────────────────────────────────────────

export interface OptionsUpdate {
  mandatory?: Partial<MandatoryOptions>;
  custom?: CustomOptions;
}
```

### API surface (public methods)

```typescript
class LiminalAPI {
  // Lifecycle
  async init(): Promise<void>
  destroy(): void

  // Options
  async getOptions(): Promise<LiminalOptions>
  async setOptions(update: OptionsUpdate): Promise<void>
  async resetOptions(): Promise<LiminalOptions>

  // Auto-sync helper
  // getValues: called on change + on beforeunload
  // debounceMs: default 800ms
  startAutoSync(getValues: () => OptionsUpdate, debounceMs?: number): () => void

  // Screensaver
  async previewScreensaver(): Promise<void>

  // Environment
  isInTauri(): boolean
  onOptionsUpdate(callback: (options: LiminalOptions) => void): () => void
}
```

**Removed from public API:**
- `authToken` parameters (all methods) — premature complexity, remove entirely
- `configureSecurity()` / `generateAuthToken()` — move to `security.ts` (opt-in only, not exported from main index)

### Standalone bundle

The library must work without npm install, via a `<script>` tag:

```html
<script src="https://cdn.jsdelivr.net/npm/@liminal-screen/api/dist/liminal-api.umd.js"></script>
<script>
  const api = new LiminalScreen.LiminalAPI();
</script>
```

**Build config:** Add Vite/Rollup UMD build target alongside the existing ESM build. Export `LiminalAPI`, `LiminalAPIError` as named exports on `LiminalScreen` global.

---

## Service Worker

The SW is **not part of the Tauri app** — it lives at the fork developer's origin. The `liminal-api` package ships a pre-built `sw.js` in `dist/sw.js`.

### SW strategy: stale-while-revalidate

```javascript
// dist/sw.js (fork developer serves this at their options page origin)
const CACHE = 'liminal-options-v1'; // fork developer bumps version on deploy

// Install: pre-cache the options page assets
self.addEventListener('install', (e) => {
  e.waitUntil(
    caches.open(CACHE).then(c => c.addAll(PRECACHE_URLS)).then(() => self.skipWaiting())
  );
});

// Activate: clear old caches
self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys()
      .then(keys => Promise.all(keys.filter(k => k !== CACHE).map(k => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

// Fetch: stale-while-revalidate for page assets
self.addEventListener('fetch', (e) => {
  if (e.request.method !== 'GET') return;
  e.respondWith(
    caches.open(CACHE).then(async cache => {
      const cached = await cache.match(e.request);
      const networkFetch = fetch(e.request).then(res => {
        if (res.ok) cache.put(e.request, res.clone());
        return res;
      }).catch(() => null);
      return cached ?? await networkFetch ?? new Response('Offline', { status: 503 });
    })
  );
});
```

**Fork developer setup:** Copy `dist/sw.js` to their options page root. Customize `PRECACHE_URLS` and bump `CACHE` version on each deploy.

---

## App Identity: VITE_APP_NAME / VITE_APP_DESCRIPTION

### Problem

A remote page at `https://example.com/options.html` cannot access `import.meta.env.VITE_APP_NAME` — it has no access to the host app's Vite env. It needs to receive the identity via IPC.

### Solution: two channels

**Channel 1 — URL params when opening the window (immediate, no async)**

```rust
// lib.rs: open_options_window()
let url = format!(
    "{}?appName={}&appDescription={}",
    options_url,
    urlencoding::encode(&options.app_name),
    urlencoding::encode(&options.app_description),
);
```

The page reads them synchronously from `new URLSearchParams(location.search)` before any IPC call.

**Channel 2 — get_options() response**

`AppInfo` is included in the full `LiminalOptions` returned by `liminalAPI.getOptions()`. Useful for displaying app name in the page title, headings, etc. after the async init.

---

## Implementation Phases

### Phase 1: Backend (`src-tauri/src/lib.rs`)

1. Add `app_name`, `app_description` to `AppOptions` struct (read from env, not persisted)
2. Add `custom_options: serde_json::Value` to `AppOptions` struct  
3. Update `AppOptions::default()` to read `VITE_APP_NAME`, `VITE_APP_DESCRIPTION`
4. Update `set_options()` to persist `customOptions` JSON key
5. Update `load_persisted_options()` to load `customOptions` key
6. Update `factory_reset_options()` to clear `customOptions`
7. Update `open_options_window()` to append `?appName=…&appDescription=…` to URL

**Cargo.toml:** Add `urlencoding` crate (or use manual percent-encoding).

### Phase 2: Screensaver engine (`src-tauri/src/screensaver_engine.rs`)

1. Update `get_saver_url()` to append `custom_options` as URL query params

```rust
fn get_saver_url<R: tauri::Runtime>(&self, app: &AppHandle<R>) -> Result<String, String> {
    let state = app.state::<super::AppState>();
    let options = state.options.lock().unwrap();
    let base = if options.debug { &options.saver_url_debug } else { &options.saver_url };
    
    if options.custom_options.is_null() || options.custom_options == serde_json::Value::Object(Default::default()) {
        return Ok(base.clone());
    }
    
    let mut url = base.clone();
    if let Some(obj) = options.custom_options.as_object() {
        let params: Vec<String> = obj.iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v.to_string().trim_matches('"'))))
            .collect();
        if !params.is_empty() {
            url = format!("{}?{}", url, params.join("&"));
        }
    }
    Ok(url)
}
```

### Phase 3: Frontend storage (`src/app/storage/storage.ts`)

1. Rename `RemoteOptions` → `CustomOptions`, tighten type to `Record<string, string | number | boolean>`
2. Update `AppOptions` interface to include `appName`, `appDescription`, `saverUrl`, `saverUrlDebug`, `optionsUrl`
3. Update `getOptions()` to include these from `get_options` invoke response
4. Ensure `setRemoteOptions` → `setCustomOptions` (rename)

### Phase 4: liminal-api redesign (`packages/liminal-api/`)

1. Redesign `LiminalOptions`, `MandatoryOptions`, `CustomOptions`, `OptionsUpdate` interfaces
2. Refactor `LiminalAPI` class:
   - `getOptions()` → maps Rust snake_case response to camelCase `LiminalOptions`
   - `setOptions(update: OptionsUpdate)` → maps back to Rust format
   - `startAutoSync(getValues, debounceMs?)` → attaches change listener + `beforeunload`
   - Remove `authToken` from all method signatures
3. Move `SecurityManager` / `generateAuthToken` to separate `security.ts` (already there, just don't re-export from `index.ts` by default)
4. Add UMD build target (Vite library mode or Rollup):
   - `dist/liminal-api.esm.js` (existing)
   - `dist/liminal-api.umd.js` (new — global `LiminalScreen`)
   - `dist/sw.js` (service worker template)

### Phase 5: Service worker (`packages/liminal-api/dist/sw.js`)

1. Write general-purpose SW for options page offline support
2. Export it as `dist/sw.js` in the liminal-api package
3. Document configuration (PRECACHE_URLS, CACHE version)

### Phase 6: Options page reference implementation

Create `options/` example page (hosted separately, not inside the Tauri app):
- Minimal HTML page using `liminalAPI`
- All 5 mandatory fields with proper labels, types, min/max hints
- Custom fields section: a dynamic key-value editor (fork devs replace with their own form)
- Save/Reset/Preview buttons
- Reads `?appName` param from URL for page title
- Registers SW for offline support
- Should work both inside Tauri webview AND in a regular browser (mock mode)

---

## Files to Modify / Create

| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Add `app_name`, `app_description`, `custom_options` to `AppOptions`; update `set/get/reset_options`, `open_options_window` |
| `src-tauri/src/screensaver_engine.rs` | `get_saver_url()` appends custom_options as URL params |
| `src-tauri/Cargo.toml` | Add `urlencoding` dependency |
| `src/app/storage/storage.ts` | Rename `RemoteOptions` → `CustomOptions`, update `AppOptions` interface |
| `src/app/options/options.ts` | Update `getSaverUrl()` to use `customOptions` key (matches backend) |
| `src/app/remote-options/remote-options.ts` | Rewrite as proper implementation using liminal-api; add `requirePassIn` field |
| `packages/liminal-api/src/index.ts` | Redesign interfaces, remove authToken from API surface |
| `packages/liminal-api/package.json` | Add build script for UMD bundle |
| `packages/liminal-api/dist/sw.js` | New: general-purpose SW template |

**New files to create:**

| File | Purpose |
|------|---------|
| `packages/liminal-api/src/types.ts` | Extracted type definitions |
| `packages/liminal-api/src/sw-template.js` | Service worker source |
| `options/index.html` | Reference options page (hosted externally) |
| `options/main.ts` | Reference options page logic |

---

## Validation (Backend)

Add to `set_options()`:

```rust
fn validate_options(options: &AppOptions) -> Result<(), String> {
    if options.starts_in < 0.1 || options.starts_in > 1440.0 {
        return Err("startsIn must be between 0.1 and 1440 minutes".into());
    }
    if options.display_off_in < 0.5 || options.display_off_in > 1440.0 {
        return Err("displayOffIn must be between 0.5 and 1440 minutes".into());
    }
    if options.require_pass_in < 0.0 || options.require_pass_in > 1440.0 {
        return Err("requirePassIn must be between 0 and 1440 minutes".into());
    }
    Ok(())
}
```

---

## Open Questions / Decisions Needed

| # | Question | Options | Recommendation |
|---|----------|---------|----------------|
| 1 | **liminal-api distribution** | npm only / CDN script tag / both | Both: ESM for bundlers, UMD for `<script>` |
| 2 | **Auto-save trigger** | On-change (debounced) + `beforeunload` / explicit Save button only | On-change + `beforeunload` — matches "open/close saves" requirement |
| 3 | **Custom options URL params prefix** | No prefix (`theme=dark`) / prefixed (`custom_theme=dark`) | No prefix — fork developer controls field names |
| 4 | **Custom options type constraint** | `string \| number \| boolean` only / any JSON value | Primitives only — URL params are strings; objects/arrays cause complexity |
| 5 | **SW ownership** | Tauri app registers it / fork developer registers it / liminal-api provides template | Fork developer registers (Tauri can't inject SW into remote origin). liminal-api ships template |
| 6 | **Reference options page location** | In this repo (`options/`) / separate repo | In this repo — easier to keep in sync with the API |
| 7 | **Validation errors** | Backend returns error / frontend validates before submit | Both: frontend for immediate feedback, backend as authoritative gate |

---

## Success Criteria

- [ ] Fork developer can set `VITE_OPTIONS_URL` in `.env` and their options page loads in the app
- [ ] Options page can read current options (mandatory + custom) via `liminalAPI.getOptions()`
- [ ] Options page can write back options; changes take effect in screensaver engine without restart
- [ ] Custom options appear as URL query params on the active screensaver URL
- [ ] Options page works offline after first load (service worker cached)
- [ ] `appName` and `appDescription` are accessible in the remote page (URL params + IPC)
- [ ] liminal-api works as `<script>` tag (no build step required for simple fork pages)
- [ ] `remote-options.ts` reference implementation demonstrates all 5 mandatory fields + custom section
