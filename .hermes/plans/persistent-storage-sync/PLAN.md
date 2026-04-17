# Persistent Storage Sync Plan
## Frontend ↔ Backend Options Synchronization

**Created:** 2026-04-17
**Status:** Planning

---

## Problem Statement

Currently the backend loads persisted options on startup, but there's no clear strategy for:
1. How frontend gets initial options from backend
2. How frontend changes get persisted to backend
3. Whether changes should reflect immediately in backend state
4. How to handle factory reset from frontend
5. What happens when multiple windows access options

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                         STARTUP FLOW                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. App starts → load_persisted_options()                      │
│     ├─ Try load options.json from store                        │
│     ├─ If missing/invalid → use .env defaults                  │
│     └─ Store in AppState (in-memory mutex)                     │
│                                                                 │
│  2. Frontend initializes → calls get_options() command         │
│     └─ Returns current AppState.options                        │
│                                                                 │
│  3. Frontend renders options UI with loaded values             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                       USER CHANGE FLOW                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. User modifies value in UI                                  │
│                                                                 │
│  2. Frontend calls set_options(newOptions) command             │
│     ├─ Backend updates AppState.options (memory)               │
│     ├─ Backend writes to options.json (disk)                   │
│     └─ Returns Ok(()) or Err(message)                          │
│                                                                 │
│  3. Frontend receives response                                 │
│     ├─ Success: Show confirmation, update local state          │
│     └─ Error: Show error toast, revert UI                      │
│                                                                 │
│  4. Backend screensaver engine reads from AppState             │
│     └─ Changes take effect immediately (no restart needed)     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                       FACTORY RESET FLOW                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. User clicks "Reset to Defaults" in UI                      │
│                                                                 │
│  2. Frontend calls factory_reset_options() command             │
│     ├─ Backend clears options.json store                       │
│     ├─ Backend saves empty store to disk                       │
│     ├─ Backend resets AppState.options to .env defaults        │
│     └─ Returns default options                                 │
│                                                                 │
│  3. Frontend receives defaults, re-renders UI                  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Component Responsibilities

### Backend (`src-tauri/src/lib.rs`)

| Function | Responsibility |
|----------|---------------|
| `load_persisted_options()` | Startup only: load from store or .env defaults |
| `get_options()` | Return current in-memory state (fast, no disk I/O) |
| `set_options()` | Update memory + persist to disk |
| `factory_reset_options()` | Clear store + reset memory to defaults |

### Frontend (`src/app/options/`)

| Component | Responsibility |
|-----------|---------------|
| `options.ts` | Fetch initial options, handle form submissions |
| Options UI | Render form, show loading/error states |
| Storage module | Local caching (optional, not source of truth) |

---

## Data Flow Specifications

### 1. Initial Load (App Start → Frontend Ready)

```typescript
// Frontend: src/app/options/options.ts
async function loadOptions(): Promise<AppOptions> {
  try {
    const options = await invoke<AppOptions>('get_options');
    renderOptionsForm(options);
    return options;
  } catch (error) {
    showError('Failed to load options: ' + error);
    return getDefaultOptions(); // Hardcoded fallback
  }
}
```

**Timing:** Called when options window opens / component mounts

---

### 2. User Saves Changes

```typescript
// Frontend: Form submit handler
async function saveOptions(newOptions: AppOptions): Promise<void> {
  try {
    await invoke('set_options', { options: newOptions });
    showSuccess('Settings saved');
    // Optionally: update local state without re-fetch
  } catch (error) {
    showError('Failed to save: ' + error);
    // Revert UI to previous values
  }
}
```

**Timing:** On form submit (not on every keystroke)

**Backend behavior:**
- Updates `AppState.options` mutex immediately
- Persists to `options.json` synchronously
- Screensaver engine reads from `AppState` on next check cycle

---

### 3. Factory Reset

```typescript
// Frontend: Reset button handler
async function factoryReset(): Promise<void> {
  const confirmed = await confirmDialog(
    'Reset all settings to defaults? This cannot be undone.'
  );
  if (!confirmed) return;
  
  try {
    const defaults = await invoke<AppOptions>('factory_reset_options');
    renderOptionsForm(defaults);
    showSuccess('Settings reset to defaults');
  } catch (error) {
    showError('Failed to reset: ' + error);
  }
}
```

