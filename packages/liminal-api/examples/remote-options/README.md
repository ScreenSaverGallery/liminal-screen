# Remote Options Example

A reference options page for Liminal Screen forks. Fork developers copy this directory, customise it, and host it at the URL they set as `VITE_OPTIONS_URL`.

## Structure

```
remote-options/
  index.html   — UI (dark-themed, no framework)
  main.ts      — Logic (uses @liminal-screen/api)
  sw.js        — Service worker (stale-while-revalidate)
  package.json — Build scripts
```

## Quick start

```bash
# Install deps (only needed for TypeScript type-checking)
bun install

# Compile main.ts → main.js
bun run build

# Watch mode during development
bun run dev

# Type-check without emitting
bun run typecheck
```

Then open `index.html` in a browser — it will run in demo mode with mock data.

## Adding custom fields

Edit the `CUSTOM_FIELDS` array in `main.ts`. Each entry becomes an input appended to the screensaver URL as a query parameter:

```typescript
const CUSTOM_FIELDS: CustomFieldDef[] = [
  { key: 'theme',     label: 'Theme',            type: 'text',     defaultValue: 'dark', hint: 'dark | light | auto' },
  { key: 'speed',     label: 'Animation speed',  type: 'number',   defaultValue: 1.0, min: 0.1, max: 5, step: 0.1 },
  { key: 'showClock', label: 'Show clock',        type: 'checkbox', defaultValue: true },
];
```

## Deploying

1. Run `bun run build` to produce `main.js`.
2. Upload the whole directory to any static HTTPS host (GitHub Pages, Netlify, Vercel, etc.).
3. Point `VITE_OPTIONS_URL` (or the runtime `optionsUrl` option) to the hosted `index.html`.

## How it works

`main.ts` uses the `liminal-api` global injected by `index.html`:

```html
<script src="../../dist/liminal-api.global.js"></script>
```

For CDN distribution replace that line with:

```html
<script src="https://unpkg.com/@liminal-screen/api/dist/liminal-api.global.js"></script>
```

The API singleton (`LiminalAPI.liminalAPI`) auto-detects whether it's running inside a Tauri webview or a plain browser, and falls back to mock data in the latter case.

## Field reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `startsIn` | number | 5 | Minutes of inactivity before screensaver activates |
| `displayOffIn` | number | 10 | Minutes before display turns off |
| `requirePassIn` | number | 0 | Minutes before password required (0 = disabled) |
| `runOnBattery` | boolean | false | Run on battery power |
| `debug` | boolean | false | Load debug screensaver URL |
| `customOptions` | object | {} | Fork-defined key/value pairs appended to the saver URL |
