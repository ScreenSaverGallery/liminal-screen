# Screensaver Engine State Machine & Timing Logic

**Created:** 2026-04-17  
**Status:** Draft - Pending Implementation  
**Priority:** High (fixes critical timing bug)

---

## Executive Summary

This plan addresses critical bugs and missing features in the screensaver engine timing logic:

1. **Critical Bug:** `display_off_in < starts_in` never triggers display blank (line 135 bug)
2. **Missing Feature:** `require_pass_in` field exists but is not implemented
3. **Architecture:** Implicit state logic → Explicit state machine
4. **Energy Saving:** Close screensaver windows before/when display blanks

---

## Current State Analysis

### File: `src-tauri/src/screensaver_engine.rs`

**Current logic (lines 106-140):**

```rust
// Line 109: Activation
if idle_time >= starts_in_seconds && !currently_active {
    self.request_activate(app);
}

// Line 122: Deactivation
else if idle_time < starts_in_seconds && currently_active {
    self.request_deactivate(app);
}

// Line 135: Display blank (BUGGY)
else if idle_time >= display_off_seconds && currently_active {
    // BUG: && currently_active means this NEVER fires if display_off_in < starts_in
    match super::power_monitor::blank_screen() { ... }
}
```

**Bug explanation:**
- If `display_off_in = 1min` and `starts_in = 5min`
- At 1min idle: `currently_active = false` (screensaver not started yet)
- Condition `idle_time >= display_off_seconds && currently_active` evaluates to `true && false = false`
- **Result:** Display never blanks

---

## Proposed State Machine

### States

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreensaverState {
    Idle,              // Monitoring, no windows, display on
    ScreensaverActive, // Windows visible, display on
    DisplayOff,        // Display blanked, monitoring continues
    Locked,            // System locked, requires authentication
}
```

### State Transition Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        STATE MACHINE                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────┐                                                      │
│  │ IDLE │ ◄─────────────────────────────────────────┐          │
│  └───┬──┘                                           │          │
│      │ idle_time >= starts_in AND                   │          │
│      │ starts_in < display_off_in                   │          │
│      ▼                                              │          │
│  ┌──────────────────┐                               │          │
│  │ SCREENSAVER_ACTIVE │                            │          │
│  │ (windows visible)  │◄──────────────────┐         │          │
│  └───┬──────────────┘                     │         │          │
│      │ idle_time >= display_off_in        │         │          │
│      │ (close windows, then blank)        │         │          │
│      ▼                                    │         │          │
│  ┌──────────────────┐                     │         │          │
│  │  DISPLAY_OFF     │─────────────────────┤         │          │
│  │ (display blanked,│  idle_time <        │         │          │
│  │  monitoring continues)                 │         │          │
│  └───┬──────────────┘  starts_in          │         │          │
│      │                                    │         │          │
│      │ user activity                      │         │          │
│      │ (display wakes, idle resets)       │         │          │
│      └────────────────────────────────────┘         │          │
│                                                     │          │
│  ┌──────────────────┐                               │          │
│  │     LOCKED       │───────────────────────────────┘          │
│  │ (password prompt)│  (require_pass_in > 0 AND                │
│  └──────────────────┘   idle_time >= require_pass_in)          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Transition Table

| From | To | Trigger | Action |
|------|----|---------|--------|
| `Idle` | `ScreensaverActive` | `idle >= starts_in` AND `starts_in < display_off_in` | Create windows |
| `Idle` | `DisplayOff` | `idle >= display_off_in` AND `display_off_in <= starts_in` | Blank display |
| `ScreensaverActive` | `DisplayOff` | `idle >= display_off_in` | Close windows, blank display |
| `ScreensaverActive` | `Idle` | `idle < starts_in` | Close windows, allow sleep |
| `DisplayOff` | `Idle` | User activity (idle resets) | Allow display sleep |
| `DisplayOff` | `ScreensaverActive` | Display wakes AND `idle >= starts_in` | Create windows |
| Any | `Locked` | `idle >= require_pass_in` AND `require_pass_in > 0` | System lock |
| `Locked` | `Idle` | User authentication (unlock) | Reset state |

---

## Implementation Phases

### Phase 1: Core State Machine & Timing Fixes

#### 1.1 Add State Enum

**File:** `src-tauri/src/screensaver_engine.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ScreensaverState {
    Idle,
    ScreensaverActive,
    DisplayOff,
    Locked,
}
```

#### 1.2 Replace Boolean Flags

**Current:**
```rust
pub struct ScreensaverEngine {
    is_monitoring: Arc<AtomicBool>,
    is_active: Arc<AtomicBool>,
    pending_transition: Arc<AtomicBool>,
}
```

**Updated:**
```rust
pub struct ScreensaverEngine {
    is_monitoring: Arc<AtomicBool>,
    state: Arc<Mutex<ScreensaverState>>,  // Changed from AtomicBool
    pending_transition: Arc<AtomicBool>,
}
```

#### 1.3 Fix Monitoring Loop Priority Order

**New logic order (critical):**

```rust
fn check_and_manage_screensaver<R: tauri::Runtime>(
    &self,
    app: &AppHandle<R>,
) -> Result<(), String> {
    let idle_time = super::power_monitor::get_system_idle_time()
        .map_err(|e| format!("Failed to get idle time: {}", e))?;

    let state = app.state::<super::AppState>();
    let options = state.options.lock().unwrap();
    let starts_in_seconds = (options.starts_in * 60.0) as u64;
    let display_off_seconds = (options.display_off_in * 60.0) as u64;
    let require_pass_seconds = (options.require_pass_in * 60.0) as u64;
    let run_on_battery = options.run_on_battery;
    drop(options);

    // Battery check (unchanged)
    if !run_on_battery {
        match super::power_monitor::is_on_battery_power() {
            Ok(on_battery) => {
                if on_battery {
                    if self.get_state() != ScreensaverState::Idle {
                        self.request_deactivate(app);
                    }
                    return Ok(());
                }
            }
            Err(e) => println!("Warning: Failed to check battery status: {}", e),
        }
    }

    let current_state = self.get_state();

    // === PRIORITY 1: LOCK (Security) ===
    if require_pass_seconds > 0 && idle_time >= require_pass_seconds {
        if current_state != ScreensaverState::Locked {
            self.request_lock(app);
        }
        return Ok(()); // Early return - nothing else matters when locked
    }

    // === PRIORITY 2: DISPLAY OFF (Power Saving) ===
    if idle_time >= display_off_seconds && current_state != ScreensaverState::DisplayOff {
        // Close windows if they're active
        if current_state == ScreensaverState::ScreensaverActive {
            self.close_all_savers(app)?;
        }
        match super::power_monitor::blank_screen() {
            Ok(_) => {
                println!("Display blanked due to extended idle");
                self.set_state(ScreensaverState::DisplayOff);
            }
            Err(e) => println!("Failed to blank display: {}", e),
        }
        return Ok(());
    }

    // === PRIORITY 3: SCREENSAVER ACTIVATION (Visual) ===
    if idle_time >= starts_in_seconds 
        && current_state == ScreensaverState::Idle 
        && starts_in_seconds < display_off_seconds 
    {
        if !self.pending_transition.load(Ordering::Relaxed) {
            self.request_activate(app);
        }
        return Ok(());
    }

    // === PRIORITY 4: DEACTIVATION (User Activity) ===
    if idle_time < starts_in_seconds && current_state != ScreensaverState::Idle {
        if !self.pending_transition.load(Ordering::Relaxed) {
            self.request_deactivate(app);
        }
        return Ok(());
    }

    Ok(())
}
```

#### 1.4 Add State Accessors

```rust
impl ScreensaverEngine {
    pub fn get_state(&self) -> ScreensaverState {
        *self.state.lock().unwrap()
    }

