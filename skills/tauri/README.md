# Tauri v2 Skills

A comprehensive skill library for building desktop applications with Tauri v2.

## Overview

This project provides detailed guidance and best practices for developing Tauri v2 desktop applications. It covers everything from project setup to distribution.

## Core Architecture

### Frontend-Backend Architecture

Tauri v2 follows a hybrid architecture:
- **Frontend**: Built with modern web frameworks (React, Vue, Svelte, etc.)
- **Backend**: Native Rust code for system operations
- **Communication**: IPC through Commands and Events

### Key Features

- Cross-platform support (Windows, macOS, Linux)
- Small bundle size (~5-10MB)
- Native system integration
- Strong security model with permission system
- Auto-update capability

## Project Structure

```
tauri-v2/
├── SKILL.md                    # Main skill index
└── rules/                      # Detailed skill rules
    ├── building.md             # Build and development
    ├── commands.md             # Commands and Events
    ├── configuration.md        # tauri.conf.json guide
    ├── capabilities.md         # Capabilities configuration
    ├── clipboard.md            # Clipboard operations
    ├── dialog.md               # System dialogs
    ├── distribution.md         # Distribution and signing
    ├── filesystem.md           # File system operations
    ├── frontend-integration.md # Frontend integration
    ├── menu.md                 # Native menus
    ├── notifications.md        # System notifications
    ├── permissions.md          # Permission system
    ├── project-structure.md    # Project structure
    ├── shell.md                # Shell commands
    ├── tray.md                 # System tray
    ├── window-customization.md # Custom window styles
    └── window-management.md    # Window management
```

## Quick Start

### Create New Project

```bash
# Using pnpm
pnpm create tauri-app@latest

# Select your preferred frontend framework
# - React
# - Vue
# - Svelte
# - Solid
# - Angular
# - Preact
# - Vanilla
```

### Development

```bash
# Start development server
pnpm tauri dev

# Build for production
pnpm tauri build
```

### Package Manager

This project uses **pnpm** for package management:

```bash
# Install dependencies
pnpm install

# Add new dependency
pnpm add <package>

# Remove dependency
pnpm remove <package>

# Update dependencies
pnpm update
```

## Core Concepts

### Commands

Commands allow the frontend to call Rust functions:

```typescript
// Frontend call
import { invoke } from '@tauri-apps/api/core';

const result = await invoke('my_command', { param: 'value' });
```

```rust
// Rust definition
#[tauri::command]
fn my_command(param: String) -> String {
    format!("Received: {}", param)
}
```

### Events

Events enable bidirectional communication:

```typescript
// Send event from frontend
import { emit } from '@tauri-apps/api/event';
await emit('my_event', { data: 'hello' });

// Listen for events
import { listen } from '@tauri-apps/api/event';
await listen('backend_event', (event) => {
    console.log(event.payload);
});
```

### Permissions

Tauri v2 uses a permission-based security model:

```json
// capabilities/default.json
{
  "permissions": [
    "core:default",
    "fs:allow-appdata-read",
    "fs:allow-appdata-write",
    "shell:allow-open"
  ]
}
```

## Plugin System

Tauri v2 provides official plugins for common functionality:

| Plugin | Description | Package |
|--------|-------------|---------|
| Shell | Execute commands | `@tauri-apps/plugin-shell` |
| Dialog | File picker, message boxes | `@tauri-apps/plugin-dialog` |
| FS | File system operations | `@tauri-apps/plugin-fs` |
| Clipboard | Clipboard operations | `@tauri-apps/plugin-clipboard-manager` |
| Notification | System notifications | `@tauri-apps/plugin-notification` |
| HTTP | HTTP client | `@tauri-apps/plugin-http` |
| Updater | Auto-update | `@tauri-apps/plugin-updater` |

## Installation

```bash
# Add Rust plugin
cargo add tauri-plugin-<name>

# Add frontend package
pnpm add @tauri-apps/plugin-<name>
```

## Configuration

### tauri.conf.json

The main configuration file:

```json
{
  "productName": "My App",
  "version": "0.1.0",
  "identifier": "com.example.myapp",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173"
  },
  "app": {
    "windows": [{
      "title": "My App",
      "width": 800,
      "height": 600
    }]
  },
  "bundle": {
    "active": true,
    "targets": "all"
  }
}
```

## Security Best Practices

1. **Least Privilege**: Request only necessary permissions
2. **Input Validation**: Validate all user input in Rust commands
3. **CSP Configuration**: Configure Content Security Policy
4. **Capability Scoping**: Use narrow scopes for file system access
5. **Command Whitelisting**: Whitelist allowed shell commands

## Distribution

### Platform Support

| Platform | Formats | Notes |
|----------|---------|-------|
| macOS | DMG, APP, MAS | Requires Apple Developer account |
| Windows | MSI, NSIS, EXE | Requires code signing |
| Linux | DEB, AppImage, RPM | Various packaging options |

### Code Signing

- **macOS**: Developer ID certificate + notarization
- **Windows**: Code signing certificate
- **Linux**: GPG signing for repositories

### Auto-Update

Configure the updater plugin for seamless updates:

```rust
tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
    // ...
```

## Resources

- [Official Documentation](https://tauri.app/)
- [API Reference](https://tauri.app/reference/javascript/api/)
- [GitHub Repository](https://github.com/tauri-apps/tauri)
- [Plugin Repository](https://github.com/tauri-apps/plugins-workspace)

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
