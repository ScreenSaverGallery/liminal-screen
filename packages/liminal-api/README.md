# Liminal Screen API

A standardized API for integrating remote options pages with the Liminal Screen application.

## Overview

The Liminal Screen API provides a unified interface for remote options pages to communicate with the main Tauri application. This enables third-party developers to create custom options interfaces that integrate seamlessly with Liminal Screen.

## Features

- **Cross-Environment Compatibility**: Works in both Tauri applications and regular browsers
- **TypeScript Support**: Full typing for enhanced developer experience
- **Automatic Environment Detection**: Seamlessly switches between real IPC and mock implementations
- **Event-Based Updates**: Real-time synchronization with the main application
- **Comprehensive Error Handling**: Clear error messages and handling patterns

## Installation

```bash
npm install @liminal-screen/api
```

## Quick Start

```javascript
import { liminalAPI } from '@liminal-screen/api';

// Initialize the API
await liminalAPI.init();

// Get current options
const options = await liminalAPI.getOptions();
console.log(options);

// Update options
await liminalAPI.setOptions({ debug: true });

// Listen for changes
const unsubscribe = liminalAPI.onOptionsUpdate((updatedOptions) => {
  console.log('Options updated:', updatedOptions);
});

// Clean up when done
// unsubscribe();
```

## Documentation

See [API.md](docs/API.md) for complete API documentation and integration guides.

## Development

```bash
# Build the library
npm run build

# Watch for changes
npm run dev

# Run tests
npm test
```

## License

MIT