    fn set_state(&self, new_state: ScreensaverState) {
        let mut state = self.state.lock().unwrap();
        let old_state = *state;
        *state = new_state;
        println!("State transition: {:?} → {:?}", old_state, new_state);
    }
}
```

---

### Phase 2: System Lock Implementation

#### 2.1 Add `lock_system()` to Power Monitor

**File:** `src-tauri/src/power_monitor.rs`

```rust
pub fn lock_system() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        // Method 1: CGSession -suspend (requires accessibility permissions)
        let output = Command::new("CGSession")
            .arg("-suspend")
            .output();
        
        match output {
            Ok(status) => {
                if status.status.map_or(false, |s| s.success()) {
                    println!("System locked via CGSession");
                    return Ok(());
                }
                // Fall through to Method 2
            }
            Err(e) => println!("CGSession failed: {}", e),
        }
        
        // Method 2: pmset displaysleepnow (sleeps display, doesn't lock)
        // For full lock, we may need to use loginwindow
        let output = Command::new("/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession")
            .arg("-suspend")
            .output();
        
        if output.map_or(false, |o| o.status.map_or(false, |s| s.success())) {
            return Ok(());
        }
        
        // Fallback: log warning
        println!("Warning: Could not lock system (macOS)");
        return Ok(()); // Don't fail - continue monitoring
    }
    
    #[cfg(target_os = "windows")]
    {
        // LockWorkStation() from User32.dll
        // Requires FFI binding
        println!("System lock requested (Windows - TODO)");
        return Ok(());
    }
    
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        
        // Try loginctl first (systemd)
        let output = Command::new("loginctl")
            .arg("lock-session")
            .output();
        
        if output.map_or(false, |o| o.status.map_or(false, |s| s.success())) {
            return Ok(());
        }
        
        // Fallback: gnome-screensaver-command
        let output = Command::new("gnome-screensaver-command")
            .arg("--lock")
            .output();
        
        if output.map_or(false, |o| o.status.map_or(false, |s| s.success())) {
            return Ok(());
        }
        
        println!("Warning: Could not lock system (Linux)");
        return Ok(());
    }
    
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        println!("Warning: Lock not implemented for this platform");
        Ok(())
    }
}
```

#### 2.2 Add `request_lock()` to Engine

```rust
fn request_lock<R: tauri::Runtime>(&self, app: &AppHandle<R>) {
    self.pending_transition.store(true, Ordering::Relaxed);
    
    let app = app.clone();
    let engine = self.clone();
    
    app.run_on_main_thread(move || {
        match super::power_monitor::lock_system() {
            Ok(_) => {
                println!("System locked");
                engine.set_state(ScreensaverState::Locked);
                let _ = app.emit("screensaver-locked", ());
            }
            Err(e) => {
                println!("Failed to lock system: {}", e);
                // Still set state to prevent deactivation
                engine.set_state(ScreensaverState::Locked);
            }
        }
        engine.pending_transition.store(false, Ordering::Relaxed);
    });
}
```

---

### Phase 3: State Events & Frontend Integration

#### 3.1 Add State Change Events

```rust
// In set_state(), after transition:
let _ = app.emit("screensaver-state-changed", ScreensaverStatePayload {
    state: new_state,
    idle_time,
    timestamp: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs(),
});
```

#### 3.2 Frontend Handler (TODO)

**File:** `src/app/power-monitor/power-monitor.ts` (or new file)

```typescript
listen('screensaver-state-changed', (event) => {
    const { state, idle_time } = event.payload;
    // Update UI, show warnings, etc.
});
```

---

### Phase 4: Testing

#### 4.1 Test Matrix

| Test # | starts_in | display_off_in | require_pass_in | Expected Behavior |
|--------|-----------|----------------|-----------------|-------------------|
| T1 | 1 min | 5 min | 0 | Screensaver at 1min, display off at 5min, no lock |
| T2 | 5 min | 1 min | 0 | Display off at 1min (no screensaver), wake → idle resets |
| T3 | 2 min | 3 min | 4 min | Screensaver 2min, display off 3min, lock 4min |
| T4 | 5 min | 2 min | 3 min | Display off 2min, lock 3min (no screensaver) |
| T5 | 0.5 min | 0.5 min | 0 | Edge case: equal times - display off fires (power priority) |
| T6 | 1 min | 10 min | 0.5 min | Lock at 0.5min (before screensaver) |

#### 4.2 Manual Test Scenarios

1. **T2 Verification:** Set `display_off_in < starts_in`, verify display blanks without screensaver windows
2. **Lock Test:** Set `require_pass_in = 0.5min`, verify system locks after 30 seconds idle
3. **Wake Test:** After display off, move mouse, verify idle timer resets
4. **Battery Test:** Unplug laptop, verify screensaver deactivates

---

## Edge Cases & Decisions

### Decision Log

| Decision | Rationale |
|----------|-----------|
| `display_off_in <= starts_in` → skip screensaver | No point creating windows user can't see |
| Lock has highest priority | Security > power saving > visual |
| Equal times (`display_off_in == starts_in`) → display off wins | Power saving priority |
| State not persisted across restarts | Ephemeral - always start from `Idle` |
| Lock failure doesn't block monitoring | Graceful degradation - continue other functions |
| Pre-close margin = 0 (Option D) | Simpler, no visual jarring of "desktop before blank" |

### Known Edge Cases

| Case | Handling |
|------|----------|
| `require_pass_in == 0` | Lock feature disabled (skip check) |
| Display wakes during `DisplayOff` | Reset to `Idle`, idle timer resets naturally |
| App restart during `Locked` | Start from `Idle` (state not persisted) |
| Multi-monitor during display off | OS handles - all displays blank together |
| Lock fails (permissions) | Log error, set state anyway, continue monitoring |

---

## Files to Modify

| File | Changes |
|------|---------|
| `src-tauri/src/screensaver_engine.rs` | State enum, state tracking, priority logic, lock request |
| `src-tauri/src/power_monitor.rs` | Add `lock_system()` function |
| `src-tauri/src/lib.rs` | Update AppState if needed |
| `src/app/power-monitor/` (TBD) | Add state event listeners |

---

## Open Questions

1. **Lock behavior confirmation:** System lock = full OS lock screen (not just password prompt)?
2. **Warning before lock:** Add countdown warning ("Locking in 30s")? Configurable?
3. **Platform priority:** macOS first for lock implementation?
4. **State events:** Does frontend need real-time updates or is current model sufficient?

---

## Dependencies

- None (internal refactor + new feature)
- Does not affect: options UI, storage, remote options, preview

---

## Rollback Plan

If issues arise:
1. Revert `screensaver_engine.rs` to previous version
2. Keep `lock_system()` stub (harmless, not called without `require_pass_in > 0`)
3. Default `require_pass_in = 0` preserves existing behavior

---

## Success Criteria

- [ ] T2 passes: `display_off_in < starts_in` blanks display correctly
- [ ] T6 passes: `require_pass_in < starts_in` locks before screensaver
- [ ] No regressions: T1 still works (normal screensaver → display off flow)
- [ ] State transitions logged correctly in console
- [ ] Lock works on macOS (manual verification)

---

## Notes

- Monitoring loop continues during `DisplayOff` state (verified - no changes needed)
- `get_system_idle_time()` works regardless of display state (OS-level)
- State machine is independent of multi-monitor logic (orthogonal concerns)
