# Implementation Summary: Persistent Storage Sync

**Created:** 2026-04-17
**Status:** ✅ Critical Fixes Complete

---

## Changes Made

### Backend (`src-tauri/src/lib.rs`)

#### 1. Fixed `set_options()` - URL Preservation
**Problem:** Frontend was sending empty strings for URLs, risking data corruption.

**Solution:** Backend now preserves URLs from current state before applying user changes.

```rust
// Preserve URLs from current state - these are fork-controlled, not user-configurable
let current = state.options.lock().unwrap();
let options_with_preserved_urls = AppOptions {
    saver_url: current.saver_url.clone(),
    saver_url_debug: current.saver_url_debug.clone(),
    options_url: current.options_url.clone(),
    ..options.clone()
};
drop(current);

// Update in-memory state with preserved URLs
let mut current = state.options.lock().unwrap();
*current = options_with_preserved_urls.clone();
drop(current);
```

**Defense-in-depth:** Even if frontend sends wrong data, backend protects fork-critical fields.

---

#### 2. Fixed `factory_reset_options()` - Actually Clears Store
**Before:** Just returned defaults, never touched disk
**After:** Clears `options.json` and resets in-memory state

```rust
let store = app.store("options.json")?;
store.clear();  // Delete all persisted data
store.save()?;  // Flush to disk

// Reset in-memory state
let default_options = AppOptions::default();
let mut current = state.options.lock().unwrap();
*current = default_options.clone();
```

---

#### 3. Fixed `set_options()` - Persists to Disk
**Before:** Only updated in-memory state (changes lost on restart)
**After:** Updates memory + writes to `options.json`

```rust
store.set("startsIn", options.starts_in);
store.set("displayOffIn", options.display_off_in);
store.set("requirePassIn", options.require_pass_in);
store.set("runOnBattery", options.run_on_battery);
store.set("debug", options.debug);
store.save()?;  // Flush to disk
```

---

#### 4. Fixed `setup_app()` - Graceful Error Handling
**Before:** `load_persisted_options(app)?` would crash if store failed
**After:** Logs warning and falls back to defaults

```rust
let options = load_persisted_options(app).unwrap_or_else(|e| {
    eprintln!("[store] Warning: Could not load persisted options, using defaults: {}", e);
    AppOptions::default()
});
```

---

#### 5. Fixed Import Statement
**Before:** `StoreExt` imported inside function
**After:** Module-level import

```rust
use tauri_plugin_store::StoreExt;
```

---

### Frontend (`src/app/remote-options/remote-options.ts`)

#### 1. Fixed `saveOptions()` - Fetch-Modify-Save Pattern
**Problem:** Frontend sent empty strings for URLs and hardcoded `require_pass_in`

**Solution:** Fetch current options first, modify only user-editable fields

```typescript
async function saveOptions(): Promise<void> {
  try {
    // Fetch current options first to preserve URLs and other backend-controlled fields
    const current = isTauri ? await invoke<any>("get_options") : null;
    
    const newOptions = {
      starts_in: startsInInput ? parseFloat(startsInInput.value) : 0.2,
      display_off_in: displayOffInput ? parseFloat(displayOffInput.value) : 1.0,
      run_on_battery: runOnBatteryInput ? runOnBatteryInput.checked : false,
      debug: debugInput ? debugInput.checked : false,
      // Preserve backend-controlled fields from current state
      saver_url: current?.saver_url || "",
      saver_url_debug: current?.saver_url_debug || "",
      options_url: current?.options_url || "",
      require_pass_in: current?.require_pass_in || 1.0,
    };
    
    await invoke("set_options", { options: newOptions });
    // ...
  }
}
```

---

#### 2. Fixed `resetOptions()` - Removed Redundant Save
**Before:** Called `factory_reset_options()` then `saveOptions()` (double write)
**After:** Just calls `factory_reset_options()` which already persists

```typescript
// Update form with default options
loadOptionsIntoForm(defaultOptions);

// Note: factory_reset_options() already persists to disk, no need to call saveOptions()

alert("Options reset to defaults");
```

---

## Files Modified

| File | Changes |
|------|---------|
| `src-tauri/src/lib.rs` | URL preservation in `set_options()`, store persistence, error handling |
| `src/app/remote-options/remote-options.ts` | Fetch-modify-save pattern, removed redundant save |

---

## Architecture (Final)

```
┌─────────────────────────────────────────────────────────────┐
│                    OPTIONS SYNC FLOW                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  STARTUP:                                                   │
│  1. load_persisted_options()                                │
│     ├─ Try options.json                                     │
│     ├─ If missing → .env defaults                           │
│     └─ Store in AppState (memory)                           │
│                                                             │
│  USER OPENS OPTIONS:                                        │
│  2. get_options() → Frontend                                │
│     └─ Returns AppState.options                             │
│                                                             │
│  USER SAVES:                                                │
│  3. Frontend: get_options() → modify → set_options()        │
│  4. Backend: set_options()                                  │
│     ├─ Preserve URLs from current state                     │
│     ├─ Update AppState.options (memory)                     │
│     ├─ Write to options.json (disk)                         │
│     └─ Engine reads updated values immediately              │
│                                                             │
│  USER RESETS:                                               │
│  5. factory_reset_options()                                 │
│     ├─ Clear options.json                                   │
│     ├─ Reset AppState to .env defaults                      │
│     └─ Frontend re-renders with defaults                    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Testing Checklist

### ✅ Backend
- [x] `set_options()` preserves URLs
- [x] `set_options()` persists to disk
- [x] `factory_reset_options()` clears store
- [x] `setup_app()` handles store failures gracefully
- [ ] Compile with `cargo check` (Rust not installed in this env)

### ✅ Frontend
- [x] `saveOptions()` fetches current options first
- [x] `saveOptions()` preserves backend-controlled fields
- [x] `resetOptions()` doesn't double-save
- [ ] Manual test: change value → save → restart → verify persists
- [ ] Manual test: factory reset → verify .env defaults restored

---

## Remaining Gaps (Low Priority)

### P2 - Nice to Have

| Issue | Impact | Effort |
|-------|--------|--------|
| No loading state during save | UX | 15min |
| Alert instead of toast | UX | 30min |
| `require_pass_in` not in UI | Consistency | 30min |
| No validation error messages | UX | 30min |

**Decision:** Defer to v1.1 - critical data integrity issues are fixed.

---

## Next Steps for User

1. **Compile and test:**
   ```bash
   cd src-tauri
   cargo check
   cargo build
   ```

2. **Test flow:**
   - Start app, open options
   - Change `starts_in` to 5.0
   - Save
   - Restart app
   - Verify value persists

3. **Test factory reset:**
   - Change multiple settings
   - Click "Reset to Defaults"
   - Verify settings match `.env`
   - Restart app
   - Verify defaults persist

---

## Plan Directory

All planning documents stored in:
```
.hermes/plans/persistent-storage-sync/
├── PLAN.md              # Architecture and design
├── GAP-ANALYSIS.md      # Issues found and decisions
└── IMPLEMENTATION.md    # This file - what was fixed
```
