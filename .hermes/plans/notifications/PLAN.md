# Notifications — Remote Feed Polling

**Created:** 2026-04-18  
**Status:** Implemented — see IMPLEMENTATION_SUMMARY.md

---

## Problem

The app has no way to deliver messages from the fork developer (or the gallery operator) to installed users. There is no push API available in the Tauri WebView model, so the only viable approach is client-side polling: the app periodically fetches a JSON feed from a configurable URL and shows new entries as native OS notifications.

---

## Current State

| What exists | What is missing |
|-------------|-----------------|
| `tauri-plugin-updater` (shows plugin registration pattern) | `tauri-plugin-notification` not installed |
| `capabilities/` structure and permission pattern | Notification permissions not declared |
| `.env` / `AppOptions` pattern for env-configured URLs | `VITE_NOTIFICATION_URL` not defined |
| Background spawn pattern (`updater`, `screensaver_engine`) | Notification polling service not implemented |

---

## Notification Feed Format

Fork developers host a JSON file at `VITE_NOTIFICATION_URL`. The app fetches it, filters out already-shown IDs, and displays new entries.

```json
[
  {
    "id": "2026-04-launch",
    "title": "New screensaver themes",
    "body": "10 new themes just dropped. Check them out at the gallery.",
    "url": "https://screensaver.gallery"
  }
]
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `id` | string | yes | Stable identifier — used to deduplicate across restarts |
| `title` | string | yes | OS notification title |
| `body` | string | yes | OS notification body text |
| `url` | string | no | Opened in browser when user clicks the notification (future) |

The app persists shown notification IDs in `options.json` under the key `shownNotificationIds` (a JSON array of strings). An entry is shown once per device, never again.

---

## Plan

### Phase 1 — Add plugin dependency

**`src-tauri/Cargo.toml`** — add to `[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]`:
```toml
tauri-plugin-notification = "2"
```

**`package.json`** — add:
```json
"@tauri-apps/plugin-notification": "^2"
```

### Phase 2 — Register plugin

**`src-tauri/src/main.rs`** — add before `.setup(setup_app)`:
```rust
.plugin(tauri_plugin_notification::init())
```

**`src-tauri/src/lib.rs`** — declare module at the top:
```rust
pub mod notification_service;
```

### Phase 3 — Add capabilities permission

**`src-tauri/capabilities/default.json`** — add to `"permissions"` array:
```json
"notification:default"
```

### Phase 4 — Environment variables

**`.env.example`** — add:
```bash
# Notifications — polling feed
VITE_NOTIFICATION_URL=""                  # Empty = notifications disabled
VITE_NOTIFICATION_CHECK_INTERVAL_SECS=3600  # Default: 1 hour
```

Copy same additions to `.env`.

### Phase 5 — Extend `AppOptions` in `src-tauri/src/lib.rs`

Add two fields to `AppOptions` struct (never persisted — always from env):
```rust
pub notification_url: String,
pub notification_check_interval_secs: u64,
```

Add to `AppOptions::default()`:
```rust
notification_url: std::env::var("VITE_NOTIFICATION_URL").unwrap_or_default(),
notification_check_interval_secs: std::env::var("VITE_NOTIFICATION_CHECK_INTERVAL_SECS")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(3600),
```

`notification_url` and `notification_check_interval_secs` are **not** loaded from the store in `load_persisted_options` — always come from env.

### Phase 6 — New `src-tauri/src/notification_service.rs`

```rust
use std::collections::HashSet;
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::StoreExt;

const SHOWN_IDS_KEY: &str = "shownNotificationIds";

#[derive(serde::Deserialize)]
struct NotificationEntry {
    id: String,
    title: String,
    body: String,
    #[allow(dead_code)]
    url: Option<String>,
}

pub fn start_notification_service<R: Runtime>(app: AppHandle<R>) {
    std::thread::spawn(move || {
        polling_loop(app);
    });
}

