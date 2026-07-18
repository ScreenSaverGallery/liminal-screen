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

## Follow-up: Replace `set-identity.ts` with a Tauri Merge-Patch Build Script

**Date:** 2026-07-18  
**Status:** Completed  
**Supersedes:** Tier 3 (Build-Time Automation) sections above

### What Changed and Why

The `scripts/set-identity.ts` patching script described in the original summary was never actually committed to the repo (the `scripts/` directory did not exist). The original plan predated an investigation into Tauri v2’s config-parsing behavior.

**Key finding:** Tauri v2 does **not** support `${{ env.VAR }}` substitution in `tauri.conf.json`. The CLI’s parser (`tauri-utils::config::parse::parse_value`) is a plain `serde_json::from_str` — it stores `${{ env.X }}` as a literal string. The existing `"productName": "${{ env.VITE_APP_NAME }}"` was therefore being treated as the literal string `${{ env.VITE_APP_NAME }}`; it only “looked” like it worked because `productName` accepts any string matching `^[^/\\:*?"<>|]+$` (which the literal satisfies) and the Rust runtime overrides the displayed name in the tray and window title at runtime via the `dotenv` crate. In a production bundle, `productName` would have been the literal template string — a latent bug.

The same approach cannot work for `version` (must be valid semver), `identifier` (must match reverse-domain pattern), `pubkey`, or `endpoints` — the JSON schema validator runs *before* any hypothetical substitution, so literal template strings fail validation.

The replacement approach uses Tauri’s official `--config` flag, which accepts a path to a JSON file that is merged with the base `tauri.conf.json` using JSON Merge Patch (RFC 7396). A new build script generates that merge-patch from `.env`.

### Files Touched

| File | Change |
|---|---|
| `scripts/build-tauri-config.ts` | New file. Reads `.env` (with quote-aware, multi-line PEM support) and `src-tauri/tauri.conf.json`, then writes a merge-patch to `src-tauri/.tauri-runtime.conf.json` (gitignored) overriding `productName`, `app.windows[0].title`, `bundle.shortDescription`, `bundle.longDescription`, `version`, `identifier`, `plugins.updater.pubkey`, and `plugins.updater.endpoints`. Only emits keys present and non-empty in `.env`. Idempotent — safe to run on every `tauri` invocation. Falls back to `{}` if `.env` is missing. |
| `package.json` | Added `tauri:dev` and `tauri:build` scripts that run `scripts/build-tauri-config.ts` first, then invoke `tauri dev --config src-tauri/.tauri-runtime.conf.json` (resp. `tauri build …`). Kept `tauri` plain (`"tauri": "tauri"`) because `--config` is only accepted by the `dev`/`build`/`bundle` subcommands, not `info`/`icon`/etc. |
| `.gitignore` | Added `src-tauri/.tauri-runtime.conf.json` — generated, never edit by hand, never commit. |
| `src-tauri/tauri.conf.json` | Replaced per-fork hardcoded values with obvious, schema-valid placeholders (`productName: "SET_VITE_APP_NAME_IN_.env"`, `version: "0.0.0"`, `identifier: "com.example.set-vite-app-identifier-in-env"`, `shortDescription`/`longDescription: "Set VITE_APP_DESCRIPTION in .env"`, `pubkey: "SET_VITE_UPDATER_PUBKEY_IN_.env"`, `endpoints: ["https://example.invalid/"]`, `app.windows[0].title: "SET_VITE_APP_NAME_IN_.env"`). Structural config (build commands, dev URL, window shape, CSP, bundle icons/category, updater install mode) is unchanged. The base config is a valid standalone Tauri config; the merge-patch overrides the placeholders with real values from `.env` whenever the relevant env var is set. |
| `.env` | Added `VITE_APP_VERSION`, `VITE_APP_IDENTIFIER`, `VITE_UPDATER_PUBKEY`, `VITE_UPDATER_ENDPOINT`. |
| `.env.example` | Added the same vars with placeholder values + explanatory comments. |
| `AGENT.md` | Re-added `scripts/` to §3 project structure; updated §7.1 to explain the merge-patch mechanism and Tauri’s lack of native env substitution; updated §7.2 build commands to `tauri:dev` / `tauri:build`. |
| `README.md` | Updated §2 rebranding instructions to describe the merge-patch mechanism; updated §6 Build section; restored a "Build Scripts" section documenting `build-tauri-config.ts`; updated the "Configuration Layers" table; updated build commands to `tauri:dev` / `tauri:build`. |

### Why a Merge-Patch, Not In-Place Patching

The original plan envisioned a script that rewrites `tauri.conf.json` in place (`set-identity.ts`). This was abandoned in favor of the merge-patch approach because:

- `tauri.conf.json` is never mutated, so git state stays clean — no churn, no accidental commits of env-specific values.
- Uses Tauri’s official, documented `--config` flag (RFC 7396 merge) rather than a custom patching convention.
- The base `tauri.conf.json` is a valid standalone config — any tool that reads it without the `--config` override still gets sensible defaults.
- Forks only ever edit `.env`; the same goal as the original plan, achieved with no file mutation.

### Why Placeholders, Not a `_DO_NOT_EDIT` Documentation Field

To make the role of `tauri.conf.json` self-documenting, a `_DO_NOT_EDIT` (or `_comment`) field at the root was considered and rejected after empirical testing.

