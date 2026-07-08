# Plan: Multi-Platform Fixes & Rust Hardening

**Created:** 2026-07-08
**Status:** Implemented

---

## Problem / Context

The project was only ever compiled on macOS. A cross-target review (requirement:
macOS, Windows, Linux — both Wayland and X11) found Windows/Linux compile
blockers, several runtime bugs, and no Wayland support at all.

## Findings & Fixes

### Compile blockers (would not build on Windows/Linux)

| File | Bug | Fix |
|------|-----|-----|
| `autoplay_media.rs` | Linux: `webview.inner()` cast to `*mut gtk::Widget` — but Tauri v2's `PlatformWebview::inner()` returns `webkit2gtk::WebView` by value on Linux | Use the returned `WebView` directly via `SettingsExt`/`WebViewExt` |
| `autoplay_media.rs` | Linux: `WebViewExt::load_blank` does not exist | `stop_loading()` + `load_uri("about:blank")` |
| `autoplay_media.rs` | Windows: `webview.controller()` treated as `Option` — it returns `ICoreWebView2Controller` directly | Removed the `if let Some(...)` |
| `power_monitor.rs` | Windows: `GetTickCount` imported from `Win32::System::SystemServices` — lives in `SystemInformation` in windows-rs 0.62 | Fixed import + Cargo feature |
| `Cargo.toml` | `glib = "0.22"` alongside `gtk = "0.18"` (which pins glib 0.18) — incompatible tree | Dropped gtk/glib; `webkit2gtk = { version = "2", features = ["v2_40"] }` matching tauri |

Windows API usage was validated with a standalone crate cross-checked against
`x86_64-pc-windows-msvc` (a full app cross-check is blocked by `ring`'s C build).
Linux API names were verified against the vendored webkit2gtk-2.0.2 sources.

### Runtime bugs

- **Windows idle time**: `tick_count - last_input.dwTime` overflows at the
  49.7-day tick wrap → `wrapping_sub`.
- **Windows sleep inhibition**: `SetThreadExecutionState(ES_CONTINUOUS)` is
  per-thread and cleared when the thread dies; Tauri commands may run on
  short-lived threads. Now funneled to one dedicated long-lived thread.
- **Linux inhibitor never released**: `pkill -f "systemd-inhibit.*liminal-screen"`
  never matched because `--who` uses `VITE_APP_NAME`. macOS/Linux inhibitor
  children (`caffeinate`/`systemd-inhibit`) are now stored as `Child` handles in
  a static and killed **and reaped** (the old kill-by-pattern also leaked zombies).
- **Linux lock/blank false success**: `spawn().is_ok()` only proves the binary
  exists (e.g. `xset` on Wayland "succeeds" but does nothing). All command
  chains now check exit status.
- **Release builds lose fork identity**: `std::env::var("VITE_*")` reads the
  *runtime* environment — a bundled app launched from Finder has none, so
  `saver_url` fell back to `about:blank` in production. New `env_setting!`
  macro: runtime env first, then `option_env!` (compile-time baked) fallback.
- **`options-updated` event never emitted**: `main.ts` and liminal-api's
  `startAutoSync` listened for it, but `set_options` never emitted it —
  cross-window sync was dead. `set_options` and `factory_reset_options` now emit.
- **Linux battery heuristic**: "no AC adapter entry → assume battery" put every
  desktop PC into battery mode (blocking the screensaver by default). Now only
  assumes battery when a `BAT*` device exists.

### Wayland support (new)

- Idle time on Linux now tries, in session-type-aware order: `xprintidle` (X11),
  Mutter IdleMonitor D-Bus (GNOME X11+Wayland), `org.freedesktop.ScreenSaver.
  GetSessionIdleTime` (KDE). The last working method is cached to avoid
  spawning three processes per tick.
- Lock: `loginctl lock-session` (X11+Wayland) → D-Bus ScreenSaver.Lock →
  `xdg-screensaver` → `gnome-screensaver-command`.
- Blank: `kscreen-doctor --dpms off` (KDE Wayland) / `xset dpms force off` (X11)
  with fallbacks. GNOME Wayland has no stable CLI for forced DPMS off — degrades
  to screensaver activation.

### macOS improvements

- Idle time: `CGEventSourceSecondsSinceLastEventType` FFI instead of spawning
  `ioreg`/`osascript` every second (ioreg parse kept as fallback).
- Battery: `IOPSCopyPowerSourcesInfo`/`IOPSGetProvidingPowerSourceType` IOKit
  FFI instead of spawning `pmset` every second (pmset kept as fallback).
- Removed unused deps: `io-kit-sys`, `mach2`, `core-graphics`, `urlencoding`.

### Windows autoplay

WebView2 has no runtime autoplay switch; `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`
is now set to `--autoplay-policy=no-user-gesture-required` in `run()` before the
first webview is created.

### Engine refactor

`check_and_manage_screensaver`'s priority logic extracted into the pure
`compute_next_action()` (unit-tested), and `get_saver_url`'s query-param
building into pure `build_saver_url()` (unit-tested). Also fixed: an
already-`Locked` state no longer re-triggers the lock path every tick.

## Verification

- `cargo check`, `cargo clippy` (0 warnings), `cargo fmt --check`, `cargo test`
  (29 tests) — green on macOS
- Windows API usage compiled against `x86_64-pc-windows-msvc` in isolation
- webkit2gtk method names/feature gates verified against crate sources
- Manual verification still needed on real Windows / Linux X11 / Linux Wayland
  machines (see Open Questions)

## Open Questions

- KDE's `GetSessionIdleTime` unit is assumed to be seconds (per the fd.o spec);
  verify on a real KDE session.
- `capabilities/options.json` and `saver.json` target remote-URL windows but do
  not declare a `remote` scope — per Tauri v2 docs, capabilities apply to local
  content only unless `remote.urls` is set. Remote options reportedly work in
  practice; worth revisiting together with the security plan.
