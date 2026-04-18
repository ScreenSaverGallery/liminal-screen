# Native WebView Storage Cleanup (Window-Independent)

**Created:** 2026-04-18  
**Status:** Draft

---

## Context

`clean_browser_storage()` in `lib.rs` uses `window.eval()` to clear localStorage, sessionStorage, Cache API entries, and unregister service workers. This is a fundamental limitation: `eval()` only operates on **open** windows. Screensaver windows are closed when factory reset is triggered — the JS approach misses them entirely.

Platform-native WebView data store APIs solve this. They operate directly on the stored data on disk, independent of whether any window is open.

This plan adds a complementary `website_data_manager.rs` module. The existing `clean_browser_storage()` is kept — it handles in-memory JS state (active service worker clients, JS heap variables) for any windows that happen to be open. The new module handles persisted storage unconditionally.

---

## Key Finding: `objc2-web-kit` Already in Lockfile

Tauri's internal `wry`/`tauri-runtime-wry` already pulls in `objc2-web-kit` with `WKWebsiteDataStore` + `block2` features. Adding it as a direct dep simply makes it explicit — no new downloads, no version conflicts.

---

## Plan

### Phase 1 — Cargo.toml: Add macOS deps

**File:** `src-tauri/Cargo.toml`

Add to `[target.'cfg(target_os = "macos")'.dependencies]`:

```toml
objc2              = "0.6"
objc2-foundation   = { version = "0.3", default-features = false, features = ["NSDate", "NSSet", "NSString"] }
objc2-web-kit      = { version = "0.3", default-features = false, features = ["WKWebsiteDataStore", "block2"] }
block2             = { version = "0.6", default-features = false, features = ["alloc"] }
```

Do **not** add `block = "0.1"` (old transitive dep of `cocoa 0.26` — different crate, don't mix generations).

---

### Phase 2 — New module: `src-tauri/src/website_data_manager.rs`

```rust
/// Clears persisted WebView storage using platform-native data store APIs.
/// Works regardless of whether any windows are open.
/// Complements clean_browser_storage() which handles in-memory state via eval().
pub fn clear_website_data<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    #[cfg(target_os = "macos")]
    clear_macos(app);

    #[cfg(not(target_os = "macos"))]
    let _ = app; // Windows/Linux: see LIMITATIONS
}

#[cfg(target_os = "macos")]
fn clear_macos<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    let result = app.run_on_main_thread(|| {
        // SAFETY: run_on_main_thread guarantees we are on the main thread.
        // WKWebsiteDataStore is MainThreadOnly in objc2's type system.
        unsafe {
            use objc2::MainThreadMarker;
            use objc2_foundation::NSDate;
            use objc2_web_kit::WKWebsiteDataStore;
            use block2::RcBlock;

            let mtm = MainThreadMarker::new_unchecked();

            // allWebsiteDataTypes() covers localStorage, sessionStorage,
            // disk/memory cache, service workers, IndexedDB, cookies, etc.
            // Avoids hardcoding string constants or adding WKWebsiteDataRecord feature.
            let data_types = WKWebsiteDataStore::allWebsiteDataTypes(mtm);
            let since_date  = NSDate::distantPast();
            let store        = WKWebsiteDataStore::defaultDataStore(mtm);

            // Fire-and-forget: the RcBlock is ref-counted and released by the
            // ObjC runtime after the completion handler fires.
            let completion = RcBlock::new(|| {
                println!("[website_data_manager] macOS: WKWebsiteDataStore cleared");
            });

            store.removeDataOfTypes_modifiedSince_completionHandler(
                &data_types,
                &since_date,
                &completion,
            );
        }
    });

    if let Err(e) = result {
        eprintln!("[website_data_manager] Failed to dispatch to main thread: {}", e);
    }
}
```

**Why `allWebsiteDataTypes`**: avoids needing the `WKWebsiteDataRecord` Cargo feature and automatically includes any future storage types WebKit adds.

**Why `distantPast`**: clears everything ever stored. Correct for a factory reset. Per-origin filtering (fetch records → filter → remove) is possible but requires the two-step async pattern and the `WKWebsiteDataRecord` feature — not needed here.

**Why `run_on_main_thread`**: WKWebsiteDataStore must be called from the main thread. `run_on_main_thread` dispatches to the Tauri app's main run loop. The call is fire-and-forget; `factory_reset_options` does not wait for the async completion.

---

### Phase 3 — Integrate into `lib.rs`

**Change 1 — Declare module** (after existing `pub mod` declarations, ~line 7):
```rust
pub mod website_data_manager;
```

**Change 2 — Call from `factory_reset_options`** (after existing `clean_browser_storage` call):
```rust
clean_browser_storage(&app);                       // existing: JS eval on open windows
website_data_manager::clear_website_data(&app);    // new: platform-native, always runs
```

Order: `clean_browser_storage` first (handles live windows), `clear_website_data` second (async cleanup, fires via main thread dispatch).

---

## Files Touched

| File | Change |
|------|--------|
| `src-tauri/Cargo.toml` | Add 4 macOS deps |
| `src-tauri/src/website_data_manager.rs` | NEW — macOS impl + stubs |
| `src-tauri/src/lib.rs` | Declare module, add call in `factory_reset_options` |

---

## Limitations

**Windows** — `ICoreWebView2Profile::ClearBrowsingData` is the correct API but requires `webview2-com` (not in Cargo.toml). `clean_browser_storage()` (eval fallback) remains the only mechanism on Windows. Adding Windows support is a separate task.

**Linux** — `webkit2gtk::WebsiteDataManager::clear()` is available via existing dep but needs a `WebContext` reference, which requires a live `WebView`. Effectively window-dependent too. Mark as follow-up.

**Per-origin filtering** — `allWebsiteDataTypes + distantPast` clears ALL origins in the default data store, including cookies. For factory reset this is intentional. If only the saver/options URL origins should be cleared, it requires the two-step fetch+filter+remove pattern.

**In-memory only state** — Active service worker clients, JS heap, `AudioContext` etc. are not reachable by the data store API. `clean_browser_storage()` handles those for open windows.

---

## Verification

- [ ] `cargo check` passes with no new errors
- [ ] `cargo tree` shows `objc2-web-kit` at a single unified version (no duplicates)
- [ ] Factory reset with all windows closed: check console for `WKWebsiteDataStore cleared` log
- [ ] After reset, open a new screensaver/preview window: confirm `localStorage` is empty for the saver URL's origin
- [ ] Factory reset with windows open: both `clean_browser_storage` and `clear_website_data` fire without error or crash
- [ ] App restart after reset: storage remains empty (confirms disk persistence was cleared, not just in-memory)
