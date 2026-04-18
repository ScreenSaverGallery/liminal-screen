# Cleanup Report: eval-based browser storage approach removed

**Date:** 2026-04-18

## What Was Removed

The `window.eval()` approach for clearing browser storage during factory reset was determined to be ineffective for the screensaver use case — screensaver windows are closed when factory reset is triggered, so the eval never runs on the windows that matter. The code was removed rather than left as dead weight.

### Removed from `src-tauri/src/lib.rs`

| Item | Reason |
|------|--------|
| `SAVER_WINDOW_PREFIX` constant | Only existed to support `clean_browser_storage` loop |
| `PREVIEW_WINDOW_PREFIX` constant | Only existed to support `clean_browser_storage` loop |
| `BROWSER_STORAGE_CLEANUP_JS` constant | The eval script itself |
| `clean_browser_storage()` function | Core of the eval approach — window-dependent |
| `clean_window_browser_storage` Tauri command | Frontend-callable variant for preview windows |
| `clean_window_browser_storage` from `invoke_handler` | Registration removed with the command |
| `clean_browser_storage(&app)` call in `factory_reset_options` | Call site removed |

### Removed from `src/main.ts`

| Item | Reason |
|------|--------|
| `invoke("clean_window_browser_storage", ...)` block in reset handler | Called the now-removed command |

## What Remains

`factory_reset_options` still clears `options.json` and resets in-memory state. The `instance_id` is regenerated on every reset — this is now the foundation for the replacement strategy (see `RETHINK.md`).

## Follow-up

See `../native-storage-cleanup/RETHINK.md` for the alternative concept: delegate cleanup responsibility to the loaded page by using the changed `navigator.id` as a self-contained signal. The page detects an `instance_id` mismatch against its stored value and clears its own storage — no native APIs, no open windows required.
