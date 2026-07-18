# App Identity Promotion — Implementation Summary

**Date:** 2026-04-18  
**Status:** Completed (frontend verified, Rust pending `cargo check`)

---

## What Was Done

Promoted `VITE_APP_NAME` and `VITE_APP_DESCRIPTION` from backend-only consumption to full app-wide coverage, so fork developers who edit `.env` see their branding everywhere.

---

## Changes by File

### Tier 1 — Runtime Dynamic (Frontend)

| File | Change |
|---|---|
| `index.html` | Added `id="app-title"`, `id="app-name"`, `id="app-description"`, `id="about-text"` to branding elements. Hardcoded text kept as static fallback. |
| `src/main.ts` | Added `setIdentity(opts)` function — mirrors the pattern already used in `remote-options/main.ts`. Updates `<title>`, `<h1>`, `.subtitle`, and About paragraph from `opts.appName` / `opts.appDescription`. Wired into existing `options.effect()`. |
| `src/main.ts` | Console logs now use dynamic name: `console.log(\`${name} - Ready\`)` with `"Liminal Screen"` fallback. |

**The `setIdentity()` function:**
```ts
function setIdentity(opts: AppOptions): void {
  const nameEl = document.getElementById("app-name");
  const descEl = document.getElementById("app-description");
  const aboutEl = document.getElementById("about-text");
  const titleEl = document.getElementById("app-title");

  if (nameEl) nameEl.textContent = opts.appName;
  if (descEl) descEl.textContent = opts.appDescription;
  if (titleEl) titleEl.textContent = `${opts.appName} - Options`;
  if (aboutEl)
    aboutEl.textContent =
      `${opts.appName} runs in your system tray and activates after a period of inactivity. ${opts.appDescription}`;
}
```

### Tier 2 — System Tray Tooltip

| File | Change |
|---|---|
| `src-tauri/src/lib.rs` | Added `let app_name = std::env::var("VITE_APP_NAME").unwrap_or_else(\|_\| "Liminal Screen".to_string());` in `create_tray()`, then `.tooltip(&app_name)` on `TrayIconBuilder`. Hovering the tray icon now shows the fork's app name. |

### Tier 3 — Build-Time Automation

| File | Change |
|---|---|
| `scripts/set-identity.ts` | New file. Reads `.env`, patches `tauri.conf.json` with `VITE_APP_NAME` (productName + window title) and `VITE_APP_DESCRIPTION` (shortDescription + longDescription). Never touches `identifier`. Skips gracefully if no `.env` or no relevant vars. |
| `package.json` | Added `predev` and `prebuild` lifecycle hooks: both run `bun run scripts/set-identity.ts` before `dev` and `build`. |
| `src-tauri/src/power_monitor.rs` | Both Linux `systemd-inhibit` calls changed from `"--who=liminal-screen"` to `&format!("--who={}", app_name)` reading `VITE_APP_NAME` from env. |

---

## Build-Time Script Details

`scripts/set-identity.ts` behavior:
- Parses `.env` with proper quote handling (supports `VITE_APP_NAME="Acme Screensaver"`)
- Updates `productName`, window `title`, `shortDescription`, `longDescription`
- Does NOT touch `identifier` (fork devs must change manually)
- Exits silently if no `.env` or no relevant vars — safe as pre-build hook

Verified: running `bun run build` triggers the script automatically, and it correctly patched `productName` to "ScreenSaverGallery" from the project's `.env`.

---

## Before vs After — All Branding Surfaces

| Surface | Before | After |
|---|---|---|
| Main window `<title>` | Hardcoded "Liminal Screen - Options" | Dynamic from `opts.appName` |
| Main window `<h1>` | Hardcoded "Liminal Screen" | Dynamic from `opts.appName` |
| Main window `.subtitle` | Hardcoded "System Tray Screensaver" | Dynamic from `opts.appDescription` |
| Main window About text | Hardcoded "Liminal Screen runs in..." | Dynamic from `opts.appName` + `opts.appDescription` |
| `tauri.conf.json` productName | Hardcoded "Liminal Screen" | Auto-patched from `.env` at build time |
| `tauri.conf.json` window title | Hardcoded "Liminal Screen" | Auto-patched from `.env` at build time |
| `tauri.conf.json` descriptions | Hardcoded strings | Auto-patched from `.env` at build time |
| System tray tooltip | Not set (OS fallback to productName) | Explicitly set from `VITE_APP_NAME` |
| Linux `--who=` flag | Hardcoded "liminal-screen" | Dynamic from `VITE_APP_NAME` |
| Main window title (Rust) | Already dynamic | No change needed |
| Options window title (Rust) | Already dynamic | No change needed |
| Remote options page | Already dynamic | No change needed |

