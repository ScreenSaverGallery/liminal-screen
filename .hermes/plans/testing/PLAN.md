# Testing — Unit & E2E

**Created:** 2026-04-23  
**Status:** Implemented (Part A unit tests; Part B E2E pending) — see IMPLEMENTATION_SUMMARY.md

---

## Problem

The project has zero tests. All correctness guarantees today come from manual testing. The state machine logic, options validation, reactive signal system, and window lifecycle all have meaningful logic that can and should be verified automatically.

---

## Current State

| Layer | Status |
|-------|--------|
| Rust `#[test]` modules | None |
| TypeScript unit tests | None |
| E2E / integration tests | None |
| Test dependencies | None in `Cargo.toml` dev-deps, none in `package.json` |
| CI test automation | None |

---

## Part A — Unit Tests

### A1. Rust — `src-tauri/src/`

**Setup:** No new crates needed. Rust's built-in `#[cfg(test)]` + `cargo test` is sufficient for all pure logic. Add `tokio = { version = "1", features = ["full"] }` to `[dev-dependencies]` in `Cargo.toml` for async tests only if needed.

Run with: `cargo test`

---

#### Target 1: `AppOptions` defaults and validation — `lib.rs`

Add `#[cfg(test)] mod tests` block at the bottom of `lib.rs`.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_options_have_valid_timing() {
        let opts = AppOptions::default();
        assert!(opts.starts_in >= 0.1);
        assert!(opts.display_off_in >= 0.5);
        assert!(opts.require_pass_in >= 0.0);
    }

    #[test]
    fn validate_options_rejects_starts_in_too_low() {
        let mut opts = AppOptions::default();
        opts.starts_in = 0.05;
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn validate_options_rejects_display_off_too_low() {
        let mut opts = AppOptions::default();
        opts.display_off_in = 0.4;
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn validate_options_accepts_boundary_values() {
        let mut opts = AppOptions::default();
        opts.starts_in = 0.1;
        opts.display_off_in = 0.5;
        opts.require_pass_in = 0.0;
        assert!(validate_options(&opts).is_ok());
    }

    #[test]
    fn validate_options_rejects_values_over_max() {
        let mut opts = AppOptions::default();
        opts.starts_in = 1441.0;
        assert!(validate_options(&opts).is_err());
    }

    #[test]
    fn instance_id_is_valid_uuid() {
        let opts = AppOptions::default();
        assert!(uuid::Uuid::parse_str(&opts.instance_id).is_ok());
    }

    #[test]
    fn two_defaults_have_different_instance_ids() {
        let a = AppOptions::default();
        let b = AppOptions::default();
        assert_ne!(a.instance_id, b.instance_id);
    }
}
```

---

#### Target 2: Screensaver state machine timing logic — `screensaver_engine.rs`

The `check_and_manage_screensaver` method reads options and computes thresholds. The priority logic (lock > display-off > activate > deactivate) is the core invariant to test. Extract the threshold comparison into a pure helper so it can be tested without Tauri:

```rust
// Add to screensaver_engine.rs
pub fn compute_next_action(
    idle_secs: u64,
    starts_in_secs: u64,
    display_off_secs: u64,
    require_pass_secs: u64,
    current_state: ScreensaverState,
) -> Option<ScreensaverState> {
    if require_pass_secs > 0 && idle_secs >= require_pass_secs {
        return Some(ScreensaverState::Locked);
    }
    if idle_secs >= display_off_secs && current_state != ScreensaverState::DisplayOff {
        return Some(ScreensaverState::DisplayOff);
    }
    if idle_secs >= starts_in_secs
        && current_state == ScreensaverState::Idle
        && starts_in_secs < display_off_secs
    {
        return Some(ScreensaverState::ScreensaverActive);
    }
    if idle_secs < starts_in_secs && current_state != ScreensaverState::Idle {
        return Some(ScreensaverState::Idle);
    }
    None // no state change needed
}
```

Tests in `#[cfg(test)]` block in `screensaver_engine.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_takes_priority_over_display_off() {
        let next = compute_next_action(120, 30, 60, 90, ScreensaverState::ScreensaverActive);
        assert_eq!(next, Some(ScreensaverState::Locked));
    }

    #[test]
    fn display_off_takes_priority_over_screensaver() {
        let next = compute_next_action(70, 30, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, Some(ScreensaverState::DisplayOff));
    }

    #[test]
    fn screensaver_activates_when_idle_enough() {
        let next = compute_next_action(40, 30, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, Some(ScreensaverState::ScreensaverActive));
    }

    #[test]
    fn deactivates_on_user_activity() {
        let next = compute_next_action(5, 30, 60, 0, ScreensaverState::ScreensaverActive);
        assert_eq!(next, Some(ScreensaverState::Idle));
    }

    #[test]
    fn no_change_when_idle_but_below_threshold() {
        let next = compute_next_action(20, 30, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, None);
    }

    #[test]
    fn no_change_when_already_in_correct_state() {
        let next = compute_next_action(70, 30, 60, 0, ScreensaverState::DisplayOff);
        assert_eq!(next, None);
    }

    #[test]
    fn screensaver_does_not_activate_when_starts_in_equals_display_off() {
        // starts_in < display_off_in is required for screensaver activation
        let next = compute_next_action(60, 60, 60, 0, ScreensaverState::Idle);
        assert_eq!(next, Some(ScreensaverState::DisplayOff));
    }

    #[test]
    fn lock_disabled_when_require_pass_is_zero() {
        let next = compute_next_action(300, 30, 60, 0, ScreensaverState::ScreensaverActive);
        // should not lock
        assert_ne!(next, Some(ScreensaverState::Locked));
    }
}
```

---

#### Target 3: `get_saver_url` query param building — `screensaver_engine.rs`

The URL-building logic (custom options → query params) can be tested by calling `get_saver_url` logic in isolation. Alternatively, extract the URL building into a standalone pure function:

```rust
pub fn build_saver_url(base: &str, custom: &serde_json::Value, debug: bool,
                        debug_url: &str) -> Result<String, String>
