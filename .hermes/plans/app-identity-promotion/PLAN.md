# Plan: Promote VITE_APP_NAME and VITE_APP_DESCRIPTION Across the App

**Date:** 2026-04-18  
**Status:** Planning  

---

## Problem

The `.env` variables `VITE_APP_NAME` and `VITE_APP_DESCRIPTION` are defined and partially consumed by the Rust backend, but the frontend and several OS-level surfaces still show hardcoded "Liminal Screen" strings. Fork developers who change `.env` expect the entire app to reflect their branding, not just the backend data model.

---

## Current State — What's Already Wired

| Surface | Status | How |
|---|---|---|
| Rust `AppOptions::default()` | ✅ Dynamic | Reads `VITE_APP_NAME` / `VITE_APP_DESCRIPTION` env vars |
| Main window title | ✅ Dynamic | `setup_app()` re-reads `VITE_APP_NAME` and calls `window.set_title()` |
| Options window title | ✅ Dynamic | Rust sets `format!("{} Options", app_name)` |
| Remote options page h1 + description | ✅ Dynamic | `setIdentity()` in remote-options/main.ts reads `opts.appName`/`opts.appDescription` |
| Remote options page `<title>` | ✅ Dynamic | `setIdentity()` sets `document.title` |
| Rust query params to remote URL | ✅ Dynamic | Appends `appName` + `appDescription` as URL params |
| `set_options` preserves identity | ✅ Read-only | Rust ignores user-submitted `app_name`/`app_description`, keeps env values |

## Current State — What's Still Hardcoded

| Surface | File | Line(s) | Current Hardcoded Value |
|---|---|---|---|
| Main window `<title>` | `index.html` | 7 | `"Liminal Screen - Options"` |
| Main window `<h1>` | `index.html` | 14 | `"Liminal Screen"` |
| Main window `.subtitle` | `index.html` | 15 | `"System Tray Screensaver"` |
| Main window About text | `index.html` | 130 | `"Liminal Screen runs in your system tray..."` |
| `src/main.ts` options.effect() | `src/main.ts` | 301-312 | Never reads `opts.appName`/`opts.appDescription` |
| `src/main.ts` console logs | `src/main.ts` | 65, 69 | `"Liminal Screen - Initializing..."` / `"...Ready"` |
| System tray tooltip | `src-tauri/src/lib.rs` | 198-224 | **Not set at all** — OS falls back to `productName` |
| `tauri.conf.json` productName | `src-tauri/tauri.conf.json` | 3 | `"Liminal Screen"` |
| `tauri.conf.json` identifier | `src-tauri/tauri.conf.json` | 5 | `"org.metazoa.tomaszatoo.liminal-screen"` |
| `tauri.conf.json` window title | `src-tauri/tauri.conf.json` | 17 | `"Liminal Screen"` |
| `tauri.conf.json` shortDescription | `src-tauri/tauri.conf.json` | 48 | `"System tray screensaver application"` |
| `tauri.conf.json` longDescription | `src-tauri/tauri.conf.json` | 49 | Hardcoded text |
| `Cargo.toml` crate name | `src-tauri/Cargo.toml` | 2 | `"liminal-screen"` |
| `package.json` name | `package.json` | 2 | `"liminal-screen"` |
| `power_monitor.rs` --who flag | `src-tauri/src/power_monitor.rs` | 455, 635, 663 | `"--who=liminal-screen"` |

---

## Plan — Grouped by Effort and Scope

### Tier 1: Runtime Dynamic (Simple JS Changes)

These changes make the main window HTML reactive to `VITE_APP_NAME` and `VITE_APP_DESCRIPTION` via the `options` signal, matching the pattern already used in the remote options page.

**1.1 — Update `index.html` with placeholder IDs**

Add `id` attributes to the elements that need dynamic branding:

```html
<!-- BEFORE -->
<h1>Liminal Screen</h1>
<p class="subtitle">System Tray Screensaver</p>

<!-- AFTER -->
<h1 id="app-name">Liminal Screen</h1>
<p class="subtitle" id="app-description">System Tray Screensaver</p>
```

Also update the About section paragraph to have an `id`:

```html
<p id="about-text">Liminal Screen runs in your system tray...</p>
```

Keep the hardcoded text as fallback — if `init()` fails or options are null, the user still sees something.

**1.2 — Add `setIdentity()` to `src/main.ts`**

Mirror the pattern from `remote-options/main.ts`:

