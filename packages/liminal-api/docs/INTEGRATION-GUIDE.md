# Liminal Screen Integration Guide

## Overview

This guide explains how third-party developers can integrate with the Liminal Screen application using the official API. The API provides a standardized way to create custom options pages that can communicate with the main Liminal Screen application.

## Prerequisites

Before you begin, ensure you have:

- Basic knowledge of HTML, CSS, and JavaScript
- Access to a web server or static hosting for your options page
- Understanding of the Liminal Screen application features

## Getting Started

### Installation

The Liminal Screen API can be installed via npm:

```bash
npm install @liminal-screen/api
```

Or loaded directly from a CDN:

```html
<script type="module" src="https://unpkg.com/@liminal-screen/api/dist/index.js"></script>
```

### Basic Usage

Here's a minimal example to get started:

```html
<!DOCTYPE html>
<html>
<head>
    <title>My Liminal Options</title>
</head>
<body>
    <form id="options-form">
        <input type="number" id="starts-in" placeholder="Start after (minutes)" />
        <button type="submit">Save</button>
    </form>

    <script type="module">
        import { liminalAPI } from '@liminal-screen/api';

        // Initialize the API
        await liminalAPI.init();

        // Load current options
        const options = await liminalAPI.getOptions();
        document.getElementById('starts-in').value = options.starts_in;

        // Handle form submission
        document.getElementById('options-form').addEventListener('submit', async (e) => {
            e.preventDefault();
            const startsIn = parseFloat(document.getElementById('starts-in').value);
            await liminalAPI.setOptions({ starts_in: startsIn });
        });
    </script>
</body>
</html>
```

## API Reference

### Initialization

Always initialize the API before use:

```javascript
import { liminalAPI } from '@liminal-screen/api';

await liminalAPI.init();
```

The API automatically detects whether it's running in a Tauri environment and adjusts its behavior accordingly.

### Getting Options

Retrieve the current application options:

```javascript
const options = await liminalAPI.getOptions();
console.log(options);
```

**Response Structure:**
```typescript
{
  starts_in: number;        // Time in minutes before screensaver starts
  display_off_in: number;   // Time in minutes before display turns off
  require_pass_in: number;  // Time in minutes before password required
  run_on_battery: boolean;  // Whether to run on battery power
  debug: boolean;           // Debug mode enabled
  saver_url: string;        // Main screensaver URL
  saver_url_debug: string;  // Debug screensaver URL
  options_url: string;      // Options page URL
}
```

### Setting Options

Update application options:

```javascript
await liminalAPI.setOptions({
  starts_in: 0.5,
  debug: true
});
```

You can update any subset of options. Only the provided fields will be updated.

### Resetting Options

Reset all options to factory defaults:

```javascript
const defaultOptions = await liminalAPI.resetOptions();
console.log('Options reset to:', defaultOptions);
```

### Previewing Screensaver

Trigger a preview of the screensaver:

```javascript
await liminalAPI.previewScreensaver();
```

### Listening for Updates

Subscribe to options updates from the main application:

```javascript
const unsubscribe = liminalAPI.onOptionsUpdate((options) => {
  console.log('Options updated:', options);
  // Update your UI here
});

// Later, to unsubscribe:
unsubscribe();
```

## Environment Detection

The API automatically detects the environment:

```javascript
if (liminalAPI.isInTauri()) {
  console.log("Running in Liminal Screen app");
} else {
  console.log("Running in browser (demo mode)");
}
```

In Tauri environments, all operations communicate with the main application. In browser environments, operations use mock implementations for testing.

## Error Handling

All API methods may throw `LiminalAPIError`:

```javascript
try {
  await liminalAPI.setOptions({ starts_in: 0.5 });
} catch (error) {
  if (error instanceof LiminalAPIError) {
    console.error("API Error:", error.message);
  }
}
```

## Best Practices

### 1. Always Initialize First

```javascript
// Good
await liminalAPI.init();
const options = await liminalAPI.getOptions();

// Bad - may fail if not initialized
const options = await liminalAPI.getOptions();
```

### 2. Handle Errors Gracefully

```javascript
try {
  await liminalAPI.setOptions({ starts_in: newValue });
  showSuccess("Settings saved!");
} catch (error) {
  showError("Failed to save settings");
}
```

### 3. Validate User Input

```javascript
const startsIn = parseFloat(input.value);
if (isNaN(startsIn) || startsIn < 0.1) {
  showError("Start time must be at least 0.1 minutes");
  return;
}
```

### 4. Provide Visual Feedback

```javascript
// Show loading state
showLoading(true);

try {
  await liminalAPI.saveOptions(formData);
  showSuccess("Settings saved!");
} catch (error) {
  showError(error.message);
} finally {
  showLoading(false);
}
```

## Security Considerations

- All IPC communication is sandboxed by Tauri
- Remote options pages cannot access sensitive system APIs directly
- Options updates are validated by the main application
- User consent is required for certain operations

## Deployment

### Testing Locally

During development, you can test your options page in a regular browser. The API will operate in demo mode with mock data.

### Production Deployment

Deploy your options page to any static hosting service. Ensure your page is accessible via HTTPS for production use.

### Configuring Liminal Screen

To use your custom options page:

1. Host your HTML file at a publicly accessible URL
2. Set the `VITE_OPTIONS_URL` environment variable to point to your page
3. Restart the Liminal Screen application

Example `.env` configuration:
```
VITE_OPTIONS_URL=https://your-domain.com/my-options.html
```

## Troubleshooting

### API Not Loading

Ensure you're importing the API correctly:

```javascript
// Correct
import { liminalAPI } from '@liminal-screen/api';

// Incorrect
import liminalAPI from '@liminal-screen/api';
```

### Operations Not Working

Check if you're running in the correct environment:

```javascript
if (!liminalAPI.isInTauri()) {
  console.warn("API operations only work in Liminal Screen app");
}
```

### CORS Issues

When testing locally, you may encounter CORS issues. These are normal and indicate the API is working correctly in browser mode.

## Support

For issues with the API or integration questions:

1. Check the [API documentation](API.md)
2. Review the example implementations
3. Open an issue on the Liminal Screen GitHub repository

## Versioning

This API follows semantic versioning. Breaking changes will be communicated in advance and result in major version increments.