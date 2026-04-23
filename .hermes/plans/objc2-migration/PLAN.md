# Migrate `cocoa` / `objc` → `objc2`

**Created:** 2026-04-23  
**Status:** Draft

---

## Problem

`autoplay_media.rs` uses `cocoa::base::id` and `objc::msg_send!` which both trigger deprecation warnings at compile time:

```
warning: use of deprecated type alias `cocoa::base::id`: use the objc2 crate instead
warning: unexpected `cfg` condition value: `cargo-clippy` (from `sel_impl` macro in objc 0.2.7)
```

These come from `cocoa = "0.26"` and `objc = "0.2.7"` — both old crates that predate the modern `objc2` ecosystem. Tauri v2 itself already pulls `objc2 = "0.6.4"` and `objc2-web-kit = "0.3.2"` as transitive dependencies, so the replacement is already in the build graph.

---

## Scope

Only **one file** is affected: `src-tauri/src/autoplay_media.rs`.

Three macOS-only unsafe blocks use the old crates:
1. `configure_autoplay_for_window()` — sets `WKWebViewConfiguration.mediaTypesRequiringUserActionForPlayback` and `WKPreferences.javaScriptCanOpenWindowsAutomatically`
2. `configure_autoplay()` (plugin callback, macOS branch) — same properties
3. `stop_webview()` — calls `[WKWebView stopLoading]`

---

## Current State

| | What | Version |
|-|------|---------|
| Direct dep | `cocoa` | 0.26.1 |
| Direct dep | `objc` | 0.2.7 |
| Transitive dep (via Tauri) | `objc2` | 0.6.4 |
| Transitive dep (via Tauri) | `objc2-web-kit` | 0.3.2 |

---

## Migration Strategy

**Minimal replacement** — swap the raw `id`/`msg_send!` calls for `objc2` equivalents using `objc2::runtime::AnyObject` as the opaque pointer type and `objc2::msg_send!` as the message-send macro. This eliminates all warnings with minimal diff.

A fuller migration using typed `WKWebView`, `WKWebViewConfiguration`, and `WKPreferences` wrappers from `objc2-web-kit` is possible but not required — the typed wrappers add no safety benefit here because we are already behind a `with_webview` closure and the raw pointer cast from `webview.inner()` is inherently unsafe regardless.

---

## Changes

### 1. `src-tauri/Cargo.toml`

Remove the two old crates and add `objc2` as an explicit (non-transitive) dependency.