```ts
function setIdentity(opts: AppOptions): void {
  const nameEl = document.getElementById("app-name");
  const descEl = document.getElementById("app-description");
  const aboutEl = document.getElementById("about-text");
  const titleEl = document.querySelector("title");

  if (nameEl)   nameEl.textContent = opts.appName;
  if (descEl)   descEl.textContent = opts.appDescription;
  if (titleEl)  titleEl.textContent = `${opts.appName} - Options`;
  if (aboutEl)  aboutEl.textContent =
    `${opts.appName} runs in your system tray and activates after a period of inactivity. ` +
    `${opts.appDescription}`;
}
```

**1.3 — Call `setIdentity()` inside the existing `options.effect()`**

```ts
options.effect((opts) => {
  if (!opts) return;
  // existing field updates...
  if (startsInInput) startsInInput.value = String(opts.startsIn);
  // ...

  // NEW: update app identity
  setIdentity(opts);
});
```

**1.4 — Update console logs to use the dynamic name**

```ts
// Before
console.log("Liminal Screen - Initializing...");
console.log("Liminal Screen - Ready", options.get());

// After — only after options are loaded
console.log(`${options.get()?.appName ?? "Liminal Screen"} - Initializing...`);
// Or simpler: just remove the hardcode, use opts.appName in the init() success log
```

Since console logs fire before and after `init()`, the simplest approach: keep "Liminal Screen" as fallback in the init log, then log the real name after options load:

```ts
async function init(): Promise<void> {
  if (initialized) return;
  initialized = true;
  console.log("Initializing...");
  try {
    options.set(await invoke<AppOptions>("get_options"));
    // ... register SW, setup listeners ...
    const name = options.get()?.appName ?? "Liminal Screen";
    console.log(`${name} - Ready`, options.get());
  } catch (error) {
    console.error("Failed to initialize:", error);
  }
}
```

### Tier 2: System Tray Tooltip (Simple Rust Change)

**2.1 — Add `.tooltip()` to `TrayIconBuilder` in `lib.rs`**

Currently the tray icon has no tooltip. On macOS, the OS falls back to displaying the `productName` from `tauri.conf.json` (hardcoded "Liminal Screen"). We should set it explicitly from the app name:

```rust
let app_name = std::env::var("VITE_APP_NAME")
    .unwrap_or_else(|_| "Liminal Screen".to_string());

let tray = app.tray_by_id("main-tray").unwrap();
tray.set_tooltip(Some(&app_name))?;
```

Or set it during `TrayIconBuilder` construction:

```rust
let tray = TrayIconBuilder::new()
    .id("main-tray")
    .icon(app.default_window_icon().unwrap().clone())
    .tooltip(&app_name)  // NEW
    .menu(&menu)
    .on_menu_event(move |app, event| { ... })
    .build(app)?;
```

This ensures the tray tooltip matches `VITE_APP_NAME` at runtime.

### Tier 3: Build-Time Templates (Moderate — Script-Based)

These values are baked into the binary at build time and **cannot** be changed at runtime. Currently, fork developers must manually edit `tauri.conf.json` (documented in README.md). We can automate this with a build script.

**3.1 — Create `scripts/set-identity.ts` (or `.sh`)**

A pre-build script that reads `.env` and patches `tauri.conf.json`:

```ts
#!/usr/bin/env bun
// scripts/set-identity.ts
// Reads .env and updates tauri.conf.json with VITE_APP_NAME and VITE_APP_DESCRIPTION

import { readFileSync, writeFileSync } from "fs";

// Parse .env
const envContent = readFileSync(".env", "utf-8");
const env: Record<string, string> = {};
for (const line of envContent.split("\n")) {
  const match = line.match(/^(\w+)=(.*)$/);
  if (match) env[match[1]] = match[2].replace(/^["']|["']$/g, "");
}

const name = env.VITE_APP_NAME || "Liminal Screen";
const desc = env.VITE_APP_DESCRIPTION || "";

// Update tauri.conf.json
const configPath = "src-tauri/tauri.conf.json";
const config = JSON.parse(readFileSync(configPath, "utf-8"));
config.productName = name;
config.app.windows[0].title = name;
config.bundle.shortDescription = desc || config.bundle.shortDescription;
config.bundle.longDescription = desc || config.bundle.longDescription;
writeFileSync(configPath, JSON.stringify(config, null, 2) + "\n");

console.log(`Updated tauri.conf.json: productName="${name}"`);
```

**3.2 — Add the script to the dev/build workflow in `package.json`**

```json
{
  "scripts": {
    "prebuild": "bun run scripts/set-identity.ts",
    "predev": "bun run scripts/set-identity.ts"
  }
}
```

