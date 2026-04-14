# Power Management Implementation Plan

## Overview
This document details the proper implementation of power management functions for the Liminal Screen application. Currently, the `prevent_display_sleep` and `allow_display_sleep` functions store dummy values instead of implementing actual platform-specific power assertion APIs.

## Requirements
1. Prevent system from sleeping/entering power saving mode while screensaver is active
2. Allow system to return to normal power management when screensaver deactivates
3. Handle errors gracefully on all platforms
4. Use appropriate platform-specific APIs

## Platform-Specific Implementation Details

### Windows
**API**: `SetThreadExecutionState` from Win32 System Power API
**Function**: `windows::Win32::System::Power::SetThreadExecutionState`
**Flags**: `ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED | ES_CONTINUOUS`
**Return Value**: Previous execution state flags
**Error Handling**: Win32 error codes via GetLastError()

### macOS
**API**: IOKit Power Management Assertions
**Framework**: IOPMLib.h
**Functions**: 
- `IOPMAssertionCreateWithName` to create assertion
- `IOPMAssertionRelease` to release assertion
**Assertion Type**: `kIOPMAssertionTypePreventUserIdleDisplaySleep`
**Return Value**: Assertion ID for later release
**Error Handling**: kern_return_t error codes

### Linux
**Approach**: systemd-inhibit command or direct D-Bus calls
**Method 1**: systemd-inhibit command
- Command: `systemd-inhibit --what=sleep:idle --who=LiminalScreen --why="Screensaver active" --mode=block COMMAND`
**Method 2**: Direct D-Bus calls to org.freedesktop.login1
**Identifier**: Process inhibitor cookie/filename
**Error Handling**: Command exit codes and stderr output

## Implementation Steps

1. Update PowerSaveBlocker struct to store actual platform-specific assertion identifiers
2. Implement proper prevent_sleep functions for each platform
3. Implement proper allow_sleep functions for each platform  
4. Add comprehensive error handling and logging
5. Test implementation across all platforms

## Expected Data Structure Changes

### Windows
Change PowerSaveBlocker assertion_id from Option<u32> to Option<EXECUTION_STATE>

### macOS
Change PowerSaveBlocker assertion_id from Option<u32> to Option<IOPMAssertionID>

### Linux
Change PowerSaveBlocker assertion_id from Option<String> to Option<String> (for inhibitor cookie/path)