fn polling_loop<R: Runtime>(app: AppHandle<R>) {
    loop {
        let (notification_url, interval_secs) = {
            let state = app.state::<super::AppState>();
            let opts = state.options.lock().unwrap();
            (opts.notification_url.clone(), opts.notification_check_interval_secs)
        };

        if notification_url.is_empty() {
            return; // feature disabled — stop the thread
        }

        if let Err(e) = check_and_notify(&app, &notification_url) {
            eprintln!("[notifications] Poll error: {}", e);
        }

        std::thread::sleep(std::time::Duration::from_secs(interval_secs));
    }
}

fn check_and_notify<R: Runtime>(app: &AppHandle<R>, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Fetch feed (blocking HTTP using ureq or reqwest sync)
    let feed: Vec<NotificationEntry> = ureq::get(url)
        .call()?
        .into_json()?;

    // Load already-shown IDs from store
    let store = app.store("options.json")?;
    let shown: HashSet<String> = store
        .get(SHOWN_IDS_KEY)
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let mut updated_shown = shown.clone();

    for entry in &feed {
        if shown.contains(&entry.id) {
            continue;
        }
        show_notification(app, &entry.title, &entry.body);
        updated_shown.insert(entry.id.clone());
    }

    if updated_shown != shown {
        let ids_vec: Vec<&String> = updated_shown.iter().collect();
        store.set(SHOWN_IDS_KEY, serde_json::to_value(&ids_vec)?);
        store.save()?;
    }

    Ok(())
}

pub fn show_notification<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}
```

**HTTP dependency** — add `ureq = { version = "2", features = ["json"] }` to `Cargo.toml` (lightweight blocking HTTP, no async runtime needed for a background thread).

### Phase 7 — Wire up in `src-tauri/src/lib.rs`

In `setup_app`, after the updater spawn (lines ~201-207), add:
```rust
if !options_snapshot.notification_url.is_empty() {
    let handle = app.handle().clone();
    std::thread::spawn(move || {
        notification_service::polling_loop(handle);
    });
}
```

Where `options_snapshot` is read from state before the block (same pattern as updater).

Actually, simpler — just call the public entry point:
```rust
notification_service::start_notification_service(app.handle().clone());
// start_notification_service returns immediately if URL is empty
```

---

## Files Touched

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add `tauri-plugin-notification`, `ureq` |
| `package.json` | Add `@tauri-apps/plugin-notification` |
| `src-tauri/src/main.rs` | Register notification plugin |
| `src-tauri/capabilities/default.json` | Add `notification:default` permission |
| `.env.example` | Add `VITE_NOTIFICATION_URL`, `VITE_NOTIFICATION_CHECK_INTERVAL_SECS` |
| `.env` | Same additions |
| `src-tauri/src/lib.rs` | Add `notification_url` + `notification_check_interval_secs` to `AppOptions`; `pub mod notification_service`; call `start_notification_service` in `setup_app` |
| `src-tauri/src/notification_service.rs` | New file — polling loop, deduplication, show helper |

---

## What's NOT in This Plan

- **UI toggle for enabling/disabling notifications** — the URL being empty is the off switch; fork developers control this per-deployment. No per-user preference needed for v1.
- **Click-to-open URL** — `tauri-plugin-notification` action support varies by platform. Deferred.
- **liminal-api `showNotification()` method** — useful for remote options pages to trigger their own notifications. Deferred to a follow-up.
- **Notification grouping / bundling** — macOS supports grouping; out of scope for v1.

---

## Verification

- [ ] `cargo check` passes with no new errors
- [ ] With `VITE_NOTIFICATION_URL=""`: notification thread exits immediately, no network calls
- [ ] With a valid URL returning a JSON array: notifications appear as OS banners
- [ ] Already-shown IDs in `options.json` are not re-shown on subsequent polls
- [ ] After factory reset (`factory_reset_options`): `shownNotificationIds` is cleared by store.clear(); next poll re-shows all notifications
- [ ] Invalid/unreachable URL: error logged to stderr, app continues normally
- [ ] Malformed JSON response: error logged, no crash
- [ ] `VITE_NOTIFICATION_CHECK_INTERVAL_SECS=60`: poll runs every 60 seconds