npm/bun lifecycle hooks run `prebuild` automatically before `build`, and `predev` before `dev`. This ensures `tauri.conf.json` always reflects `.env` values before the Rust compilation.

**Important constraint:** The `identifier` field (`org.metazoa.tomaszatoo.liminal-screen`) MUST be changed manually by fork developers. Changing the identifier programmatically is dangerous — it affects macOS keychain, preferences, and could conflict with other installed apps. The README already documents this. The script should NOT touch the identifier.

**3.3 — Also update `power_monitor.rs` --who flag**

The `--who=liminal-screen` argument sent to `systemd-inhibit` on Linux should use the app name:

```rust
// Before
.arg("--who=liminal-screen")

// After — use app_name from env or state
.arg(format!("--who={}", app_name))
```

This requires passing the app name to wherever `systemd-inhibit` is called, or reading the env var directly (same pattern used for window title). Since this only affects Linux builds, it's a lower priority.

### Tier 4: Installer / OS Metadata (Future Consideration)

These are OS-specific surfaces that show the app name outside the running app:

| Surface | Platform | Source |
|---|---|---|
| macOS `.app` bundle name in Finder | macOS | `productName` in `tauri.conf.json` (Tier 3) |
| macOS About dialog (cmd+shift+?) | macOS | Would need native About dialog via Tauri |
| Windows taskbar label | Windows | `productName` (Tier 3) |
| Linux `.desktop` file Name= | Linux | Generated by Tauri bundler from `productName` |
| macOS Dock label | macOS | `productName` (Tier 3) |
| Process name in Activity Monitor / Task Manager | All | `Cargo.toml` `name` field |

Tier 3's build script handles `productName`, which covers the dock, taskbar, installer, and `.desktop` file. The process name (Cargo.toml) would require a more invasive build script and is not recommended — `liminal-screen` as a binary name is fine since users don't see it directly.

---

## Summary — Implementation Order

| Step | Tier | Effort | Impact |
|---|---|---|---|
| 1. Add `id` attrs to `index.html` branding elements | 1 | Trivial | Main window shows fork name |
| 2. Add `setIdentity()` to `src/main.ts` + wire into `options.effect()` | 1 | Easy | Main window fully branded |
| 3. Update console logs | 1 | Trivial | Consistency |
| 4. Add `.tooltip()` to system tray | 2 | Easy | Hovering tray shows fork name |
| 5. Create `scripts/set-identity.ts` | 3 | Moderate | tauri.conf.json always in sync with .env |
| 6. Wire `predev` / `prebuild` hooks | 3 | Easy | Automatic sync |
| 7. Dynamic `--who=` in power_monitor | 3 | Easy | Linux systemd-inhibit uses fork name |

Steps 1-4 can be done in one pass. Steps 5-7 are a second pass.

---

## What We're NOT Changing

| Item | Why |
|---|---|
| `tauri.conf.json` `identifier` | Too dangerous to automate — fork devs MUST change this manually |
| `Cargo.toml` `name` | Binary name doesn't affect user-facing branding; not worth automating |
| `package.json` `name` | npm package name, not user-visible |
| Source file comment headers (`// Liminal Screen ...`) | Developer-facing, not user-facing; low priority |
| `src/styles.css` comment | Same |

---

## Testing Checklist

After implementation, test with a custom `.env`:

```bash
VITE_APP_NAME="Acme Screensaver"
VITE_APP_DESCRIPTION="The finest corporate screensaver"
```

- [ ] Main window `<title>` shows "Acme Screensaver - Options"
- [ ] Main window `<h1>` shows "Acme Screensaver"
- [ ] Main window `.subtitle` shows "The finest corporate screensaver"
- [ ] Main window About section references "Acme Screensaver"
- [ ] System tray tooltip on hover shows "Acme Screensaver"
- [ ] Options window title shows "Acme Screensaver Options"
- [ ] Remote options page shows "Acme Screensaver — Options"
- [ ] After `bun run build`, macOS .app bundle name is "Acme Screensaver"
- [ ] After `bun run dev`, main window title is "Acme Screensaver"
- [ ] `factory_reset_options` + reload still shows env-based name (not stale persisted name)

---

## Relation to Existing Architecture

This plan follows the existing design principle from `src/app/types.ts`:

> "Read-only fields (saverUrl, optionsUrl, appName, appDescription) come from the fork's `.env` and cannot be changed by the user."

The Rust backend already enforces this — `set_options` preserves identity fields from env. This plan extends the same principle to the frontend UI surfaces that were missed in the initial implementation.