```

Test: custom options with string/number/bool values appear as query params; nested objects/null are skipped; empty custom options returns bare base URL.

---

### A2. TypeScript — `src/`

**Setup:** Add Vitest (fastest, native ESM, works with Vite projects):

```bash
bun add -D vitest
```

Add to `package.json` scripts:
```json
"test": "vitest run",
"test:watch": "vitest"
```

Add `vitest.config.ts`:
```ts
import { defineConfig } from 'vitest/config';
export default defineConfig({ test: { environment: 'jsdom' } });
```

Run with: `bun test`

---

#### Target 1: `Signal` class — `src/app/reactive.ts`

Create `src/app/reactive.test.ts`:

```typescript
import { describe, it, expect, vi } from 'vitest';
import { Signal } from './reactive';

describe('Signal', () => {
  it('holds initial value', () => {
    const s = new Signal(42);
    expect(s.get()).toBe(42);
  });

  it('set updates value', () => {
    const s = new Signal(0);
    s.set(5);
    expect(s.get()).toBe(5);
  });

  it('update applies transform', () => {
    const s = new Signal(3);
    s.update(n => n * 2);
    expect(s.get()).toBe(6);
  });

  it('effect fires immediately with current value', () => {
    const s = new Signal('hello');
    const fn = vi.fn();
    s.effect(fn);
    expect(fn).toHaveBeenCalledWith('hello');
  });

  it('effect fires on each set', () => {
    const s = new Signal(0);
    const values: number[] = [];
    s.effect(v => values.push(v));
    s.set(1);
    s.set(2);
    expect(values).toEqual([0, 1, 2]);
  });

  it('effect cleanup removes listener', () => {
    const s = new Signal(0);
    const fn = vi.fn();
    const cleanup = s.effect(fn);
    fn.mockClear();
    cleanup();
    s.set(99);
    expect(fn).not.toHaveBeenCalled();
  });

  it('derive produces computed child', () => {
    const s = new Signal(2);
    const doubled = s.derive(n => n * 2);
    expect(doubled.get()).toBe(4);
    s.set(5);
    expect(doubled.get()).toBe(10);
  });

  it('multiple effects all fire', () => {
    const s = new Signal(0);
    const a = vi.fn(), b = vi.fn();
    s.effect(a); s.effect(b);
    a.mockClear(); b.mockClear();
    s.set(1);
    expect(a).toHaveBeenCalledWith(1);
    expect(b).toHaveBeenCalledWith(1);
  });

  it('derive chain updates correctly', () => {
    const s = new Signal(1);
    const x2 = s.derive(n => n * 2);
    const x4 = x2.derive(n => n * 2);
    s.set(3);
    expect(x4.get()).toBe(12);
  });
});
```

---

#### Target 2: `formatIdle` helper — `src/main.ts`

Export `formatIdle` (currently unexported) and test boundary cases:
- `< 60s` → `"Idle: 45s"`
- `< 3600s` → `"Idle: 2m 30s"`
- `>= 3600s` → `"Idle: 1h 5m"`

---

### A3. liminal-api — `packages/liminal-api/src/`

Run with: `bun test` from `packages/liminal-api/`

Test `src/reactive.ts` (same Signal class copy) with the same test suite as above. Test `src/security.ts` — whatever validation/sanitization it applies to invoke payloads.

---

## Part B — E2E Tests

E2E tests verify that the full app boots, windows open, and the screensaver lifecycle works end-to-end. Tauri v2 supports WebDriver via the `tauri-driver` tool.

### Setup

```bash
# Install tauri-driver (Tauri's WD bridge)
cargo install tauri-driver

