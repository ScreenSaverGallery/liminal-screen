# Autoupdate — Implementation Summary

**Implemented:** 2026-07-08

Implemented as planned, with these deviations:

- `check_for_updates` returns `Option<UpdateInfo>` directly instead of `bool`,
  so `liminalAPI.checkForUpdates()` needs no `_lastUpdateInfo` event capture —
  the command result IS the info. Events are still emitted for listeners.
- `update-download-progress` events are throttled to ~every 512 KiB to avoid
  flooding the IPC bus.
- `update_silent` now reuses `download_and_install` (single code path, so the
  startup auto-update also emits progress/installed events).
- Added `liminalAPI.installUpdate()` (not in the plan, needed for a remote
  options page to actually trigger the install).
- Options window UI additionally shows a "Checking…" state via an
  `updateChecking` signal.

Files touched: `src-tauri/src/updater.rs`, `src-tauri/src/lib.rs` (commands +
tray item "Check for Updates"), `index.html` (#update-section),
`src/main.ts`, `packages/liminal-api/src/types.ts`,
`packages/liminal-api/src/index.ts`.

Not verified end-to-end against a real release: `tauri.conf.json` still has the
placeholder `pubkey` ("CONTENT FROM PUBLICKEY.PEM") and the endpoint URL points
at `releases/download/latest.json` (should normally be
`releases/latest/download/latest.json`). Until those are fixed for the fork,
every check fails gracefully with a logged error.
