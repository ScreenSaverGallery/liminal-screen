# Plan: Preview Window Deadlock on Windows (White Screen, App Unresponsive)

**Created:** 2026-07-17
**Status:** Proposed (code revision only — written on macOS, fixes must be applied and tested on a Windows machine)

---

## Reported Symptom

On Windows, opening the screensaver **preview window** breaks the app:

- The preview window appears but shows only a **white background** — no content.
- The preview window **cannot be closed** (the X button does nothing).
- The **entire app becomes unresponsive** — the options window and tray menu
  stop reacting; the app cannot be quit normally.

macOS is unaffected. Linux status unknown (see analysis — likely unaffected by
the primary bug).

---

## Root Cause (primary, confirmed against upstream docs)

`create_preview_window` in `src-tauri/src/lib.rs:452` is a **synchronous**
`#[tauri::command]` that calls `WebviewWindowBuilder::build()`.

This is a documented, Windows-only deadlock in Tauri v2. The
[`WebviewWindowBuilder` rustdoc](https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html)
states verbatim:

> "On Windows, this function deadlocks when used in a synchronous command and
> event handlers, see [the Webview2 issue](https://github.com/tauri-apps/wry/issues/583).
> You should use `async` commands and separate threads when creating windows."

### Mechanism

1. On Windows, Tauri IPC (`invoke`) is delivered through a **WebView2 callback
   on the UI thread** (WebResourceRequested/WebMessageReceived). A *synchronous*
   command handler runs inline, inside that callback, with the callback still
   on the stack.
2. `WebviewWindowBuilder::build()` creates the Win32 window (HWND) immediately,
   then blocks waiting for WebView2's asynchronous
   `CreateCoreWebView2Controller` completion callback.
3. WebView2 enforces **re-entrancy protection**: it will not deliver a new
   callback while another WebView2 callback is executing. The controller-created
   callback therefore queues forever behind the still-running IPC callback.
4. Deadlock. The HWND exists but the browser controller never attaches →
   **white surface**. The UI thread (which owns the message loop for *all*
   windows and the tray) is stuck inside the command → **nothing can be closed,
   the whole app freezes**. This matches the reported symptom exactly.

### Why only Windows

- **macOS**: `WKWebView` creation is synchronous on the main thread — no async
  controller handshake, nothing to wait for.
- **Linux (X11 & Wayland)**: WebKitGTK's `WebView` is likewise constructed
  synchronously on the GLib main loop; same reasoning. (Identical behavior on
  X11 and Wayland — the deadlock mechanism is a WebView2 threading property,
  not a display-server one.)

### Trigger path

Every preview goes through the deadlocking command:

- Options UI "Preview" button, or tray → "Preview Screensaver" → `preview_screensaver`
  command emits `preview-screensaver` (`src-tauri/src/lib.rs:428`)
- → `main.ts` listener → `previewScreensaver()` (`src/main.ts:329`)
- → `Preview.show()` → `invoke("create_preview_window", …)`
  (`src/app/preview/preview.ts:19`)
- → sync command builds the window inside the IPC callback → deadlock.

Note the saver windows do **not** deadlock: `ScreensaverEngine` creates them via
`run_on_main_thread` from its own monitoring thread
(`screensaver_engine.rs:227`), i.e. from a plain event-loop dispatch, *not* from
inside a WebView2 callback — re-entrancy protection doesn't apply there.

---

## Secondary findings (latent bugs found during the same review)

### S1. `open_options` has the same deadlock class

`open_options` (`lib.rs:437`) is also a **sync** command that builds a window
(`open_options_window`, `lib.rs:379`). It deadlocks the same way whenever it is
invoked over IPC — e.g. the `open-options-window` event listener in
`src/main.ts:97` does `invoke("open_options")`. It has probably not been
reported yet because the usual entry point is the tray menu, which calls
`open_options_or_fallback` natively.

The rustdoc quoted above also flags **"event handlers"** — the tray
`on_menu_event` closure (`lib.rs:289`) builds windows inline on the main
thread. Tray events originate from tao's own event loop rather than a WebView2
callback, so this path *appears* to work today, but upstream explicitly labels
it unsafe. Both tray entries ("options", "preview") should be dispatched off
the handler rather than run inline.

### S2. Preview windows are never destroyed — hidden zombie windows leak (all platforms)

Chain of problems in `Preview` (`src/app/preview/preview.ts`) + capabilities:

1. `Preview.show()` registers `onCloseRequested`. In Tauri v2, once such a
   listener exists, the window is **not** closed automatically; the JS wrapper
   runs the handler and then calls `window.destroy()` unless `preventDefault()`
   was called.
2. That implicit `destroy()` executes with the **caller window's** (main)
   permissions — and `core:window:allow-destroy` is granted **nowhere**
   (`capabilities/default.json` grants `allow-close` but not `allow-destroy`;
   `core:default`'s window set contains only getters — verified against
   `gen/schemas/acl-manifests.json`). The `destroy()` is rejected by the ACL,
   the rejection is swallowed, and the window survives.
3. Meanwhile the close handler calls `hide()`, which calls `close()` **again**
   from inside the close-requested handling → a re-entrant second
   close-requested cycle → `onClose` fires twice (`previewActive` toggles
   twice) and a second doomed `destroy()` is attempted.
4. Net effect: each preview cycle leaves a **hidden, never-destroyed webview**
   (`preview-<timestamp>` labels are unique, so they accumulate). On Windows
   that is a leaked WebView2 process per preview; on macOS/Linux a leaked
   WKWebView/WebKitGTK view. The `onCloseRequested` unlisten function is also
   discarded, leaking the listener.

### S3. Minor notes

- `Preview.hide()` relies on a blind 100 ms `setTimeout` before closing — a
  race, not a correctness guarantee. Acceptable, but worth replacing when S2 is
  fixed (navigate-then-destroy makes the wait mostly moot).
- No capability targets `preview-*` windows. The preview loads remote content
  that shouldn't get IPC, so this is fine — but it's the same "remote windows
  without a `remote` scope" open question already tracked in
  `.hermes/plans/security/PLAN.md`.

---

## Proposed Solutions

### Fix 1 (primary, recommended): make window-creating commands `async`

The upstream-documented fix. Async commands run on the async runtime instead of
inside the WebView2 callback; `build()` internally dispatches window creation
to the main thread, which is now free to pump messages.

```rust
#[tauri::command]
async fn create_preview_window<R: Runtime>(
    app: AppHandle<R>,
    url: String,
    label: String,
) -> Result<(), String> {
    // body unchanged
}

#[tauri::command]
async fn open_options(app: AppHandle) -> Result<(), String> {
    open_options_or_fallback(&app)
}
```

Cross-platform assessment:

- **Windows**: removes the deadlock (documented pattern).
- **macOS / Linux (X11 + Wayland)**: `build()` from a non-main thread is
  supported — Tauri dispatches to the main thread internally. No behavior
  change.
- **Ordering preserved**: the JS `await invoke(...)` still resolves only after
  `build()` returns, so the `WebviewWindow.getByLabel()` immediately following
  in `preview.ts:21` keeps working. (This is why the alternative — keeping the
  command sync and fire-and-forgetting into `tauri::async_runtime::spawn` — is
  worse: it would race `getByLabel`.)
- **Rust caveat**: `WebviewWindowBuilder` is not `Send`. Keep the entire
  builder-chain + `.build()` in one synchronous block with **no `.await` in
  between** (true of the current bodies), or the command won't compile.

Also apply to the tray handler (per the same rustdoc warning about event
handlers): in `on_menu_event`, wrap the two window-creating branches:

```rust
"options" => {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = open_options_or_fallback(&app);
    });
}
```

(`open_options_or_fallback` only needs an `AppHandle`, which is `Send + Clone`,
so this is a mechanical change.)

### Fix 1b (optional follow-up): create the preview from Rust, like the saver windows

The saver windows never deadlock because their `build()` reaches the main
thread as a plain event-loop dispatch (engine background thread →
`run_on_main_thread`) while the main thread is free. The preview could use the
same architecture: have `preview_screensaver` create the window directly
(async command or spawned task) instead of the current round-trip
(Rust emits `preview-screensaver` → JS in the hidden main window →
`invoke("create_preview_window")` → Rust). JS would only consume
`preview-started` / `preview-ended` events for the `previewActive` UI state.

Additional benefits beyond the deadlock:

- Preview no longer depends on the main window's JS context being alive.
- Fixes a fidelity gap: `main.ts:329` builds the preview URL from
  `import.meta.env.VITE_SAVER_URL` **without** the user's `customOptions`
  query params, whereas real savers go through `get_saver_url()`
  (`screensaver_engine.rs:549`), which appends them. A Rust-side preview
  reusing `get_saver_url()` shows exactly what the screensaver will show.
- One shared window-creation helper (init script, autoplay config, URL
  building) instead of two diverging copies.

Trade-off: a small refactor of the event flow and `main.ts` versus the
two-keyword Fix 1. Suggested sequence: land Fix 1 first to confirm the
diagnosis on Windows, then consolidate with Fix 1b as cleanup. Note that
`run_on_main_thread` from *inside* the current sync command is NOT a
substitute — the main thread is the one blocked in the IPC callback, so the
closure would queue behind itself (see rejected alternatives).

### Fix 2: correct the preview teardown (fixes S2, all platforms)

1. Add `core:window:allow-destroy` to `src-tauri/capabilities/default.json`
   (and `options.json` if the options window ever drives previews directly).
2. Rework `Preview`:
   - keep the unlisten function returned by `onCloseRequested` and call it in
     `hide()`;
   - the close-requested handler must **not** call `hide()`/`close()` — just do
     cleanup (optional `navigate about:blank`), call `onClose`, and let the JS
     wrapper's implicit `destroy()` run (it works once the permission exists);
   - programmatic `hide()` should unlisten first, navigate to `about:blank`,
     then call `destroy()` directly instead of `close()` — `close()` from code
     re-enters the close-requested machinery; `destroy()` doesn't.

Cross-platform assessment: `destroy()` is a plain Tauri API implemented on all
three platforms; no X11/Wayland divergence. On Windows this also releases the
per-preview WebView2 process (verify in Task Manager, see below).

### Explicitly rejected alternatives

- **`std::thread::spawn` inside a sync command** (the rustdoc's second
  pattern): works, but loses error propagation to JS and breaks the
  await-then-`getByLabel` ordering without extra synchronization. `async fn` is
  strictly simpler here.
- **`run_on_main_thread` from the sync command**: does *not* help on Windows —
  the sync command already blocks the very thread that would run the closure.
- **Creating the preview from JS (`new WebviewWindow(...)`)**: was rejected
  earlier for a real reason (the JS API can't inject the
  `initialization_script` that sets `navigator.id` — see comment in
  `preview.ts:17`); doesn't fix the tray/`open_options` paths anyway.

---

## Verification plan (on the Windows machine)

1. Reproduce first on unmodified code: tray → "Preview Screensaver" → expect
   white window + frozen app (confirms diagnosis before changing anything).
2. Apply Fix 1, rebuild, repeat: preview should render and the X button close
   it; tray and options window must stay responsive throughout.
3. Exercise `open_options` over IPC (emit `open-options-window` or call
   `invoke("open_options")` from devtools) — must not freeze.
4. After Fix 2: open/close the preview ~5×, watch Task Manager — the number of
   `msedgewebview2.exe` processes must return to baseline each time (no zombie
   accumulation), and `previewActive` in the options UI must toggle exactly
   once per close.
5. Regression pass on macOS and Linux (X11 and Wayland session each): preview
   open/close, options window from tray and via event, full screensaver
   activate/deactivate cycle.
6. Static cross-check from macOS is possible per the scratch-crate/cargo-fetch
   approach used for the multiplatform-fixes plan (full `--target
   x86_64-pc-windows-msvc` check is blocked by `ring`'s C build; the changed
   code here is platform-neutral Rust, so `cargo check` on macOS covers it).

## References

- Tauri rustdoc warning + async-command pattern:
  https://docs.rs/tauri/latest/tauri/webview/struct.WebviewWindowBuilder.html
- Underlying WebView2 re-entrancy issue: https://github.com/tauri-apps/wry/issues/583
- Affected code: `src-tauri/src/lib.rs:452` (`create_preview_window`),
  `src-tauri/src/lib.rs:437` (`open_options`), `src-tauri/src/lib.rs:289`
  (tray `on_menu_event`), `src/app/preview/preview.ts`,
  `src-tauri/capabilities/default.json`
