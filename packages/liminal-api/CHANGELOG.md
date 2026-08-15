# Changelog

## 0.5.0

### Added

- `isMediaActive()` — reports whether a video player, video call, etc. is
  holding a display-sleep power assertion that's currently suppressing the
  saver, so an options page can tell the user why it hasn't started rather
  than leaving them thinking it's broken. Backed by the `is_media_active`
  command; macOS only (always resolves `false` on Windows/Linux, where this
  detection isn't implemented). Requires an app build with the
  `is_media_active` command (unreleased as of app 0.2.0) — rejects with
  `LiminalAPIError` on older builds.

- `getMediaBlockerName()` — names the process responsible for a suppressed
  saver (e.g. `"LocalSend"`), or `null` if none. `isMediaActive()` alone only
  tells you *that* something is blocking the saver — obvious for genuine media
  playback, but not for an idle background app holding the same kind of
  assertion; this fills in the *what*. Reads the OS's per-process assertion
  list, so it's heavier than `isMediaActive()` — call it only once you already
  know the saver is blocked. macOS only; same app-version requirement and
  error behavior as `isMediaActive()`.

## 0.4.0

### Added

- `closeOptions()` — close the options window from the page itself, for a
  "Close"/"Done" button. Backed by the app's `close_options` command rather than
  the Tauri window API: a remote page would otherwise need
  `core:window:allow-close` granted to its origin, and a denied core command just
  looks like nothing happening. App commands aren't ACL-gated, so this works
  without widening what a remote page may do. Requires app 0.3.0+; a no-op
  outside Tauri, and when the window is already closed.

  Named `closeOptions()`, not `close()`, to keep it distinct from `destroy()`,
  which only detaches event listeners and leaves the window alone.

## 0.3.1

Error-handling fixes for remotely-hosted options pages.

### Fixed

- `ask()`, `showMessage()`, `startAutoSync()` and `onUpdateAvailable()` now
  degrade instead of failing hard when the IPC call is rejected.

  Tauri's ACL scopes every capability to *local* content unless it declares
  remote origins, so on a page served over http(s) these plugin and core commands
  were denied even though the options window's capability lists them. App-defined
  commands (`getOptions`, `setOptions`, `previewScreensaver`, …) aren't ACL-gated,
  which made the failures look arbitrary:

  - `ask()` / `showMessage()` rejected instead of falling back to
    `confirm()` / `alert()` — so a reset confirmation dialog broke the flow
  - `onUpdateAvailable()` produced an unhandled promise rejection
  - `startAutoSync()` rejected, and `createOptionsStore()` swallows that error —
    so live option updates silently stopped working, with no symptom to notice

  Each now logs a warning and falls back, matching what `openUrl()` already did.
  Liminal Screen 0.3.0+ grants the permissions to the options page's origin at
  runtime, so the native paths work there; the fallbacks cover older builds.

### Changed

- Documented the remote-origin ACL requirement — the cause of
  `opener.open_url not allowed on window "options" … URL: local` — in the
  integration guide, with the capability JSON forks need if they maintain their
  own `options.json`.

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
