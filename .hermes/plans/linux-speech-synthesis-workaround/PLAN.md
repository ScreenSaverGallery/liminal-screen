# Plan: Linux Speech Synthesis Workaround

**Created:** 2026-07-10  
**Status:** Draft

## Problem / Context

WebKitGTK on Linux exposes `navigator.serviceWorker` but **does not expose a working `window.speechSynthesis` API**. Feature detection in the Tauri webview confirms:

| API | Detected |
|---|---|
| `webWorkers` | `true` |
| `serviceWorkers` | `true` |
| `speechSynthesis` | `false` |
| `speechRecognition` / `webkitSpeech` | `false` |

The screensaver content (a remote PWA) uses `speechSynthesis` to speak on-screen text. On Linux this fails silently because the API is entirely absent. macOS and Windows are not affected: their webviews ship with functional speech synthesis backends.

The project does not currently need speech recognition, so only **text-to-speech (TTS)** needs a fallback.

## Goal

Provide a **transparent, platform-gated fallback** so that frontend code can continue calling a Web Speech-compatible API, and on Linux the audio is produced by a native backend invoked through a Tauri command.

## Current State

- `src-tauri/src/lib.rs` registers all Tauri commands and exposes `AppState` / `AppOptions`.
- `src-tauri/Cargo.toml` already has a Linux-specific dependency section with `webkit2gtk`.
- No frontend code currently references `speechSynthesis`; the remote saver page does.
- The project uses a `liminal-api` SDK to bridge remote pages to native features.

## Proposed Changes

### Architecture

1. **Rust backend (Linux only):**
   - Add a new module `src-tauri/src/linux_speech.rs`.
   - Use `std::process::Command` to call `spd-say` (simplest, no extra deps).
   - Gate the module and its commands with `#[cfg(target_os = "linux")]`.
   - Expose:
     - `linux_speak(text: String) -> Result<(), String>` — fire-and-forget TTS.
     - `linux_cancel_speech() -> Result<(), String>` — stop current utterance.
     - `linux_speech_supported() -> bool` — runtime probe for `spd-say`.

2. **Frontend / SDK polyfill:**
   - Add a small `speechSynthesis` polyfill in `packages/liminal-api/src/speech.ts`.
   - When running inside Tauri on Linux, the polyfill forwards `speak()` / `cancel()` to the Rust commands.
   - On macOS/Windows or in a regular browser, the polyfill steps aside and uses the native `window.speechSynthesis`.
   - Publish the polyfill as part of `liminal-api` so remote saver pages can import it.

3. **Tauri command registration and capability:**
   - Register the Linux-only commands in `src-tauri/src/lib.rs` under `#[cfg(target_os = "linux")]`.
   - Add matching permissions to `src-tauri/capabilities/default.json` (Tauri v2 command allow-list).

4. **Build / runtime dependency:**
   - Document that Linux users need `speech-dispatcher` installed (`spd-say` binary).
   - No new Rust crate dependencies unless we later need richer voice control.

## Implementation Phases

### Phase 1: Rust TTS module

1. Create `src-tauri/src/linux_speech.rs`:
   - Probe `which spd-say` once at first use (or on startup).
   - `linux_speak(text)` spawns `spd-say "$text"`, non-blocking.
   - `linux_cancel_speech()` runs `spd-say --cancel` (or `spd-say -C`).
   - `linux_speech_supported()` returns `true` if `spd-say` is on `PATH`.
2. Add `pub mod linux_speech;` in `lib.rs` under `#[cfg(target_os = "linux")]`.
3. Add commands to the `invoke_handler!` with `#[cfg(target_os = "linux")]` guard.
4. Add permissions to `capabilities/default.json`.
5. Run `cargo check` inside `src-tauri`.

### Phase 2: Frontend polyfill in liminal-api

1. Create `packages/liminal-api/src/speech.ts`:
   - Detect Tauri + Linux (e.g. via user agent or `navigator.userAgent`).
   - If polyfill needed, define minimal `SpeechSynthesisUtterance` and `SpeechSynthesis` shim that calls `invoke('linux_speak', ...)` and `invoke('linux_cancel_speech')`.
   - Export an `initSpeechSynthesis()` function that injects the shim only when `window.speechSynthesis` is missing and the runtime is Tauri on Linux.
2. Export the new module from `packages/liminal-api/src/index.ts`.
3. Build the SDK with `bun run build` in `packages/liminal-api/`.
4. Add a unit test in `packages/liminal-api/` that verifies the polyfill is not injected when native speech synthesis exists.

### Phase 3: Wire into remote saver / options pages

1. In the main app (`src/main.ts`) and in the `options/` remote page, call `initSpeechSynthesis()` from `liminal-api` at startup when running under Tauri.
2. Verify with a local test page that `speechSynthesis.speak(new SpeechSynthesisUtterance("test"))` works in the Linux webview.

### Phase 4: Documentation and verification

1. Update `README.md` with the Linux runtime dependency (`speech-dispatcher`).
2. Update `.env.example` if any new env var is introduced (none planned).
3. Add a line to `TODO.md` marking the feature complete.
4. Run `bun run test`, `bun run build`, and `cargo test`.

## Files Touched

| File | Action |
|---|---|
| `src-tauri/src/linux_speech.rs` | Create |
| `src-tauri/src/lib.rs` | Add Linux-gated mod + commands |
| `src-tauri/Cargo.toml` | No change unless we add a crate |
| `src-tauri/capabilities/default.json` | Add command permissions |
| `packages/liminal-api/src/speech.ts` | Create polyfill |
| `packages/liminal-api/src/index.ts` | Export new module |
| `src/main.ts` | Call `initSpeechSynthesis()` on startup |
| `options/main.ts` | Call `initSpeechSynthesis()` on startup |
| `README.md` | Document Linux dependency |
| `TODO.md` | Mark task done after implementation |

## Verification

1. On Linux:
   - Run `spd-say "hello"` to confirm TTS backend is present.
   - Run `bun tauri dev`.
   - Open WebKit inspector and evaluate:
     ```js
     const u = new SpeechSynthesisUtterance('hello from liminal screen');
     window.speechSynthesis.speak(u);
     window.speechSynthesis.cancel();
     ```
   - Audio should play and cancel should stop it.
2. On macOS / Windows:
   - Confirm `window.speechSynthesis` remains native and no Rust command is invoked.
3. Automated:
   - `cargo test` passes.
   - `bun run test` passes.
   - `bun run build` succeeds.

## Open Questions

1. Do we want to support voice / rate / pitch parameters later? `spd-say` has limited options; a richer Rust TTS crate (`tts`) could replace it without changing the JS API.
2. Should the polyfill also queue utterances like the native API, or is fire-and-forget acceptable for the screensaver use case?
3. Is the remote saver page already importing `liminal-api`, or do we need to expose the polyfill through a global script injection as well?

## Related Plans

- `.hermes/plans/multiplatform-fixes/PLAN.md` — prior Linux/Wayland work.
- `.hermes/plans/app-identity-promotion/PLAN.md` — remote page integration patterns.
