# Plan: Frontend Cleanup + Reactive UX Redesign

## Context

The screensaver activation/idle/locking logic was migrated entirely to the Rust backend. The frontend is now only responsible for:
- displaying the options form and status
- relaying user changes to Rust via `invoke("set_options")`
- responding to events from Rust (`screensaver-started`, `screensaver-ended`, `options-updated`)

As a result ~35% of the frontend code is dead, the architecture has redundant layers (Storage, OptionsManager, remote-options), and the reactive foundation laid by `Signal<T>` hasn't been fully exploited for the status display. The patterns found here should also propagate to the fork-developer liminal-api.

---

## Phase 1 — Dead Code Removal

### Delete entire modules (zero callers)

| Path | Why |
|------|-----|
| `src/app/saver/` | Frontend screensaver windows moved to Rust. Never imported. |
| `src/app/remote-options/` | No HTML entry point loads it. Main window IS the fallback options UI. |
| `src/app/storage/` | `Storage` is only called from dead code. `main.ts` talks directly to Rust via `invoke`. |

### Clean surviving files

**`src/app/options/options.ts`**
- Remove `handleSaveOptions()` — nobody emits `save-options`
- Remove `OptionsManager.getSaverUrl()` — dead, URL built by Rust
- Remove `OptionsManager.openOptions()` — main invokes Rust directly
- What remains: `registerServiceWorker()` + `openExternalLink()` — inline both into `main.ts` directly, then delete `options.ts` entirely
- Remove Storage import

**`src/app/power-monitor/power-monitor.ts`**
- Keep only `getSystemIdleTime()`
- Remove: `getSystemIdleState`, `isOnBatteryPower`, `preventDisplaySleep`, `allowDisplaySleep`, `blankScreen`, `lockScreen`, `isDisplaySleepPrevented`, `resetBlocker`
- Remove: `MonitorInfo`, `Position`, `Size` interfaces

**`src/app/preview/preview.ts`**
- Remove `PreviewOptions` interface (never used)

**`src/main.ts`**
- Remove `Storage.init()` call and import
- Remove `OptionsManager.init()` call and import (after inlining helpers)
- Inline `registerServiceWorker()` and `openExternalLink()` directly

### AppOptions type

After removing `storage.ts`, `AppOptions` has no home. Create `src/app/types.ts` — a pure types file with no imports:

```typescript
export interface AppOptions {
  saverUrl: string;
  saverUrlDebug: string;
  optionsUrl: string;
  appName: string;
  appDescription: string;
  startsIn: number;
  displayOffIn: number;
  requirePassIn: number;
  runOnBattery: boolean;
  debug: boolean;
  customOptions: Record<string, string | number | boolean>;
}
```

This mirrors the Rust struct exactly (camelCase via `#[serde(rename_all = "camelCase")]`).

### Result after Phase 1

```
src/
  main.ts             (~200 lines, self-contained)
  vite-env.d.ts
  app/
    types.ts          (new — AppOptions interface only)
    reactive.ts       (unchanged)
    preview/
      preview.ts      (cleaned)
    power-monitor/
      power-monitor.ts (getSystemIdleTime only)
```

---

## Phase 2 — Reactive UX Architecture

### Problem with current approach

`main.ts` uses a single `Signal<AppOptions | null>` for everything. The status display (active dot + idle time) is updated by a 1-second `setInterval` that calls `invoke("get_screensaver_status")`. These are two independent concerns mixed together — options changes shouldn't re-render status, and status ticks shouldn't re-render the form.

### Two focused signals

```typescript
const options = new Signal<AppOptions | null>(null);

interface ScreensaverStatus { active: boolean; idleSeconds: number; }
const status = new Signal<ScreensaverStatus>({ active: false, idleSeconds: 0 });
```

Each signal gets its own `effect()` — form reacts to `options`, status indicator reacts to `status`. The interval only calls `status.update(...)`, never touches `options`.

### Add `derive()` to Signal

A computed/derived signal eliminates the need for separate `updateStatusDisplay()` / `updateIdleDisplay()` helpers:

```typescript
derive<U>(fn: (value: T) => U): Signal<U> {
  const child = new Signal<U>(fn(this._value));
  this.effect((v) => child.set(fn(v)));
  return child;
}
```

Usage in main.ts:
```typescript
const isActive = status.derive(s => s.active);
const idleSeconds = status.derive(s => s.idleSeconds);

isActive.effect((active) => {
  statusDot.classList.toggle('active', active);
  statusDot.classList.toggle('inactive', !active);
  statusText.textContent = active ? 'Active' : 'Inactive';
});

idleSeconds.effect((secs) => {
  idleTimeElement.textContent = formatIdle(secs);
});
```

### Event-driven status (no polling for active state)

