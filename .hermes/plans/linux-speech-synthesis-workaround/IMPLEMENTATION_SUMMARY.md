# Implementation Summary: Linux Speech Synthesis Workaround

**Implemented:** 2026-07-17 (on macOS — Linux runtime verification pending, see below)

## What was built

Corrected plan first (see PLAN.md "Corrections" section): delivery via
Rust-side `initialization_script` injection into saver + preview windows
instead of a `liminal-api` export; commands registered on all platforms with
the implementation gated to Linux; no capability changes.

### New files

- `src-tauri/src/speech.rs` — three unconditional commands
  (`speak_text`, `cancel_speech`, `speech_synthesis_supported`); Linux
  implementation drives `spd-say` (`-w` so `speak_text` resolves when the
  utterance finishes; `-C` for cancel; text passed after `--`, exec-style, no
  shell); non-Linux stubs return `false`/`Err` per AGENT.md fallback rule.
  Pure mapping functions Web Speech → spd-say ranges
  (`web_rate_to_spd`, `web_pitch_to_spd`, `web_volume_to_spd`) with unit
  tests. `spd-say` calls run via `spawn_blocking`, never on the main thread.
- `src-tauri/src/speech_polyfill.js` — feature-detecting Web Speech shim
  (embedded via `include_str!`): inert when native `speechSynthesis` exists or
  `__TAURI_INTERNALS__` is absent (warns once); otherwise installs
  `SpeechSynthesisUtterance` + `speechSynthesis` with promise-chain utterance
  queueing, truthful `start`/`end`/`error` events, generation-counter cancel.
- `src/app/speech-polyfill.test.ts` — vitest coverage of the shim against fake
  `window` objects: native-present (untouched), no-bridge (warn, no shim),
  speak payload + event order, error path, queue + cancel semantics, inert
  surface (`getVoices`, `pause`, listeners).

### Modified

- `src-tauri/src/lib.rs` — `pub mod speech`; commands registered; polyfill
  injected into the preview window builder.
- `src-tauri/src/screensaver_engine.rs` — polyfill injected into saver window
  builders.
- `README.md` — architecture entry + "Speech Synthesis on Linux" section with
  the `speech-dispatcher` runtime dependency per distro.
- `TODO.md` — item checked (with pending-verification note).

## Verification done (macOS)

- `cargo test` — 36 passed (5 new: 4 mapping + 1 shim/command drift guard).
- `cargo check` — no warnings.
- `bun run test` — 29 passed (6 new polyfill tests).
- `bun run build` — clean.
- Linux `imp` module (std-only) compile-checked in a scratch crate with the
  `cfg` gate stripped.

## Pending on a Linux machine (X11 and Wayland)

Follow PLAN.md "Verification" steps 1–4. Key unknowns only answerable there:

1. Whether remote saver pages receive `__TAURI_INTERNALS__` at all. If the
   shim logs "no IPC bridge", add a `remote` scope to
   `capabilities/saver.json` (coordinate with `.hermes/plans/security/PLAN.md`).
2. `spd-say -w` + `spd-say -C` interplay (cancel must make the waiting
   `speak_text` return promptly).
3. `utterance.lang` values like `en-US` passed to `spd-say -l` — verify the
   installed speech-dispatcher module accepts region-qualified codes; if not,
   truncate to the bare language code in `imp::speak`.
