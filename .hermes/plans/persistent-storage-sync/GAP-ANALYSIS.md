# Gap Analysis: Frontend ↔ Backend Options Sync

**Created:** 2026-04-17
**Status:** Analysis Complete

---

## Current Implementation Status

### ✅ What's Working

| Component | Status | Notes |
|-----------|--------|-------|
| Backend `get_options()` | ✅ Implemented | Returns in-memory state |
| Backend `set_options()` | ✅ Fixed | Now persists to store + updates memory |
| Backend `factory_reset_options()` | ✅ Fixed | Now clears store + resets memory |
| Frontend `loadOptions()` | ✅ Implemented | Calls `get_options()` on init |
| Frontend `saveOptions()` | ✅ Implemented | Calls `set_options()` on save |
| Frontend `resetOptions()` | ✅ Implemented | Calls `factory_reset_options()` |
| Form validation | ✅ Basic | Min values for starts_in, display_off_in |

---

## ⚠️ Issues Found

### 1. Frontend sends incomplete options to `set_options()`

**Location:** `src/app/remote-options/remote-options.ts:235-245`

```typescript
const newOptions = {
  starts_in: startsInInput ? parseFloat(startsInInput.value) : 0.2,
  display_off_in: displayOffInput ? parseFloat(displayOffInput.value) : 1.0,
  run_on_battery: runOnBatteryInput ? runOnBatteryInput.checked : false,
  debug: debugInput ? debugInput.checked : false,
  // These come from environment variables, so we don't change them here
  saver_url: "", // Will be set by main app  ← PROBLEM
  saver_url_debug: "", // Will be set by main app  ← PROBLEM
  options_url: "", // Will be set by main app  ← PROBLEM
  require_pass_in: 1.0, // Default value  ← PROBLEM
};
```

**Problem:** Frontend sends empty strings for URLs and hardcoded `require_pass_in`. Backend overwrites current values with these empties.

**Impact:**
- URLs might get corrupted if backend doesn't preserve them
- `require_pass_in` always resets to 1.0 on every save

**Fix Required:** Frontend should:
1. Fetch current options first
2. Only modify user-editable fields
3. Send complete object with existing values preserved

---

### 2. No validation for `require_pass_in`

**Location:** Backend `AppOptions` struct + Frontend form

**Problem:** `require_pass_in` is not exposed in the options UI, but it's part of `AppOptions`. Frontend hardcodes it to 1.0.

**Decision Needed:**
- **Option A:** Add `require_pass_in` to UI (consistent with other timing fields)
- **Option B:** Remove from `set_options()` - backend should preserve existing value
- **Option C:** Document that it's .env only (not user-configurable)

**Recommendation:** Option A - add to UI for consistency, or Option B - exclude from payload.

---

### 3. Factory reset flow has redundant save

**Location:** `src/app/remote-options/remote-options.ts:306-307`

```typescript
// Save the reset options
await saveOptions();
```

**Problem:** After `factory_reset_options()` already persists to disk, frontend calls `saveOptions()` again which calls `set_options()`.

**Impact:** Double write to disk (harmless but unnecessary)

**Fix:** Remove the `await saveOptions()` call after reset.

---

### 4. No error feedback for store persistence failures

**Location:** Backend `set_options()` - current implementation

**Problem:** If `store.save()` fails, we return error. But frontend just shows generic "Failed to save" without explaining why.

**Enhancement:** Add specific error messages:
- "Disk full - settings saved for this session only"
- "Permission denied - check app data directory"

---

### 5. Missing `require_pass_in` in frontend form

**Location:** `src/app/remote-options/remote-options.html` (need to check)

**Problem:** UI doesn't have input for `require_pass_in`, but it's a user-configurable timing value.

**Fix:** Add form field or document as .env-only.

---

### 6. No loading state during save

**Location:** Frontend `saveOptions()`

**Problem:** User clicks save, no visual feedback until alert appears.

**Enhancement:** Disable save button + show spinner during `invoke()` call.

---

## Recommended Fixes (Priority Order)

### P0 - Critical (Data Corruption Risk)