`screensaver-started` and `screensaver-ended` already arrive as Tauri events — don't poll for `is_active`. Only poll for idle time (no event available from Rust for that yet).

```typescript
listen('screensaver-started', () => status.update(s => ({ ...s, active: true })));
listen('screensaver-ended',   () => status.update(s => ({ ...s, active: false })));

setInterval(async () => {
  const secs = await PowerMonitor.getSystemIdleTime();
  status.update(s => ({ ...s, idleSeconds: secs }));
}, 1000);
```

The 1-second interval now only touches `status`, never `options`. The `get_screensaver_status` invoke poll is removed entirely (replaced by events).

### Final reactive wiring in main.ts

```
DOMContentLoaded
  └── cacheUIElements()
  └── setupUIButtonHandlers()
  └── options.effect(opts => { /* populate form fields */ })     ← runs on load + reset + IPC update
  └── isActive.effect(active => { /* status dot */ })
  └── idleSeconds.effect(secs => { /* idle display */ })
  └── init()
        ├── invoke("get_options") → options.set(...)
        ├── listen("options-updated", ...) → options.set(event.payload)
        ├── listen("screensaver-started", ...) → status.update active=true
        ├── listen("screensaver-ended",  ...) → status.update active=false
        └── setInterval → status.update idleSeconds
```

No `loadOptionsIntoForm()`, no `updateStatusDisplay()`, no `updateIdleTimeDisplay()` — all replaced by effects.

---

## Phase 3 — Propagate patterns to liminal-api

### Export Signal from liminal-api

Add `Signal` and `derive()` to `packages/liminal-api/src/index.ts` exports. Fork developers loading the CDN bundle get reactivity for free:

```typescript
// in liminal-api/src/index.ts
export { Signal } from './reactive';
```

Copy `src/app/reactive.ts` → `packages/liminal-api/src/reactive.ts` (it has zero dependencies).

### createOptionsStore() helper

A factory that wraps `Signal<AppOptions | null>` + auto-sync from the API, so fork devs don't hand-wire the pattern:

```typescript
// packages/liminal-api/src/store.ts
export function createOptionsStore(api: LiminalAPIType) {
  const options = new Signal<AppOptions | null>(null);

  api.getOptions().then(opts => options.set(opts));
  api.startAutoSync(opts => options.set(opts));

  return {
    signal: options,
    save: (patch: Partial<AppOptions>) => {
      const current = options.get();
      if (!current) return Promise.resolve();
      return api.setOptions({ ...current, ...patch } as any).then(() =>
        api.getOptions().then(opts => options.set(opts))
      );
    },
    reset: () => api.resetOptions().then(opts => options.set(opts)),
  };
}
```

### Update remote-options example

Replace the imperative `populateForm()` callback in `packages/liminal-api/examples/remote-options/main.ts` with:

```typescript
const store = createOptionsStore(api);

store.signal.effect((opts) => {
  if (!opts) return;
  startsIn.value = String(opts.startsIn);
  displayOff.value = String(opts.displayOffIn);
  // ...
});

$('reset-btn')?.addEventListener('click', () => store.reset());
$('save-btn')?.addEventListener('click', () => store.save(collectForm()));
```

The `current` variable, `dirty` flag, and manual `populateForm()` calls all disappear.

---

## Files touched

| File | Action |
|------|--------|
| `src/app/saver/saver.ts` | DELETE |
| `src/app/remote-options/remote-options.ts` | DELETE |
| `src/app/storage/storage.ts` | DELETE |
| `src/app/options/options.ts` | DELETE (helpers inlined into main.ts) |
| `src/app/power-monitor/power-monitor.ts` | CLEAN — keep `getSystemIdleTime()` only |
| `src/app/preview/preview.ts` | CLEAN — remove `PreviewOptions` |
| `src/app/reactive.ts` | ENHANCE — add `derive()` method |
| `src/app/types.ts` | CREATE — AppOptions interface |
| `src/main.ts` | REFACTOR — two signals, derive, inlined helpers, no Storage/OptionsManager |
| `packages/liminal-api/src/reactive.ts` | CREATE — copy of app/reactive.ts |
| `packages/liminal-api/src/store.ts` | CREATE — createOptionsStore() |
| `packages/liminal-api/src/index.ts` | UPDATE — export Signal, createOptionsStore |
| `packages/liminal-api/examples/remote-options/main.ts` | UPDATE — use createOptionsStore |

---

## Verification

1. `bun run typecheck` in root — zero TS errors
2. `cargo tauri dev` — options window opens, form populates, status dot updates, save/reset/preview all work
3. Toggle debug mode — saver URL display switches immediately (effect fires)
4. Click Reset to defaults — all form fields update instantly (single effect)
5. `bun run build` in `packages/liminal-api/examples/remote-options/` — compiles cleanly
6. Open example `index.html` in browser — demo mode works with mock data
