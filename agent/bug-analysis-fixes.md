# Screensaver Engine Bug Analysis & Fixes

## Bugs Found

### Bug 1 (CRITICAL): Window creation on background thread
**Location**: `screensaver_engine.rs`, `activate_screensaver()` and `create_saver_window()`

**Problem**: The monitoring loop runs on a background thread (`std::thread::spawn`), but `WebviewWindowBuilder::new().build()` **MUST** be called on the main thread in Tauri v2. When called from a background thread, the window creation silently fails — the `println!` before the builder call executes, but the actual `.build()` call returns an error or the window is created in an invalid state.

**Symptoms**: Console shows "Creating saver window for monitor..." but no window appears.

**Fix**: Use `app.run_on_main_thread()` to dispatch all window creation/destruction operations to the main event loop. The background monitoring thread now only detects idle state transitions and sends requests to the main thread.

### Bug 2 (CRITICAL): Power management functions require Tauri State context
**Location**: `screensaver_engine.rs`, `activate_screensaver()` and `deactivate_screensaver()`

**Problem**: The original code called `prevent_display_sleep(app.clone(), app.state())` which passes `State<PowerSaveBlocker>` obtained from `app.state()`. The `State<T>` type from `app.state()` in Tauri is designed for use within command handler contexts. Calling this from the background thread (or even from `run_on_main_thread` closures) doesn't have a valid request context, causing failures.

**Fix**: Added `prevent_display_sleep_direct()` and `allow_display_sleep_direct()` public functions in `power_monitor.rs` that don't require the `State<PowerSaveBlocker>` wrapper. These call the platform-specific sleep prevention directly.

### Bug 3: `window.eval()` and `window.close()` on background thread
**Location**: `screensaver_engine.rs`, `close_all_savers()`

**Problem**: JavaScript evaluation (`window.eval()`) and window closing (`window.close()`) must be called on the main thread in Tauri v2. The original code called these from the background thread.

**Fix**: These calls are now made on the main thread via `run_on_main_thread()` dispatch.

### Bug 4: `window.set_fullscreen()` on background thread
**Location**: `screensaver_engine.rs`, `create_saver_window()`

**Problem**: Fullscreen mode setting must be done on the main thread.

**Fix**: Now runs on main thread via dispatch.

### Bug 5: Duplicate window creation
**Location**: `screensaver_engine.rs`, `create_saver_window()`

**Problem**: If the monitoring thread dispatched multiple activation requests before the first one was processed, duplicate windows with the same label could be attempted, causing errors.

**Fix**: Added `pending_transition` AtomicBool flag to prevent duplicate dispatches. Also added a check `app.get_webview_window(&label).is_some()` before creating a window to prevent label conflicts.

### Bug 6: `allow_sleep_linux()` missing return value
**Location**: `power_monitor.rs`, `allow_sleep_linux()`

**Problem**: The function signature returns `Result<(), String>` but was missing `Ok(())` at the end. This would be a compile error.

**Fix**: Added `Ok(())` at the end of the function.

### Bug 7: `std::thread::sleep` on main thread
**Location**: `screensaver_engine.rs`, `close_all_savers()`

**Problem**: `std::thread::sleep(Duration::from_millis(100))` was called on the main thread (after the fix to dispatch close operations to main thread), which would freeze the entire UI.

**Fix**: Removed the sleep from `close_all_savers()`. The `about:blank` navigation is fire-and-forget; closing immediately after is acceptable.

### Bug 8: Stub commands that did nothing
**Location**: `lib.rs`, `activate_screensaver_command()` and `deactivate_screensaver_command()`

**Problem**: These commands were empty stubs that returned `Ok(())` without doing anything. The JS code calls `invoke("deactivate_screensaver_command")` which did nothing.

**Fix**: Made these commands properly call `engine.activate_screensaver()` / `engine.deactivate_screensaver()` directly (since command handlers already run on the main thread).

## Architecture Summary (Post-Fix)

```
Background Thread (monitoring)      Main Thread (window management)
─────────────────────────          ────────────────────────────────
Every 1 second:
  1. Get idle time (OS API)        
  2. Read app state (thread-safe)  
  3. Check battery status          
  4. Detect transition             
     ─── run_on_main_thread ───→   5. Create/destroy windows
                                    6. Set fullscreen
                                    7. Emit events
                                    8. Update is_active flag
                                    9. Prevent/allow display sleep
```

## Files Modified
1. `src-tauri/src/screensaver_engine.rs` — Complete rewrite of activation/deactivation flow
2. `src-tauri/src/power_monitor.rs` — Added `prevent_display_sleep_direct()` and `allow_display_sleep_direct()` functions
3. `src-tauri/src/lib.rs` — Fixed stub commands to actually call engine methods