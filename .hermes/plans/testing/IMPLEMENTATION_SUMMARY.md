# Testing — Implementation Summary

**Implemented:** 2026-07-08 (Part A — unit tests only; Part B E2E intentionally skipped)

## Rust (`cargo test` in `src-tauri/` — 29 tests)

- `lib.rs`: `AppOptions` defaults/validation boundaries, UUID generation +
  uniqueness, camelCase serialization, `build_init_script` escaping.
- `screensaver_engine.rs`: `compute_next_action` extracted as a pure function
  (as planned) and used by `check_and_manage_screensaver`; 11 tests covering
  all four priorities, plus the already-Locked no-retrigger case.
  `build_saver_url` also extracted (7 tests: primitives, nested/null skipping,
  URL encoding, existing query params, invalid URL).

## TypeScript (`bun run test` = vitest, root config — 23 tests)

- `src/app/reactive.test.ts` + `packages/liminal-api/src/reactive.test.ts`:
  full Signal suite (initial value, set, update, effect, cleanup, derive,
  chained derive, multiple effects).
- `src/app/format.test.ts`: `formatIdle` boundaries. The helper was moved from
  `main.ts` to `src/app/format.ts` — importing `main.ts` in tests would execute
  Tauri bootstrap code, so extraction beats exporting.

Deviations from plan:

- Single root `vitest.config.ts` covers both `src/` and
  `packages/liminal-api/src/` (no separate per-package runner). No jsdom needed
  — nothing under test touches the DOM.
- `security.ts` has no tests: the module is an empty placeholder.
- Test files are excluded from the liminal-api declaration build
  (`tsconfig.json` exclude).

## Part B (E2E) — not implemented

Deliberately skipped per request. The plan's WebDriver notes remain valid.