# Install WebdriverIO (recommended for Tauri v2 e2e)
bun add -D @wdio/cli @wdio/local-runner @wdio/mocha-framework @wdio/spec-reporter webdriverio
```

Create `wdio.conf.ts` targeting the built app binary. Reference: https://v2.tauri.app/develop/tests/webdriver/

---

### E2E Scenarios

| # | Scenario | Window | Assertion |
|---|----------|--------|-----------|
| 1 | App launches, tray icon appears | — | Process running, no crash |
| 2 | Tray → Options opens the options window | options | Window visible, has `#app-name` |
| 3 | Options form shows default timing values | options | starts-in, display-off inputs have expected values |
| 4 | Save → values persist across restart | options | After app restart, saved values match |
| 5 | Preview button opens a preview window | preview-* | New window with correct label appears |
| 6 | Preview window close sets status back to inactive | options | Status dot is inactive after close |
| 7 | Reset to Defaults clears options.json | options | After reset, values match .env defaults |
| 8 | Screensaver activates after idle threshold | saver-display-* | Window with `saver-display-` label appears |
| 9 | Screensaver deactivates on mouse move | — | `saver-display-*` windows close |
| 10 | `navigator.id` is set in saver window | saver-display-* | `window.navigator.id` is a UUID string |
| 11 | `navigator.userAgent` contains app suffix | saver-display-* | UA contains `LiminalScreen/` |

---

### Notes on Tauri E2E

- Tauri WebDriver only works against a **built** (not dev) binary — run `bun run tauri build` before running E2E
- Screensaver activation tests (scenarios 8–9) require manipulating the `starts_in` option to a short value (e.g., 0.1 min) and using `PowerMonitor.getSystemIdleTime()` assertions
- E2E tests are inherently slower and more brittle than unit tests — run in CI only, not during development

---

## Files to Create / Modify

| File | Action |
|------|--------|
| `src-tauri/Cargo.toml` | Add `[dev-dependencies]` section (tokio if needed) |
| `src-tauri/src/lib.rs` | Add `#[cfg(test)] mod tests` block |
| `src-tauri/src/screensaver_engine.rs` | Extract `compute_next_action` pure fn; add `#[cfg(test)] mod tests` |
| `src/app/reactive.test.ts` | New — Vitest unit tests for Signal |
| `src/main.ts` | Export `formatIdle` for testing |
| `package.json` | Add `vitest` dev dep; add `test` script |
| `vitest.config.ts` | New — Vitest config |
| `wdio.conf.ts` | New — WebdriverIO config for E2E |
| `tests/e2e/` | New directory — e2e spec files |

---

## Verification

- [ ] `cargo test` passes with all Rust unit tests green
- [ ] `bun test` passes with all TS unit tests green
- [ ] State machine tests cover all 4 priority transitions
- [ ] Signal tests cover: initial value, set, update, effect, effect cleanup, derive, chained derive
- [ ] `AppOptions` tests cover: UUID generation, uniqueness, validation boundaries
- [ ] E2E: app launches without crash
- [ ] E2E: options window opens from tray
- [ ] E2E: `navigator.id` is a valid UUID in saver window
