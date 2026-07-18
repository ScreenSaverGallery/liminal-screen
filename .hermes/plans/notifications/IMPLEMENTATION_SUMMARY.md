# Notifications — Implementation Summary

**Implemented:** 2026-07-08

Implemented as planned, with these deviations:

- Poll interval is clamped to a minimum of 60 s.
- The HTTP client uses a shared `ureq::Agent` with a 30 s timeout (the plan's
  bare `ureq::get` has no timeout — a hung server would stall the thread).
- At most 5 notifications are shown per poll (`MAX_PER_POLL`); remaining unseen
  entries surface on later polls instead of flooding a fresh install with the
  feed's entire history. Entries are only marked shown after actually showing.
- Plugin registered in `lib.rs::run()` (not `main.rs` — registration lives in
  `run()` in this codebase).
- `notification_url` / `notification_check_interval_secs` were added to
  `AppOptions` with `#[serde(default)]` so older payloads (and liminal-api
  `set_options` calls that don't know the fields) still deserialize; the fields
  are env-only and preserved server-side in `set_options` like other identity
  fields.

Files touched: `src-tauri/Cargo.toml` (tauri-plugin-notification, ureq),
`src-tauri/src/notification_service.rs` (new), `src-tauri/src/lib.rs`,
`src-tauri/capabilities/default.json` (notification:default),
`.env.example`, `.env`, `src/app/types.ts`.

## Follow-up: user consent (2026-07-08)

Notifications are now **opt-in**:

- New persisted, user-settable `notifications_enabled` option (store key
  `notificationsEnabled`), default `false` (overridable via
  `VITE_DEFAULT_NOTIFICATIONS_ENABLED`). `#[serde(default)]` guarantees a
  missing field in older payloads deserializes to `false` — consent is never
  implicitly granted (unit-tested).
- "Allow Notifications" checkbox in the options window settings form; the row
  is hidden when the fork ships no `VITE_NOTIFICATION_URL`.
- The polling thread never fetches or shows anything while consent is off; it
  re-checks every 60 s so enabling takes effect without a restart. It also
  checks/requests OS-level permission via the plugin before showing (currently
  a Granted stub on desktop, future-proofing).
- liminal-api: `notificationsEnabled?` added to `MandatoryOptions` (optional in
  payloads — `setOptions()` merges with current options), required on
  `AppOptions` together with read-only `notificationUrl` /
  `notificationCheckIntervalSecs`.

## Follow-up: dropped tauri-plugin-notification (2026-07-18)

`tauri-plugin-notification` injects a `window.Notification` shim into **every**
webview via a plugin init script, and that shim unconditionally calls
`plugin:notification|is_permission_granted` over IPC at page load. In the
remote saver/preview windows (https origin, no notification capability) this
produced console errors on every load: "[blocked] requested insecure content
from ipc://…", "Not allowed to request resource", and an unhandled rejection
"notification.is_permission_granted not allowed on window preview-…". The
plugin offers no way to scope the injection per-window.

Since notifications are shown from Rust only (the JS API was never used), the
plugin was replaced with a direct `notify-rust = "4.11"` dependency — the same
crate the plugin's desktop backend wraps. `show_notification` replicates the
plugin's platform handling (macOS `set_application` with the bundle id, or
Terminal's in dev; Windows AUMID only when installed). `ensure_os_permission`
was removed — the plugin's desktop permission API was a hardcoded `Granted`
stub, so nothing real was lost. Removed `.plugin(tauri_plugin_notification::
init())` from `lib.rs` and `notification:default` from
`capabilities/default.json`.

No webview gets notification code injected anymore, so the errors are gone in
all windows; the feed poller keeps working from its background thread
regardless of which windows exist (including main hidden in the tray).
Windows-only code verified via scratch-crate `cargo check --target
x86_64-pc-windows-msvc` (the app itself can't cross-check — `ring`).
