# Liminal Screen API Specification

## Overview

The Liminal Screen API provides a standardized interface for remote options pages to communicate with the Liminal Screen application. This API enables third-party developers to create custom options interfaces that integrate seamlessly with the main application.

## API Client

The API is accessed through the `LiminalAPI` class, which provides methods for all supported operations.

### Installation

```bash
npm install @liminal-screen/api
```

### Usage

```javascript
import { liminalAPI } from '@liminal-screen/api';

// Initialize the API (automatically detects Tauri environment)
await liminalAPI.init();

// Get current options
const options = await liminalAPI.getOptions();

// Set options
await liminalAPI.setOptions({ debug: true });

// Listen for updates
const unsubscribe = liminalAPI.onOptionsUpdate((options) => {
  console.log('Options updated:', options);
});

// Clean up
unsubscribe();
```

## API Endpoints

### Get Options

Retrieves the current application options.

**Method**: `getOptions()`

**Response**:
```typescript
interface LiminalOptions {
  /** Time in minutes before screensaver starts */
  startsIn: number;

  /** Time in minutes before display turns off */
  displayOffIn: number;

  /** Time in minutes before password is required */
  requirePassIn: number;

  /** Whether to run on battery power */
  runOnBattery: boolean;

  /** Debug mode enabled */
  debug: boolean;

  /** Main screensaver URL */
  saverUrl: string;

  /** Debug screensaver URL */
  saverUrlDebug: string;

  /** Options page URL */
  optionsUrl: string;
}
```

**Example Response**:
```json
{
  "startsIn": 0.2,
  "displayOffIn": 1.0,
  "requirePassIn": 1.0,
  "runOnBattery": false,
  "debug": false,
  "saverUrl": "https://save.screensaver.gallery",
  "saverUrlDebug": "https://save.screensaver.gallery/debug",
  "optionsUrl": "http://localhost:3000/options"
}
```

### Set Options

Updates the application options.

**Method**: `setOptions(partialOptions)`

**Parameters**:
- `partialOptions`: Partial<LiminalOptions> - Only the fields to update

**Example**:
```javascript
await liminalAPI.setOptions({
  startsIn: 0.5,
  debug: true
});
```

### Reset Options

Resets all options to their factory defaults.

**Method**: `resetOptions()`

**Response**: `LiminalOptions` - The reset options

### Preview Screensaver

Triggers a preview of the screensaver.

**Method**: `previewScreensaver()`

### Listen for Updates

Subscribe to options updates from the main application.

**Method**: `onOptionsUpdate(callback)`

**Parameters**:
- `callback`: Function that receives updated options

**Returns**: Unsubscribe function

## Environment Detection

The API automatically detects whether it's running in a Tauri environment:

- **Tauri Environment**: Real IPC communication with the main app
- **Browser Environment**: Mock implementations for testing/demo purposes

You can check the environment manually:
```javascript
if (liminalAPI.isInTauri()) {
  console.log("Running in Tauri environment");
}
```

## Error Handling

All API methods may throw `LiminalAPIError` exceptions:

```javascript
try {
  await liminalAPI.setOptions({ startsIn: 0.5 });
} catch (error) {
  if (error instanceof LiminalAPIError) {
    console.error("API Error:", error.message);
  }
}
```

## Events

### Options Update

Emitted when options are updated in the main application.

**Event**: `liminal-options-update`

**Payload**: `LiminalOptions`

You can listen for this event directly:
```javascript
window.addEventListener('liminal-options-update', (event) => {
  console.log('Options updated:', event.detail);
});
```

## Integration Guide

### Basic HTML Page

```html
<!DOCTYPE html>
<html>
<head>
  <title>Custom Options</title>
</head>
<body>
  <form id="options-form">
    <input type="number" id="starts-in" />
    <button type="submit">Save</button>
  </form>

  <script type="module">
    import { liminalAPI } from 'https://unpkg.com/@liminal-screen/api/dist/index.js';

    // Initialize API
    await liminalAPI.init();

    // Load current options
    const options = await liminalAPI.getOptions();
    document.getElementById('starts-in').value = options.startsIn;

    // Handle form submission
    document.getElementById('options-form').addEventListener('submit', async (e) => {
      e.preventDefault();
      const startsIn = parseFloat(document.getElementById('starts-in').value);
      await liminalAPI.setOptions({ startsIn: startsIn });
    });

    // Listen for updates from main app
    liminalAPI.onOptionsUpdate((options) => {
      document.getElementById('starts-in').value = options.startsIn;
    });
  </script>
</body>
</html>
```

## Security Considerations

- All IPC communication is sandboxed by Tauri
- Options updates are validated by the main application
- Remote options pages cannot access sensitive system APIs directly

## Versioning

This API follows semantic versioning. Breaking changes will result in major version increments.