1. **Fix `set_options()` payload** - Preserve URLs and existing `require_pass_in`
   - Frontend fetches current options, modifies only editable fields
   - OR backend preserves URLs regardless of input

### P1 - Important (User Experience)

2. **Add `require_pass_in` to UI** or exclude from payload
3. **Add loading state** to save button
4. **Remove redundant save** after factory reset

### P2 - Nice to Have

5. **Better error messages** for persistence failures
6. **Success toast** instead of alert (less intrusive)

---

## Proposed Architecture Update

### Backend: Preserve URLs in `set_options()`

```rust
#[tauri::command]
fn set_options<R: Runtime>(
    app: AppHandle<R>, 
    state: tauri::State<AppState>, 
    options: AppOptions
) -> Result<(), String> {
    // Get current options to preserve URLs
    let current = state.options.lock().unwrap();
    let preserved_urls = AppOptions {
        saver_url: current.saver_url.clone(),
        saver_url_debug: current.saver_url_debug.clone(),
        options_url: current.options_url.clone(),
        ..options.clone() // User-provided values override
    };
    drop(current);
    
    // Update in-memory state with preserved URLs
    let mut current = state.options.lock().unwrap();
    *current = preserved_urls.clone();
    drop(current);
    
    // Persist to store (only non-URL fields)
    let store = app.store("options.json")?;
    store.set("startsIn", options.starts_in);
    store.set("displayOffIn", options.display_off_in);
    store.set("requirePassIn", options.require_pass_in);
    store.set("runOnBattery", options.run_on_battery);
    store.set("debug", options.debug);
    store.save()?;
    
    Ok(())
}
```

### Frontend: Fetch-Modify-Save Pattern

```typescript
async function saveOptions(): Promise<void> {
  try {
    // Fetch current options first
    const current = await invoke<AppOptions>('get_options');
    
    // Modify only user-editable fields
    const newOptions = {
      ...current,
      starts_in: startsInInput ? parseFloat(startsInInput.value) : current.starts_in,
      display_off_in: displayOffInput ? parseFloat(displayOffInput.value) : current.display_off_in,
      run_on_battery: runOnBatteryInput ? runOnBatteryInput.checked : current.run_on_battery,
      debug: debugInput ? debugInput.checked : current.debug,
      require_pass_in: requirePassInput ? parseFloat(requirePassInput.value) : current.require_pass_in,
    };
    
    await invoke('set_options', { options: newOptions });
    showSuccess('Settings saved');
  } catch (error) {
    showError('Failed to save: ' + error);
  }
}
```

---

## Testing Plan

### Test Case 1: Fresh Install
1. Delete `options.json`
2. Start app
3. Open options
4. Verify values match `.env` defaults
5. Change `starts_in` to 5.0
6. Save
7. Restart app
8. Verify `starts_in` is still 5.0

### Test Case 2: URL Preservation
1. Note current `saver_url` from `.env`
2. Change `starts_in` in UI
3. Save
4. Check `get_options()` - URL should be unchanged
5. Verify `options.json` doesn't contain URL fields

### Test Case 3: Factory Reset
1. Change multiple settings
2. Save
3. Click "Reset to Defaults"
4. Verify settings match `.env` defaults
5. Restart app
6. Verify defaults persist (not old custom values)

### Test Case 4: Concurrent Saves
1. Open options in two windows (if possible)
2. Change different values in each
3. Save in window A
4. Save in window B
5. Verify last save wins (no corruption)

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/app/remote-options/remote-options.ts` | Fix `saveOptions()` to fetch-modify-save pattern |
| `src/app/remote-options/remote-options.ts` | Remove redundant `saveOptions()` after reset |
| `src/app/remote-options/remote-options.html` | Add `require_pass_in` field (optional) |
| `src-tauri/src/lib.rs` | Add URL preservation in `set_options()` (defensive) |
| `src-tauri/src/lib.rs` | Add validation for all fields |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-04-17 | Backend preserves URLs | Defense-in-depth - even if frontend sends wrong data, backend protects fork-critical fields |
| 2026-04-17 | Add `require_pass_in` to UI | Consistency with other timing fields, user should control all timeouts |
| 2026-04-17 | Fetch-modify-save pattern | Prevents accidental data loss, clearer intent |
