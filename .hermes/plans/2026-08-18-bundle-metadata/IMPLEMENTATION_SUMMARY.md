# Implementation Summary: Bundle Metadata Improvements

**Status:** Implemented  
**Completed:** 2026-08-18

---

## What Changed

| Goal | How It Was Addressed |
|---|---|
| Linux App Center shows no developer | Removed `authors` from `src-tauri/Cargo.toml` so Tauri uses `bundle.publisher` as the `.deb` `Maintainer:` and Windows `Manufacturer`. Added `VITE_APP_PUBLISHER` env var to `.env.example` and wired it through `scripts/build-tauri-config.ts`. |
| macOS Login Items name is the developer | `scripts/build-tauri-config.ts` now explicitly emits `bundle.macOS.bundleName` from `VITE_APP_NAME`, which maps to `CFBundleName` in the generated `Info.plist`. |
| macOS Login Items icon missing | Documented that the icon depends on a valid `app-icon.png` / `APP_ICON` CI secret and proper code signing. Structural metadata is now in place; the actual rendered icon should be re-verified on the next signed release build. |
| General bundle metadata gaps | Added optional `VITE_APP_COPYRIGHT`, Debian `section`/`priority` defaults, and macOS `minimumSystemVersion` default. |
| Updater placeholder crash | Changed base `tauri.conf.json` updater `endpoints` placeholder to `https://example.invalid/` so the app does not panic when `VITE_UPDATER_ENDPOINT` is unset and the merge-patch isn't applied. |

---

## Files Changed

- `.env.example`
  - Added `VITE_APP_PUBLISHER` (recommended format: `"Name <email>"`)
  - Added optional `VITE_APP_COPYRIGHT`
- `src-tauri/Cargo.toml`
  - Removed `authors = ["tomaszatoo"]`
  - Added `license = "Apache-2.0"`
- `src-tauri/tauri.conf.json`
  - Added `bundle.macOS.minimumSystemVersion`: `"10.13"`
  - Added `bundle.linux.deb.section`: `"utils"`
  - Added `bundle.linux.deb.priority`: `"optional"`
  - Changed updater `endpoints` placeholder to `"https://example.invalid/"`
- `scripts/build-tauri-config.ts`
  - Emits `bundle.publisher` from `VITE_APP_PUBLISHER`
  - Emits `bundle.copyright` from `VITE_APP_COPYRIGHT`
  - Emits `bundle.macOS.bundleName` from `VITE_APP_NAME`, preserving existing macOS config
- `README.md`
  - Documented `VITE_APP_PUBLISHER` and `VITE_APP_COPYRIGHT`
  - Added notes on Linux `.deb` Maintainer and macOS Login Items icon
- `TODO.md`
  - Added completed item referencing this plan

---

## Validation

- `bun run scripts/build-tauri-config.ts` — produced a valid merge-patch with `bundle.macOS.bundleName` and the base `minimumSystemVersion` preserved.
- `cargo check` in `src-tauri` — passed.
- `bun run test` — 29 tests passed.

---

## Follow-up for the Next Release

1. Add `VITE_APP_PUBLISHER` (and optionally `VITE_APP_COPYRIGHT`) to the `RELEASE_ENV` repository secret.
2. Verify the built `.deb` control file contains a proper `Maintainer:` line.
3. Verify the built macOS `.app` `Info.plist` contains the expected `CFBundleName` and that the icon renders in System Settings > Login Items after code signing.