**Before:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.26"
objc = "0.2.7"
core-foundation = "0.10"
...
```

**After:**
```toml
[target.'cfg(target_os = "macos")'.dependencies]
objc2 = "0.6"
core-foundation = "0.10"
...
```

`cocoa` and `objc` removed. `objc2` declared explicitly so the version is pinned and clear. `objc2-web-kit` does not need to be listed — it remains a transitive dep from Tauri; only `objc2` itself is needed for `AnyObject` and `msg_send!`.

---

### 2. `src-tauri/src/autoplay_media.rs` — imports

**Before:**
```rust
#[cfg(target_os = "macos")]
use cocoa::base::id;
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
```

**After:**
```rust
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::runtime::AnyObject;
```

`sel` and `sel_impl` are not needed — `objc2::msg_send!` handles selector registration internally.

---

### 3. `configure_autoplay_for_window()` — macOS block

**Before:**
```rust
#[cfg(target_os = "macos")]
unsafe {
    let wkwebview: id = webview.inner() as *mut _ as id;
    let config: id = msg_send![wkwebview, configuration];
    let _: () = msg_send![config, setMediaTypesRequiringUserActionForPlayback: 0];
    let preferences: id = msg_send![config, preferences];
    let _: () = msg_send![preferences, setJavaScriptCanOpenWindowsAutomatically: true];
    println!("macOS autoplay configured for window {}", closure_label);
}
```

**After:**
```rust
#[cfg(target_os = "macos")]
unsafe {
    let wkwebview = &*(webview.inner() as *mut AnyObject);
    let config: *mut AnyObject = msg_send![wkwebview, configuration];
    let _: () = msg_send![&*config, setMediaTypesRequiringUserActionForPlayback: 0_usize];
    let preferences: *mut AnyObject = msg_send![&*config, preferences];
    let _: () = msg_send![&*preferences, setJavaScriptCanOpenWindowsAutomatically: true];
    println!("macOS autoplay configured for window {}", closure_label);
}
```

Key changes:
- `id` → `*mut AnyObject` for intermediate pointers, `&AnyObject` as the `msg_send!` receiver (objc2 requires a reference, not a raw pointer)
- `0` → `0_usize` — `WKAudiovisualMediaTypeNone` is a `NSUInteger` (pointer-sized); the explicit type avoids inference ambiguity
- The cast `as *mut _ as id` collapses to `as *mut AnyObject`

---

### 4. `configure_autoplay()` — macOS plugin callback (lines 104-128)

Same substitution as above — identical pattern, identical fix.

**Before:**
```rust
fn configure_autoplay<R: Runtime>(window: Webview<R>) {
    window.with_webview(|webview| {
        unsafe {
            let wkwebview: id = webview.inner() as *mut _ as id;
            let config: id = msg_send![wkwebview, configuration];
            let _: () = msg_send![config, setMediaTypesRequiringUserActionForPlayback: 0];
            let preferences: id = msg_send![config, preferences];
            let _: () = msg_send![preferences, setJavaScriptCanOpenWindowsAutomatically: true];
            println!("macOS autoplay configuration applied");
        }
    }).ok();
}
```

**After:**
```rust
fn configure_autoplay<R: Runtime>(window: Webview<R>) {
    window.with_webview(|webview| {
        unsafe {
            let wkwebview = &*(webview.inner() as *mut AnyObject);
            let config: *mut AnyObject = msg_send![wkwebview, configuration];
            let _: () = msg_send![&*config, setMediaTypesRequiringUserActionForPlayback: 0_usize];
            let preferences: *mut AnyObject = msg_send![&*config, preferences];
            let _: () = msg_send![&*preferences, setJavaScriptCanOpenWindowsAutomatically: true];
            println!("macOS autoplay configuration applied");
        }
    }).ok();
}
```

---

### 5. `stop_webview()` — macOS block (lines ~201-213)

**Before:**
```rust
#[cfg(target_os = "macos")]
{
    match window.with_webview(move |webview| unsafe {
        let wkwebview: cocoa::base::id = webview.inner() as *mut _ as cocoa::base::id;
        let _: () = objc::msg_send![wkwebview, stopLoading];
        println!("macOS: Called [WKWebView stopLoading] on {}", closure_label);
    }) {
        ...
    }
}
```

**After:**
```rust
#[cfg(target_os = "macos")]
{
    match window.with_webview(move |webview| unsafe {
        let wkwebview = &*(webview.inner() as *mut AnyObject);
        let _: () = msg_send![wkwebview, stopLoading];
        println!("macOS: Called [WKWebView stopLoading] on {}", closure_label);
    }) {
        ...
    }
}
```

Note: `stop_webview` uses the fully-qualified `cocoa::base::id` and `objc::msg_send!` inline (not the top-level imports). After this change both the imports and these inline uses are gone, so the `use` statements at the top of the file are the only import site.

---

### 6. Remove `#![allow(unexpected_cfgs)]`

This `allow` attribute at line 2 exists solely to suppress the `sel_impl` macro warnings from `objc 0.2.7`. Once `objc` is removed it can be deleted.

---

## Files Touched

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Remove `cocoa`, `objc`; add `objc2 = "0.6"` |
| `src-tauri/src/autoplay_media.rs` | Swap imports; update 3 unsafe blocks; remove `#![allow(unexpected_cfgs)]` |

---

## What Does NOT Change

- Runtime behaviour is identical — the same Objective-C messages are sent to the same objects
- No change to the `#[cfg(target_os = "macos")]` guards
- No change to Windows or Linux branches
- No change to public API (`configure_autoplay_for_window`, `stop_webview`, `init`)

---

## Verification

- [ ] `cargo check` produces zero warnings related to `cocoa`, `objc`, or `sel_impl`
- [ ] `cargo build` succeeds on macOS
- [ ] Dev mode (`bun run tauri dev`): screensaver windows autoplay video without user gesture
- [ ] Screensaver deactivation: audio stops cleanly (stopLoading called correctly)
- [ ] `cargo check` on Linux and Windows: no regressions in platform-specific branches
- [ ] `cocoa` and `objc` no longer appear in `cargo tree --depth 1` for this crate