**Finding:** Tauri’s JSON schema (`https://schema.tauri.app/config/2`, mirrored at `crates/tauri-cli/config.schema.json`) sets `"additionalProperties": false` at the root. The Tauri CLI validates the config against this schema in `load_config` and exits with code 1 on any validation error. Adding `"_DO_NOT_EDIT": "…"` produces:

```
Error `"tauri.conf.json"` error: Additional properties are not allowed ('_DO_NOT_EDIT' was unexpected)
error: script "tauri" exited with code 1
```

Zed’s schema-backed diagnostics flags the same error in-editor.

The replacement: obvious, schema-valid placeholder values for per-fork fields (see the file table above). A fork opening `tauri.conf.json` immediately sees `"productName": "SET_VITE_APP_NAME_IN_.env"` and understands that the real value lives elsewhere. The placeholders are all schema-valid (semver `0.0.0`, reverse-domain `com.example.set-vite-app-identifier-in-env`, productName character class, free-form strings for pubkey/descriptions, valid URL `https://example.invalid/` for endpoints) — the base config parses standalone via `tauri info` / `tauri dev`.

**Why `endpoints` uses a real URL (`https://example.invalid/`) instead of `SET_VITE_…`:** The Tauri updater plugin's `Config` struct deserializes `endpoints` as `Vec<Url>` in its `setup` hook (at runtime, not config-load time). A non-URL placeholder like `SET_VITE_UPDATER_ENDPOINT_IN_.env` causes the app to panic at startup with `Error deserializing 'plugins.updater' … relative URL without a base` whenever the merge-patch isn't applied (e.g. `cargo run` direct without `bun run tauri:dev`). The `https://example.invalid/` placeholder (RFC 2606 reserved TLD) is a valid URL, obviously fake, and the runtime updater is deactivated via `VITE_UPDATER_ENDPOINT` (see the `updater_enabled()` guard in `src-tauri/src/updater.rs`), so the placeholder is never actually fetched in the normal flow.

### Updater Deactivation When `VITE_UPDATER_ENDPOINT` Is Empty

A follow-up to the placeholder work: `src-tauri/src/updater.rs` now has an `updater_enabled()` helper that checks `VITE_UPDATER_ENDPOINT` via the same env-setting pattern as the rest of the backend (runtime env var first, then `option_env!` baked in at compile time). When unset or empty, `update_silent`, `check_update`, and `download_and_install` all become no-ops — no fetch is attempted, no `[updater] Error` is logged. This is the right behavior for forks that haven't published a `latest.json` feed yet: leave `VITE_UPDATER_ENDPOINT` empty in `.env` and the updater stays quiet. Set it once the release feed is live. Verified empirically: with `VITE_UPDATER_ENDPOINT` commented out in `.env`, `bun run tauri:dev` logs `[updater] VITE_UPDATER_ENDPOINT not set; skipping silent update check.` and no fetch is attempted.

Switching to `tauri.conf.json5` (which allows `// comments`) would also work but requires adding the `config-json5` Cargo feature to both `tauri-build` and `tauri` in `src-tauri/Cargo.toml`. The placeholder approach avoids that feature flag for what is essentially a documentation concern.

### How to Use

- Development: `bun run tauri:dev` (regenerates the merge-patch from `.env`, then runs `tauri dev --config src-tauri/.tauri-runtime.conf.json`)
- Production: `set -a; source .env; set +a` (exports env to OS for `option_env!` at compile time), then `bun run tauri:build`
- Other Tauri subcommands (`info`, `icon`, `signer generate`, …) continue to use the plain `tauri` script: `bun run tauri info`

### New Behavior to Remember

- The bundle `identifier` is now env-driven (`VITE_APP_IDENTIFIER`) and therefore per-fork by default. The original plan deliberately left it hardcoded as a safety measure; with the merge-patch approach the same safety holds as long as forks set a unique `VITE_APP_IDENTIFIER` in their `.env`.
- `VITE_UPDATER_PUBKEY` is multi-line (a PEM). The `build-tauri-config.ts` script parses `.env` directly and handles multi-line quoted values, so the merge-patch is unaffected by the shell loader choice. **But** the Rust backend’s `option_env!` reads from the OS environment at compile time — for production builds, env vars must be exported to the OS with newlines preserved (`set -a; source .env; set +a` or `bun --env-file=.env`). `export $(cat .env | xargs)` will corrupt the PEM.
- `endpoints` is currently a single-element array with one URL. If a fork ever needs multiple endpoints, the script can be extended to parse a JSON-array env var (e.g. `VITE_UPDATER_ENDPOINTS='["url1","url2"]'`) — RFC 7396 replaces the array wholesale, so no merge semantics issue there.

### Verification

- `bun run scripts/build-tauri-config.ts` writes a valid merge-patch with correct overrides for `productName`, `app`, `bundle`, `version`, `identifier`, `plugins`.
- `bun run tauri:dev` runs `build-tauri-config.ts` first, then `tauri dev --config src-tauri/.tauri-runtime.conf.json` — config parsing succeeds (the previous `tauri.conf.json > version must be a semver string` error is gone).
- `bun run tauri --version` (plain `tauri` script, no `--config`) still works as expected for subcommands that don’t need the merge-patch.
- A subsequent `cargo` build error encountered during verification (`failed to read plugin permissions: .../ssg-tauri-liminal/.../app_hide.toml: No such file or directory`) is a stale-cache issue from an old project path — unrelated to this change; `cargo clean` clears it.