# Liminal Screen - Technical Debt and Known Issues

## Major Issues Identified

### 1. Incomplete Power Management Implementation
**Location**: `src-tauri/src/power_monitor.rs`
**Severity**: High
**Description**: The power management functions store dummy values instead of implementing platform-specific power assertion APIs.
**Impact**: Screensaver may not properly prevent display sleep, leading to inconsistent behavior.
**Required Changes**:
- Windows: Implement `SetThreadExecutionState` for sleep prevention
- macOS: Implement `IOPMAssertionCreate` for sleep prevention  
- Linux: Implement `systemd-inhibit` for sleep prevention

### 2. Remote Options Disabled
**Location**: `.env`
**Severity**: Medium
**Description**: The `VITE_OPTIONS_URL` is commented out, disabling remote options functionality.
**Impact**: Users cannot configure the application through the intended remote web interface.
**Required Changes**:
- Uncomment and properly configure the options URL
- Verify service worker implementation works correctly

### 3. Tauri v1/v2 API Mismatch
**Location**: Throughout codebase
**Severity**: Medium
**Description**: Some API patterns appear to follow Tauri v1 conventions while using Tauri v2.
**Impact**: Potential compatibility issues and unexpected behavior.
**Required Changes**:
- Audit all Tauri API calls for v2 compatibility
- Update event naming and handler patterns if needed

## Technical Debt Items

### 1. Error Handling Improvements Needed
**Location**: Across Rust plugins and TypeScript wrappers
**Severity**: Medium
**Description**: Error handling is basic with minimal platform-specific error details.
**Impact**: Difficult troubleshooting and reduced user experience when errors occur.
**Recommended Improvements**:
- Add detailed error context for each platform
- Implement proper error categorization and recovery strategies
- Add logging for debugging purposes

### 2. Service Worker Implementation Issues
**Location**: `src/app/options/sw.js`
**Severity**: Low-Medium
**Description**: Service worker caching strategy may not work correctly with actual deployment paths.
**Impact**: Offline support for options page may not function as expected.
**Recommended Improvements**:
- Verify caching paths match actual deployment structure
- Add more robust cache invalidation strategies
- Improve fallback mechanisms

### 3. Code Duplication
**Location**: Various UI components
**Severity**: Low
**Description**: Similar UI handling code exists in both main window and remote options.
**Impact**: Maintenance overhead when updating UI behavior.
**Recommended Improvements**:
- Extract common UI components into shared modules
- Standardize UI update patterns

## Performance Optimization Opportunities

### 1. Monitoring Loop Efficiency
**Location**: `src/main.ts`
**Issue**: Polling-based monitoring checks every second
**Opportunity**: Could use more efficient system event-based monitoring where available
**Potential Solution**: Investigate platform-specific idle notifications instead of polling

### 2. Window Management Optimization
**Location**: `src/app/saver/saver.ts`
**Issue**: Window creation/destruction may be slow with many displays
**Opportunity**: Reuse windows or pre-create for faster response
**Potential Solution**: Implement window pooling for frequently used displays

## Security Considerations

### 1. Content Security Policy
**Location**: `src-tauri/tauri.conf.json`
**Consideration**: CSP is relatively permissive
**Recommendation**: Tighten CSP for production releases while maintaining functionality

### 2. External URL Loading
**Location**: `src-tauri/src/lib.rs`
**Consideration**: Remote URLs loaded without extensive validation
**Recommendation**: Add URL validation and security scanning for loaded content

## Testing Gaps

### 1. Cross-Platform Testing
**Issue**: Limited automated testing across Windows, macOS, and Linux
**Recommendation**: Add platform-specific integration tests

### 2. Edge Case Coverage
**Issue**: Minimal testing of error conditions and edge cases
**Recommendation**: Add unit tests for error handling scenarios

## Future Enhancement Opportunities

### 1. Enhanced Synchronization
**Opportunity**: Add support for syncing settings across devices
**Approach**: Integrate with cloud storage or synchronization services

### 2. Advanced Scheduling
**Opportunity**: Add scheduling features (screensaver only during certain hours)
**Approach**: Extend options with time-based activation rules

### 3. Media Type Detection
**Opportunity**: Intelligent screensaver selection based on content type
**Approach**: Analyze media characteristics and select appropriate screensaver

## Immediate Action Items

1. **Fix Power Management Implementation** - Critical for proper screensaver functionality
2. **Enable Remote Options** - Restore intended configuration mechanism
3. **Audit Tauri v2 Compatibility** - Ensure all APIs are correctly implemented
4. **Add Comprehensive Error Handling** - Improve robustness and user experience
5. **Implement Automated Testing** - Add coverage for core functionality

This technical debt assessment provides a roadmap for improving the Liminal Screen application's stability, maintainability, and user experience.