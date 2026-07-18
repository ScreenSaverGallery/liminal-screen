# Plan: Linux Speech Synthesis Workaround

**Created:** 2026-07-10
**Revised:** 2026-07-17 (corrected delivery mechanism + platform-gating strategy, see "Corrections" below)
**Status:** Implemented (pending Linux runtime verification — see IMPLEMENTATION_SUMMARY.md)

## Problem / Context

WebKitGTK on Linux exposes `navigator.serviceWorker` but **does not expose a working `window.speechSynthesis` API**. Feature detection in the Tauri webview confirms:

| API | Detected |
|---|---|
| `webWorkers` | `true` |
| `serviceWorkers` | `true` |
| `speechSynthesis` | `false` |
| `speechRecognition` / `webkitSpeech` | `false` |

The screensaver content (a remote PWA) uses `speechSynthesis` to speak on-screen text. On Linux this fails silently because the API is entirely absent. macOS and Windows are not affected: their webviews ship with functional speech synthesis backends.

The API gap exists in *every* WebKitGTK webview (main, options, saver, preview alike — it is an engine property), but only the **saver content** actually calls it. The saver URL is loaded by two window kinds: the `saver-display-*` windows and the **preview window** — both must get the fallback.

The project does not currently need speech recognition, so only **text-to-speech (TTS)** needs a fallback.

## Corrections to the original draft (2026-07-17)

1. **Delivery: init-script injection, not a liminal-api export.** The original
   Phase 3 wired `initSpeechSynthesis()` into `src/main.ts` and the remote
   *options* page — the two places that never speak — and left delivery to the
   actual consumer (the remote saver PWA) as Open Question 3. Corrected: the
   polyfill is injected from Rust via `initialization_script` into the
   **saver** and **preview** windows — the same mechanism already used for
   `navigator.id` (`build_init_script`). The remote PWA needs zero changes and
   no `liminal-api` import; every fork gets it automatically, including in the
   preview window.

2. **Commands registered on all platforms, implementation gated.** The original
   plan gated the commands themselves with `#[cfg(target_os = "linux")]`.
   AGENT.md's cross-platform discipline requires every `#[cfg]` branch to have
   a fallback, and unconditional registration keeps `generate_handler!` free of
   cfg gymnastics. Corrected: `speak_text` / `cancel_speech` /
   `speech_synthesis_supported` exist everywhere; on non-Linux they return
   `Err("…only available on Linux")` / `false`. The polyfill feature-detects
   (`'speechSynthesis' in window`) rather than platform-detects, so on
   macOS/Windows it steps aside before ever invoking. Bonus: the whole
   JS → invoke → Rust round-trip is testable from macOS.

3. **No capability changes needed for the commands themselves.** App-defined
   commands are not governed by the plugin/core permission ACL, so the original
   "add permissions to `capabilities/default.json`" step is dropped. The real
   open point is whether **remote-URL windows get the IPC bridge at all**
   (`window.__TAURI_INTERNALS__`) — the standing question tracked in
   `.hermes/plans/security/PLAN.md` (capabilities target remote windows without
   a `remote` scope). The polyfill degrades gracefully (console warning, no
   shim) when the bridge is absent; if Linux verification shows the bridge
   missing in saver windows, the fix is a `remote` scope on `saver.json` —
   to be decided together with the security plan.

4. **File corrections:** `options/main.ts` does not exist (the local options UI
   is `src/main.ts`; the remote options page lives outside this repo). The
   polyfill lives in `src-tauri/src/speech_polyfill.js` (embedded via
   `include_str!`), not in `packages/liminal-api`.

## Goal

Provide a **transparent, feature-gated fallback** so saver content can keep
calling the standard Web Speech API; on Linux the audio is produced by
`spd-say` (speech-dispatcher) invoked through Tauri commands.

## Architecture (as implemented)

1. **Rust backend — `src-tauri/src/speech.rs`:**
   - `speak_text(text, rate?, pitch?, volume?, lang?)` — async command; on
     Linux runs `spd-say -w` via `spawn_blocking` (blocks until the utterance
     finishes, so the JS side can fire a truthful `end` event). Utterance
     parameters are mapped from Web Speech ranges to spd-say's −100..100 via
     pure functions (`web_rate_to_spd`, `web_pitch_to_spd`,
     `web_volume_to_spd`) with unit tests.
   - `cancel_speech()` — runs `spd-say -C` (flushes the speech-dispatcher
     queue, matching `speechSynthesis.cancel()` semantics).
   - `speech_synthesis_supported()` — probes `spd-say --version`.
   - Text is passed as an exec-style argument after `--` — no shell, no
     injection, option-lookalike text is safe.
   - Non-Linux: stubs returning `false` / `Err`, per AGENT.md fallback rule.

