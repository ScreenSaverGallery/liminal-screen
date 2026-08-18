# Plan: Improve Bundle Metadata for Storefronts and Login Items

**Date:** 2026-08-18  
**Status:** Implemented  

---

## Problem

Production builds produced by the CI/CD pipeline are missing OS-level branding metadata:

1. **macOS — System Settings > Login Items & Extensions**
   - The app appears without the proper app icon.
   - The displayed name is the developer/publisher name instead of the app name.

2. **Linux — App Center / Software Center (`.deb` install)**
   - Only the sanitized package name is shown (e.g. `screen-saver-gallery`).
   - The **Developer / Publisher** field is empty.

These surfaces read from bundle metadata that is either not set or is being pulled from the wrong source.

---

## Root Cause Analysis

| Surface | Source Today | Why It Fails |
|---|---|---|
| Linux `.deb` `Maintainer:` | `Cargo.toml` `[package].authors` (`tomaszatoo`) | The field is a bare username with no email/name. Debian/App Center expects a proper `Name <email>` maintainer string, so it renders nothing useful. Tauri uses `authors` in preference to `bundle.publisher`, so setting `bundle.publisher` alone does not help while `authors` is present. |
| macOS `CFBundleName` | `bundle.productName` (patched from `VITE_APP_NAME`) | The name should already be correct, but Login Items also leans on explicit bundle naming (`bundle.macOS.bundleName`) and a complete, signed `.app` bundle icon. Today neither `bundleName` nor `copyright`/`publisher` are emitted, so macOS falls back to the code-signing identity / author metadata in some security/login contexts. |
| Windows Installer `Manufacturer` | Defaults to second element of `identifier` | Not reported as broken, but the same `bundle.publisher` fix will make Windows installer metadata consistent with Linux. |

---

## Proposed Changes

1. **Remove `[package].authors` from `src-tauri/Cargo.toml`.**
   - This lets Tauri use `bundle.publisher` as the single source of truth for `.deb` Maintainer and Windows Manufacturer.
   - `license` remains, so Cargo metadata is still valid for an application binary.

2. **Add `VITE_APP_PUBLISHER` and `VITE_APP_COPYRIGHT` to `.env.example`.**
   - `VITE_APP_PUBLISHER` should be in `"Name or Org <email@example.com>"` format (Debian/App Center best practice).
   - `VITE_APP_COPYRIGHT` is optional but recommended.

3. **Extend `scripts/build-tauri-config.ts` to emit:**
   - `bundle.publisher` from `VITE_APP_PUBLISHER`
   - `bundle.copyright` from `VITE_APP_COPYRIGHT`
   - `bundle.macOS.bundleName` from `VITE_APP_NAME` (preserves existing `dmg` / `minimumSystemVersion` defaults via JSON Merge Patch aware merging)

4. **Add structural defaults to `src-tauri/tauri.conf.json`:**
   - `bundle.macOS.minimumSystemVersion`: `"10.13"`
   - `bundle.linux.deb.section`: `"utils"`
   - `bundle.linux.deb.priority`: `"optional"`
   - No placeholders for `publisher` / `copyright` — those are omitted from the patch when the env var is unset, so they never leak a placeholder string into released metadata.

5. **Fix the updater `endpoints` placeholder in `src-tauri/tauri.conf.json`.**
   - Change from `["SET_VITE_UPDATER_ENDPOINT_IN_.env"]` to `["https://example.invalid/"]`.
   - The updater plugin deserializes `endpoints` as `Vec<Url>` at runtime; a non-URL placeholder crashes the app when the merge-patch isn't applied (e.g. a dev build with no `VITE_UPDATER_ENDPOINT`).

6. **Update `README.md` App Identity section** to document the new env vars.

7. **Update `TODO.md`** with a checked/unchecked item tracking this work.

---

## Implementation Phases

1. Update `.env.example` with `VITE_APP_PUBLISHER` and `VITE_APP_COPYRIGHT`.
2. Remove `authors` from `src-tauri/Cargo.toml`.
3. Extend `scripts/build-tauri-config.ts` to patch publisher / copyright / macOS bundleName.
4. Add `bundle.macOS` and `bundle.linux.deb` defaults to `src-tauri/tauri.conf.json`.
5. Update `README.md` documentation.
6. Update `TODO.md`.
7. Fix the updater `endpoints` placeholder in `src-tauri/tauri.conf.json`.
8. Run `cargo check` / `bun run build` / `bun run scripts/build-tauri-config.ts` to validate the merge-patch output.

---

## Files Touched

| File | Action |
|---|---|
| `.env.example` | Add `VITE_APP_PUBLISHER` and `VITE_APP_COPYRIGHT` |
| `src-tauri/Cargo.toml` | Remove `authors = ["tomaszatoo"]` |
| `src-tauri/tauri.conf.json` | Add `bundle.macOS` / `bundle.linux.deb` structural defaults; fix updater `endpoints` placeholder |
| `scripts/build-tauri-config.ts` | Patch `bundle.publisher`, `bundle.copyright`, `bundle.macOS.bundleName` |
| `README.md` | Document new env vars |
| `TODO.md` | Add tracking item |

---

## Verification

- `bun run scripts/build-tauri-config.ts` with a populated `.env` produces a merge-patch containing the new keys.
- `cargo check` passes after `Cargo.toml` change.
- Inspecting a built `.deb` control file shows a proper `Maintainer:` line.
- Inspecting a built macOS `.app` `Info.plist` shows `CFBundleName` equal to `VITE_APP_NAME`.

---

## Open Questions

- **macOS icon in Login Items:** The icon ultimately depends on the `APP_ICON` CI secret / local `app-icon.png` being a valid 1024×1024+ PNG and the app being code-signed. This plan makes the metadata pipeline correct; the actual Login Items icon rendering should be re-verified after the next signed release build.
- Should `VITE_APP_PUBLISHER` be required by the release workflow? For now it remains optional to avoid breaking forks that have not set it, but the README strongly recommends it.