---

## Immediate Reflection Strategy

### Question: Should backend reflect changes immediately?

**Answer: YES** - Here's why:

1. **Screensaver engine reads from AppState** which is updated by `set_options()`
2. **No restart needed** - engine checks options on each idle time evaluation
3. **User expectation** - changes should take effect when they click "Save"

### Implementation Detail

```rust
// Backend: set_options command
fn set_options<R: Runtime>(
    app: AppHandle<R>, 
    state: tauri::State<AppState>, 
    options: AppOptions
) -> Result<(), String> {
    // 1. Update in-memory state (immediate effect)
    let mut current = state.options.lock().unwrap();
    *current = options.clone();
    drop(current); // Release lock so engine can read
    
    // 2. Persist to disk (for next restart)
    let store = app.store("options.json")?;
    store.set("startsIn", options.starts_in);
    // ... other fields
    store.save()?;
    
    Ok(())
}
```

```rust
// Engine reads from AppState on each check cycle
fn check_idle_time(&self, app: &AppHandle) {
    let state = app.state::<AppState>();
    let options = state.options.lock().unwrap();
    let starts_in = options.starts_in;
    // ... use current values
}
```

---

## Edge Cases & Error Handling

### 1. Store Unavailable (Disk Full / Permission Error)

```rust
// Backend: Graceful degradation
fn set_options(...) -> Result<(), String> {
    // Update memory (changes work until restart)
    let mut current = state.options.lock().unwrap();
    *current = options.clone();
    
    // Try to persist, but don't fail if disk is full
    match app.store("options.json") {
        Ok(store) => {
            store.set(...);
            if let Err(e) = store.save() {
                eprintln!("[store] Warning: Could not persist: {}", e);
                // Return success anyway - memory updated
            }
        }
        Err(e) => {
            eprintln!("[store] Warning: Could not open store: {}", e);
            // Return success anyway - memory updated
        }
    }
    
    Ok(())
}
```

**Frontend:** Show warning toast "Settings saved for this session, but may not persist"

---

### 2. Concurrent Modifications

**Scenario:** User opens options in two windows, saves in both

**Resolution:** Last write wins (standard mutex behavior)

**Mitigation:** 
- Options window should be single-instance (check if already open)
- Or: Add timestamp field and warn if stale

---

### 3. Invalid Values from Frontend

**Current approach:** TypeScript types + serde validation

**Additional validation:**
```rust
fn validate_options(options: &AppOptions) -> Result<(), String> {
    if options.starts_in < 0.0 || options.starts_in > 1440.0 {
        return Err("starts_in must be between 0 and 1440 minutes".into());
    }
    // ... other validations
    Ok(())
}
```

---

## File Structure

```
src/
├── app/
│   └── options/
│       ├── options.ts      # Options window management, invoke calls
│       └── options.html    # Form UI
src-tauri/src/
└── lib.rs                  # get_options, set_options, factory_reset_options
```

---

## Testing Checklist

- [ ] App starts with .env defaults (fresh install)
- [ ] App loads saved options from options.json (returning user)
- [ ] Changing value in UI → save → restart → value persists
- [ ] Factory reset → options.json deleted → .env defaults restored
- [ ] Engine uses updated values without restart
- [ ] Error handling when store is unavailable
- [ ] Multiple saves in quick succession don't corrupt data

---

## Open Questions

1. **Should we debounce saves?** 
   - Pro: Reduces disk writes
   - Con: User expects immediate save
   - **Decision:** Save on form submit, not on change

2. **Should frontend cache options locally?**
   - Pro: Faster UI updates
   - Con: Risk of drift from backend
   - **Decision:** No cache, always fetch from backend on load

3. **Should we emit events when options change?**
   - Pro: Other windows can update in real-time
   - Con: Added complexity
   - **Decision:** Not needed for v1 (single options window)

---

## Next Steps

1. Verify current frontend implementation matches this plan
2. Add validation to `set_options()` backend command
3. Implement error handling for store failures
4. Test full flow: load → modify → save → restart → verify
5. Document in README for fork maintainers
