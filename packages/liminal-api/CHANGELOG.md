# Changelog

## 0.3.0

Lets a remote options page open external links and screensaver previews.

### Added

- `openUrl(url, openWith?)` — open an external URL in the user's default browser
  or application via the Tauri `opener` plugin, instead of navigating the options
  window away from your page. Falls back to
  `window.open(url, '_blank', 'noopener')` outside Tauri or if the plugin call
  fails. **Requires the `opener:default` permission on the options window**,
  which Liminal Screen ships from 0.3.0 — on older app builds the fallback runs
  and the webview ignores it, so the link appears dead (a warning is logged).

### Changed

- `previewScreensaver()` now resolves the saver URL itself (`saverUrlDebug` when
  `debug` is on, else `saverUrl`) and calls the backend's
  `create_preview_window` command directly, instead of invoking
  `preview_screensaver` and relying on the main window's event relay — the old
  path didn't work reliably from a remote options page. It throws
  `LiminalAPIError('No saver URL configured for preview')` when the resolved URL
  is empty. Same signature; needs app 0.2.0+.
- Documented both methods in the README, API spec, integration guide and
  security model, and added an app-compatibility table to the README.

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