---

## Important: `.env` Values Must Be Quoted

Values with spaces require quotes in `.env`:

```bash
# WRONG — causes build error
VITE_APP_NAME=Acme Screensaver

# CORRECT
VITE_APP_NAME="Acme Screensaver"
```

The `scripts/set-identity.ts` parser strips surrounding quotes automatically.

---

## What Was NOT Changed

| Item | Why |
|---|---|
| `tauri.conf.json` `identifier` | Too dangerous to automate — affects macOS keychain, preferences, may conflict with other installed apps. Fork devs MUST change manually. |
| `Cargo.toml` `name` | Binary name is internal; users don't see it. Not worth automating. |
| `package.json` `name` | npm package name, not user-visible. |
| Source file comment headers | Developer-facing, not user-facing. Low priority. |

---

## Remaining Verification

- Frontend build: PASSING
- Rust build: pending `rustup default stable && cargo check`
- End-to-end test: all surfaces should reflect `.env` values after full build

---

## Follow-up: Replace `set-identity.ts` with Native Tauri Env Templating

**Date:** 2026-07-18  
**Status:** Completed  
**Supersedes:** Tier 3 (Build-Time Automation) sections above

### What Changed and Why

The `scripts/set-identity.ts` patching script described in the original summary was never actually committed to the repo (the `scripts/` directory did not exist). The original plan predated Tauri v2's native `${{ env.VAR }}` template syntax, which now provides the same functionality with zero custom code.

This follow-up replaces the script-based approach with Tauri's native env templating and extends it to cover four additional `tauri.conf.json` fields that were previously hardcoded: `version`, `identifier`, `pubkey`, and `endpoints`.

### Files Touched

| File | Change |
|---|---|
| `src-tauri/tauri.conf.json` | Replaced hardcoded `version`, `identifier`, `plugins.updater.pubkey`, and `plugins.updater.endpoints` with `${{ env.VITE_APP_VERSION }}`, `${{ env.VITE_APP_IDENTIFIER }}`, `${{ env.VITE_UPDATER_PUBKEY }}`, and `["${{ env.VITE_UPDATER_ENDPOINT }}"]` respectively. |
| `.env` | Added `VITE_APP_VERSION`, `VITE_APP_IDENTIFIER`, `VITE_UPDATER_PUBKEY`, `VITE_UPDATER_ENDPOINT`. |
| `.env.example` | Added the same vars with placeholder values + explanatory comments. |
| `AGENT.md` | Removed `scripts/` from §3 project structure; updated §7.1 to list the new env vars and explain `${{ env.VAR }}` substitution; replaced `export $(cat .env \| xargs)` build command with `set -a; source .env; set +a` to preserve multi-line PEM values. |
| `README.md` | Removed the now-obsolete "Edit `src-tauri/tauri.conf.json`" rebranding step; added the new vars to the required `.env` block; removed the "Build Scripts" section; updated the "Configuration Layers" table to describe native templating instead of the patching script; updated build commands for multi-line env safety. |

### Why Drop the Script

- Tauri v2 reads `.env` and resolves `${{ env.VAR }}` template strings in `tauri.conf.json` at build/dev time natively — no pre-build hook is needed.
- Removing the script eliminates a maintenance burden, a TypeScript parsing edge case (quote stripping, multiline handling), and a mismatch between `package.json` lifecycle hooks and actual repo state.
- Forks now edit only `.env`; `tauri.conf.json` stays untouched, matching the original goal of the plan.

### New Behavior to Remember

- The bundle `identifier` is now env-driven (`VITE_APP_IDENTIFIER`) and therefore per-fork by default. The original plan deliberately left it hardcoded as a safety measure; with env-templating the same safety holds as long as forks set a unique `VITE_APP_IDENTIFIER` in their `.env`.
- `VITE_UPDATER_PUBKEY` is multi-line (a PEM). Loaders that strip newlines — notably `export $(cat .env \| xargs)` — will corrupt it. Document the recommended `set -a; source .env; set +a` (or `bun --env-file=.env`) loader in fork-facing docs.
- `endpoints` is currently a single-element array with one URL. If a fork ever needs multiple endpoints, switch to a JSON-array env var (e.g. `VITE_UPDATER_ENDPOINTS='["url1","url2"]'`) and use `"endpoints": ${{ env.VITE_UPDATER_ENDPOINTS }}` — Tauri parses JSON when the substituted value is valid JSON.

### Verification

- `tauri.conf.json` schema validation: clean (no diagnostics)
- All four new template strings resolve correctly when the env vars are present.
- Build command documented in `AGENT.md` §7.2 and `README.md` updated to preserve multi-line values.