2. **JS polyfill — `src-tauri/src/speech_polyfill.js`:**
   - Injected at document-start; exits immediately when the native API exists
     or the IPC bridge (`__TAURI_INTERNALS__`) is missing (warns once).
   - Defines minimal `SpeechSynthesisUtterance` + `window.speechSynthesis`
     with `speak` / `cancel` / `pause` (no-op) / `resume` (no-op) /
     `getVoices` (empty), `speaking` / `pending` getters.
   - Utterances are queued (promise chain) like the native API;
     `start` / `end` / `error` events fire per utterance; `cancel()` bumps a
     generation counter so queued utterances drain without speaking.

3. **Injection points:**
   - `screensaver_engine.rs::create_saver_window` — saver windows.
   - `lib.rs::create_preview_window` — preview window (same saver URL).

4. **Runtime dependency (Linux only):** `speech-dispatcher` (`spd-say`
   binary). Documented in README. Absent binary → commands return errors →
   polyfill fires `error` events; nothing crashes.

## Files Touched

| File | Action |
|---|---|
| `src-tauri/src/speech.rs` | Create — commands, spd-say driver, param mapping + tests |
| `src-tauri/src/speech_polyfill.js` | Create — Web Speech API shim (embedded via `include_str!`) |
| `src-tauri/src/lib.rs` | `pub mod speech`, register 3 commands, inject polyfill into preview window |
| `src-tauri/src/screensaver_engine.rs` | Inject polyfill into saver windows |
| `src/app/speech-polyfill.test.ts` | Create — vitest coverage of the shim (native-present, no-bridge, speak/cancel paths) |
| `README.md` | Document Linux `speech-dispatcher` dependency |
| `TODO.md` | Mark item done |

## Verification

Done on macOS (see IMPLEMENTATION_SUMMARY.md): `cargo test`, `cargo check`,
`bun run test`, `bun run build`; polyfill behavior unit-tested against fake
`window` objects (native API present → untouched; no bridge → warns, no shim;
speak → correct invoke payload + event order; cancel → queue drained).

**Pending on a Linux machine (X11 and Wayland session each):**

1. `spd-say "hello"` — confirm the TTS backend is present.
2. Run the app; in the WebKit inspector of a **saver window** (not options):
   ```js
   'speechSynthesis' in window                       // true (shim installed)
   const u = new SpeechSynthesisUtterance('hello from liminal screen');
   u.onend = () => console.log('end');
   window.speechSynthesis.speak(u);                  // audio plays, 'end' logs
   window.speechSynthesis.cancel();                  // audio stops
   ```
3. **If the shim logs "no IPC bridge"**: remote saver windows lack
   `__TAURI_INTERNALS__` → add a `remote` scope to
   `src-tauri/capabilities/saver.json` per the security plan, and re-test.
4. Repeat step 2 in the preview window.
5. On macOS/Windows: confirm `window.speechSynthesis` is still the native
   implementation (shim exits at the feature check).

## Open Questions

1. Voice selection (`getVoices` returns `[]`): `spd-say -t`/`-y` could map to
   synthetic voice entries later; a richer Rust TTS crate (`tts`) could replace
   spd-say without changing the JS surface. Deferred.
2. `pause()`/`resume()` are no-ops (spd-say has no pause control). Acceptable
   for the screensaver use case.
3. ~~Is the remote saver page already importing liminal-api?~~ Resolved by
   correction 1 — irrelevant, injection needs no page cooperation. The
   remaining half of the question (does the saver window have an IPC bridge)
   moves to Linux verification step 3.

## Related Plans

- `.hermes/plans/multiplatform-fixes/PLAN.md` — prior Linux/Wayland work.
- `.hermes/plans/security/PLAN.md` — remote-URL windows / `remote` scope question.
- `.hermes/plans/preview-window-windows-deadlock/PLAN.md` — preview window creation path (the polyfill injection lands in the same builder).
