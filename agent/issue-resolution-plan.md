# Liminal Screen - Issue Resolution Plan

## Priority Issues Analysis

### 1. Screensaver Monitoring Doesn't Start Until Options Window Activated
**Problem**: The idle monitoring loop doesn't begin until the user opens the options window at least once.

**Root Cause**: The monitoring loop is only started in the `init()` function which runs when the main window (options window) is loaded. Since the window starts hidden, `init()` never runs until the window is shown.

**Solution Plan**:
1. Move the monitoring initialization to happen independently of the main window visibility
2. Ensure the main JavaScript context loads even when window is hidden
3. Implement proper application lifecycle management that starts monitoring on app launch

### 2. Preview Window Media Persistence Issue
**Problem**: Preview window continues playing media after "closing" because it only hides instead of actually closing.

**Root Cause**: The preview window uses `previewWindow.hide()` instead of `previewWindow.close()` and doesn't properly clean up media content before hiding.

**Solution Plan**:
1. Implement proper close event handling for preview window
2. Navigate to "about:blank" before hiding/closing to stop media playback
3. Ensure complete cleanup of window resources

### 3. Preview Window Code Organization
**Problem**: Preview functionality is mixed with main application code without clear separation.

**Solution Plan**:
1. Create dedicated Preview class to encapsulate preview functionality
2. Separate preview logic from main application logic
3. Make preview behavior consistent with saver windows

### 4. Multi-Monitor Fullscreen Issue
**Problem**: On dual monitor setups, one window doesn't properly fullscreen.

**Root Cause**: Race condition between window positioning and fullscreen setting; possibly timing issues with Tauri window management.

**Solution Plan**:
1. Add proper timing delays between window operations
2. Implement more robust fullscreen verification
3. Add error handling for fullscreen operations

## Detailed Implementation Plan

### Task 1: Fix Monitoring Initialization Issue

**Files to Modify**: `src/main.ts`

**Steps**:
1. Create a separate initialization function that runs regardless of window visibility
2. Ensure the main JavaScript context initializes monitoring even when window is hidden
3. Refactor the monitoring start to be independent of UI initialization

**Implementation Details**:
- Move monitoring setup outside of DOMContentLoaded event
- Ensure the main process starts monitoring when the app launches
- Verify that hidden window still executes JavaScript

### Task 2: Fix Preview Window Media Persistence

**Files to Modify**: `src/main.ts`

**Steps**:
1. Uncomment and fix the onCloseRequested handler for preview window
2. Implement proper media cleanup before window closure
3. Ensure window actually closes rather than just hiding

**Implementation Details**:
```typescript
previewWindow.onCloseRequested(async (event) => {
  event.preventDefault(); // Prevent default close
  if (previewWindow) {
    // Navigate to blank page to stop media
    try {
      await invoke("navigate_webview", {
        label: "preview",
        url: "about:blank"
      });
      // Small delay for navigation
      await new Promise(resolve => setTimeout(resolve, 100));
    } catch (error) {
      console.warn("Could not navigate preview to blank:", error);
    }
    // Actually close the window
    await previewWindow.close();
    previewWindow = null;
  }
});
```

### Task 3: Create Preview Class

**Files to Create**: `src/app/preview/preview.ts`

**Steps**:
1. Create new Preview class similar to Saver class
2. Move preview functionality from main.ts to Preview class
3. Ensure consistent API with Saver class

**Implementation Details**:
- Create dedicated Preview class with show/hide methods
- Implement proper cleanup in hide/close methods
- Add parameter to distinguish preview from regular saver

### Task 4: Fix Multi-Monitor Fullscreen Issue

**Files to Modify**: `src/app/saver/saver.ts`

**Steps**:
1. Add timing delays between window positioning and fullscreen setting
2. Implement fullscreen state verification
3. Add retry mechanism for fullscreen operations

**Implementation Details**:
```typescript
// In Saver.show() method after window creation:
await this.webviewWindow.once("tauri://created", async () => {
  console.log(`Window ${this.label} created successfully`);
  if (!resolved && this.webviewWindow) {
    resolved = true;
    
    try {
      // Add small delay to ensure window is properly positioned
      await new Promise(resolve => setTimeout(resolve, 100));
      
      // Set fullscreen and maximize with verification
      await this.webviewWindow.setFullscreen(true);
      await new Promise(resolve => setTimeout(resolve, 50)); // Small delay
      await this.webviewWindow.maximize();
      
      // Verify fullscreen state
      const isFullscreen = await this.webviewWindow.isFullscreen();
      if (!isFullscreen) {
        console.warn(`Window ${this.label} failed to go fullscreen, retrying...`);
        await this.webviewWindow.setFullscreen(true);
      }
      
      // Setup custom navigator properties
      await this.setupCustomNavigator();
      
      resolve();
    } catch (error) {
      console.error(`Error configuring saver window ${this.label}:`, error);
      resolve(); // Resolve anyway, window is created
    }
  }
});
```

## Priority Implementation Order

### Phase 1: Critical Fixes (High Priority)
1. **Fix Monitoring Initialization** - Without this, the entire application doesn't work properly
2. **Fix Preview Window Media Persistence** - User experience issue with media continuing to play

### Phase 2: Code Organization (Medium Priority)
3. **Create Preview Class** - Improves code maintainability and clarity

### Phase 3: Display Issues (Medium Priority)
4. **Fix Multi-Monitor Fullscreen** - Resolves display positioning problems

## Risk Assessment

### Low Risk Changes
- Preview window close handling
- Creating Preview class
- Adding timing delays

### Medium Risk Changes
- Moving monitoring initialization
- Modifying window lifecycle management

### Mitigation Strategies
- Implement gradual rollout with feature flags
- Add extensive logging for monitoring start
- Create backup monitoring mechanisms
- Test with various multi-monitor configurations

## Testing Approach

### For Monitoring Initialization
1. Launch application without opening options window
2. Verify monitoring loop starts and detects idle time
3. Confirm screensaver activates without manual window opening

### For Preview Window Fix
1. Open preview window
2. Play media content
3. Close preview window
4. Verify media stops playing

### For Multi-Monitor Fix
1. Test on dual monitor setup
2. Verify both windows go fullscreen correctly
3. Check window positioning on each display
4. Confirm no timing race conditions

## Timeline Estimate

### Phase 1 (1-2 days)
- Fix monitoring initialization
- Implement preview window cleanup

### Phase 2 (1 day)
- Create Preview class abstraction

### Phase 3 (1-2 days)
- Fix multi-monitor fullscreen issues
- Testing and refinement

Total estimated time: 3-5 days for complete implementation

## Dependencies

1. Tauri v2 API stability for window management
2. Proper error handling in Rust backend for navigation commands
3. Cross-platform compatibility of timing solutions

This plan addresses all four priority issues systematically while maintaining code quality and user experience.