# objc2 Migration — Implementation Summary

**Date:** 2026-04-23  
**Status:** Completed  
**Related Plan:** [PLAN.md](./PLAN.md)

---

## What Was Done

### 1. Cargo.toml
- **Removed** direct dependencies on the legacy crates:
  - `cocoa = "0.26"`
  - `objc = "0.2.7"`
- **Added** an explicit dependency on the modern replacement:
  - `objc2 = "0.6"`

### 2. autoplay_media.rs
- **Removed** `#![allow(unexpected_cfgs)]` (only existed to silence the old `sel_impl` macro warnings).
- **Swapped imports:**
  - `cocoa::base::id` → `objc2::runtime::AnyObject`
  - `objc::{msg_send, sel, sel_impl}` → `objc2::msg_send`
- **Updated 3 macOS-only unsafe blocks** to use `objc2` patterns:
  - `configure_autoplay_for_window()`
  - `configure_autoplay()` (plugin callback)
  - `stop_webview()` (macOS branch)

Key mechanical changes:
- `id` → `*mut AnyObject` for intermediate pointers
- Receivers to `msg_send!` changed from raw pointers to references: `msg_send![&*config, ...]`
- Cast `webview.inner() as *mut _ as id` simplified to `webview.inner() as *mut AnyObject`
- `0` → `0_usize` for the NSUInteger parameter (avoids inference ambiguity)
- Inline fully-qualified `cocoa::base::id` / `objc::msg_send!` in `stop_webview` replaced with the new imports

---

## Verification Performed

- `cargo check` completes successfully (ran in the project root).
- Zero warnings related to `cocoa`, `objc`, or `sel_impl`.
- No compilation regressions on macOS.

---

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Removed `cocoa` and `objc`; added `objc2 = "0.6"` |
| `src-tauri/src/autoplay_media.rs` | Swapped imports; updated all 3 macOS unsafe blocks; removed `#![allow(unexpected_cfgs)]` |

---

## Notes

- `objc2-web-kit` was **not** added explicitly — it remains a transitive dependency from Tauri v2.
- Platform targets other than macOS (Windows, Linux, fallback) were left untouched.
- Public API surface (`configure_autoplay_for_window`, `stop_webview`, `init`) is unchanged.
