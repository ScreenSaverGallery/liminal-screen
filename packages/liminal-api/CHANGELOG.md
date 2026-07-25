# Changelog

## 0.2.0 — first public release

Version aligned with the Liminal Screen app release it targets.

### Added

- `checkForUpdates()`, `installUpdate()`, `onUpdateAvailable()` and the `UpdateInfo` type
- `getVersion()`, which prefers the injected `navigator.liminalScreen.version` snapshot over IPC
- OS screensaver conflict handling: `getOsScreensaverStatus()`, `disableOsScreensaver()`,
  `restoreOsScreensaver()`, `getSavedOsScreensaverIdle()` and the `OsScreensaverStatus` type
- `AppOptions.instanceId`, `autostart`, `notificationsEnabled`, `notificationUrl` and
  `notificationCheckIntervalSecs`, mirroring the Rust `AppOptions` struct
- Optional `notificationsEnabled` / `autostart` in `SetOptionsPayload` — omit to keep current values
- `src/global.ts` CDN entry point that attaches the public surface to `globalThis.LiminalAPI`,
  so the documented `<script>` usage works (the previous IIFE bundle exposed no global)

### Changed

- Dialog and version helpers no longer touch a bare `window`, so the package can be
  imported in Node/SSR without throwing
- `docs/SECURITY.md` rewritten: it described a shared-secret auth layer
  (`configureSecurity()`, `generateAuthToken()`) that does not exist
- Package metadata: repository, homepage, license file, `sideEffects: false`,
  `unpkg`/`jsdelivr` entries, and `docs/` + `src/` shipped in the tarball

### Removed

- `src/security.ts` — an empty placeholder for the removed auth layer
