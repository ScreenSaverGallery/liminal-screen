# Liminal Screen

A cross-platform screensaver application built with Tauri v2 that runs in the system tray and activates after a configurable period of system inactivity.

## Features

- **Multi-monitor support** - Displays on all monitors with proper fullscreen handling
- **Autoplay media** - Supports video/audio content without user interaction
- **System tray integration** - Runs silently in the background
- **Start at login** - Registers as an OS login item on first install (opt-out via the options window)
- **Configurable timing** - Customize activation delay, display off timing, and more
- **Remote options** - Load custom configuration from a web-based form
- **Cross-platform** - Works on macOS, Windows, and Linux

## For Developers: Forking and Rebranding

Liminal Screen is designed to be forked and rebranded for different organizations. Here's how to customize it:

### 1. Copy the Environment Template

```bash
cp .env.example .env
```

### 2. Edit `.env` with Your Branding

**Required changes:**

```bash
# App Identity
VITE_APP_NAME="Your App Name"
VITE_APP_DESCRIPTION="Your app description here"
VITE_APP_VERSION="1.0.0"                              # semver
VITE_APP_IDENTIFIER="com.yourcompany.your-app-name"  # MUST be unique per fork
VITE_APP_PUBLISHER="Your Org <support@your-domain.com>"  # Linux .deb / Windows Manufacturer
VITE_APP_COPYRIGHT="© 2026 Your Org"                      # optional bundle metadata

# Screensaver URLs
VITE_SAVER_URL="https://your-domain.com/screensaver"
VITE_SAVER_URL_DEBUG="https://your-domain.com/screensaver?debug=true"

# Remote Options (optional)
VITE_OPTIONS_URL="https://your-domain.com/options.html"

# Updater (REQUIRED if using the Tauri updater plugin)
VITE_UPDATER_PUBKEY="-----BEGIN PUBLIC KEY-----
 paste your public key here
-----END PUBLIC KEY-----"
VITE_UPDATER_ENDPOINT="https://your-domain.com/releases/latest/download/latest.json"
```

**Important:** These values are read at **build time**. The Tauri CLI does **not** natively substitute env vars into `tauri.conf.json`, so a build script (`scripts/build-tauri-config.ts`) reads `.env` and emits a Tauri merge-patch (`src-tauri/.tauri-runtime.conf.json`, gitignored) that is applied to the base config via `--config`. This runs automatically via the `tauri:dev` / `tauri:build` npm scripts — forks never need to edit `tauri.conf.json` directly.

The base `tauri.conf.json` carries **structural config** plus **obvious placeholder values** for per-fork fields (e.g. `"productName": "SET_VITE_APP_NAME_IN_.env"`, `"version": "0.0.0"`, `"identifier": "com.example.set-vite-app-identifier-in-env"`, `"pubkey": "SET_VITE_UPDATER_PUBKEY_IN_.env"`, `"endpoints": ["https://example.invalid/"]`). When you open the file, those placeholders are the signal that the real values come from `.env` — don't edit them here. (Tauri's JSON schema forbids unknown root keys, so there's no `_DO_NOT_EDIT` field; the placeholders serve that role.)

> **`endpoints` uses a real URL placeholder (`https://example.invalid/`), not the `SET_VITE_…` pattern** — the Tauri updater plugin deserializes `endpoints` as `Vec<Url>` at runtime, so a non-URL placeholder would crash the app at startup if the merge-patch weren't applied. The `https://example.invalid/` placeholder (RFC 2606 reserved TLD) is a valid URL, obviously fake, and the runtime updater is deactivated when `VITE_UPDATER_ENDPOINT` is unset (see below), so the placeholder is never actually fetched.
>
> **Updater deactivation:** If you haven't published a `latest.json` release feed yet, leave `VITE_UPDATER_ENDPOINT` empty in `.env`. The Rust updater module checks that env var and skips all update checks/downloads when it's unset — no `[updater] Error` noise in the logs. Set it once your release feed is live.

**Loading `.env` for production builds:** the merge-patch is generated from `.env` directly (the script handles multi-line values like the updater PEM), but the Rust backend's `option_env!` reads from the **OS environment at compile time** — not the `.env` file. Bun's automatic `.env` loading only fills the Bun JS `process.env` and does **not** propagate to child processes (`tauri`/`cargo`/`rustc`), so the `tauri:build` script wraps its command body in `bun --env-file=.env run` to load `.env` into the real OS environment. This preserves multi-line values like the updater PEM and works on PowerShell, cmd, bash, and zsh.

```bash
# Production build — the script applies --env-file internally; just run:
bun run tauri:build

# If invoking cargo directly outside Bun, use Bun's env loader:
bun --env-file=.env run cargo build
```

> Avoid `export $(cat .env | xargs)` — it breaks on multi-line values like the updater PEM.

**Why bundle identifier matters:** If two apps have the same identifier on one system, they'll share preferences, keychain entries, and may crash each other. Each fork MUST use a unique `VITE_APP_IDENTIFIER`.

**Why `VITE_APP_PUBLISHER` matters:** Tauri uses this value for the `.deb` package `Maintainer:` field (shown as the developer in Ubuntu App Center / GNOME Software) and for the Windows installer `Manufacturer`. The Cargo.toml `authors` field is intentionally removed so this env var is the single source of truth. Use `"Name <email>"` format for the best Linux storefront rendering.

**macOS Login Items:** The app name shown in System Settings comes from the bundle's `CFBundleName`, which the build script sets explicitly from `VITE_APP_NAME`. The icon comes from the generated `icon.icns` — make sure your `app-icon.png` (local) or `APP_ICON` CI secret is a valid 1024×1024+ PNG.

**Optional: Customize default timing values:**

```bash
VITE_DEFAULT_STARTS_IN=0.5        # Minutes before activation
VITE_DEFAULT_DISPLAY_OFF_IN=2     # Minutes before display off
VITE_DEFAULT_REQUIRE_PASS_IN=0    # Minutes until password required (0 = none)
VITE_DEFAULT_RUN_ON_BATTERY=false # Run on battery power?
VITE_DEFAULT_DEBUG=false          # Enable debug mode?
VITE_DEFAULT_AUTOSTART=true       # Register as OS login item on first install?
```

> **Autostart:** the OS login item — not `options.json` — is the source of truth for "Start at Login". The `VITE_DEFAULT_AUTOSTART` default applies **once**, on first install (release builds only; `tauri dev` never auto-registers the debug binary). Afterwards users toggle it in the options window, and changes made directly in System Settings are picked up on the next launch.

### 4. Replace the App Icon

Place an `app-icon.png` (minimum 1024×1024 px) in the project root. For local builds, `bun run tauri:build` now generates the full `src-tauri/icons/*` set automatically from that source file. For release builds, the icon is injected via the `APP_ICON` repository secret (see [Releases (CI/CD)](#releases-cicd)).

### 5. Edit `package.json` (Optional)

Update the package name for your fork:

```json
{
  "name": "your-app-name",
  "version": "1.0.0"
}
```

### 6. Build

```bash
# Install dependencies
bun install

# Development (hot reload) — generates merge-patch from .env, then runs tauri dev
bun run tauri:dev

# Production build (the script applies --env-file internally; preserves multi-line values like VITE_UPDATER_PUBKEY)
bun run tauri:build
```

## Releases (CI/CD)

Releases are cut with one command. `bun run tauri:release` computes the next version (from the latest tag), tags `vX.Y.Z`, and pushes the tag — **no version-bump commit is pushed to `main`**, so a fork's `main` stays a clean fast-forward of upstream. The tag triggers `.github/workflows/release.yml`, which stamps the version from the tag into `package.json` / `src-tauri/Cargo.toml` / `src-tauri/Cargo.lock` *on the runner*, then builds bundles for **macOS** (universal `.dmg`), **Windows** (`.msi`/`.exe`), and **Linux** (`.deb`/`.rpm`/AppImage), signs the updater artifacts, and publishes everything — including the `latest.json` manifest the auto-updater consumes — to a **draft** GitHub release.

The release config (URLs, updater pubkey, branding) is intentionally **not committed** — CI reads it from the `RELEASE_ENV` repository secret and stamps the version from the tag, so nothing fork-specific is baked into git history. The committed version files are stamped in CI too (not bumped locally), which is why cutting a release no longer creates a fork-local commit and a fork's `main` can mirror upstream cleanly.

### One-Time Setup

Requires the [GitHub CLI](https://cli.github.com/) (`gh`) authenticated against your repository.

1. **Generate the updater keypair** and paste the `.pub` file's contents into your local `.env`'s `VITE_UPDATER_PUBKEY`:

   ```bash
   bun tauri signer generate -w ~/.tauri/liminal-screen.key
   ```

2. **Upload your `.env` as the release config secret** (CI writes it to `.env` on the runner):

   ```bash
   gh secret set RELEASE_ENV < .env
   ```

3. **Upload the updater signing key** (the private half of the keypair from step 1, plus the password you chose when generating it):

   ```bash
   gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/liminal-screen.key
   gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
   ```

4. **Upload your app icon** (required because icon files are gitignored so forks can provide their own). It must be a 1024×1024+ PNG, base64-encoded:

   ```bash
   base64 < app-icon.png | tr -d '\n' | gh secret set APP_ICON
   ```

   CI decodes the secret to `app-icon.png` and runs `bun tauri icon` to generate the full `src-tauri/icons/*` set before the build.

### Optional: macOS Code Signing & Notarization

This is **optional** and only useful if you have a paid Apple Developer Program membership. Forks without one keep producing an unsigned macOS build exactly as before — leaving the secrets below unset is the default and is safe.

You will need a **Developer ID Application** certificate (not an "Apple Development" or "Mac App Distribution" certificate, which are for Xcode/App Store builds).

1. **Export the certificate.** In Keychain Access, select your Developer ID Application certificate and its private key, export as a `.p12`, and set a password. Then base64-encode it:

   ```bash
   base64 -i DeveloperIDApplication.p12 | pbcopy
   ```

2. **Create an App Store Connect API key.** Go to [App Store Connect](https://appstoreconnect.apple.com) → **Users and Access** → **Integrations** → **App Store Connect API** → **Team Keys**, click **+**, name it (e.g. `notarization-ci`), and give it the **Developer** role — that is sufficient for notarization; do not grant Admin. Then:
   - Download the `AuthKey_XXXXXXXXXX.p8`. **Apple lets you download it exactly once** — if you lose it, revoke the key and make a new one. (The download link may only appear after reloading the page.)
   - Copy the **Key ID** (the 10-character string in the row).
   - Copy the **Issuer ID** (the UUID shown above the keys table).
   - Base64-encode the key file:
     ```bash
     base64 -i AuthKey_XXXXXXXXXX.p8 | pbcopy
     ```

3. **Set the six repository secrets:**

   ```bash
   gh secret set APPLE_CERTIFICATE           # base64 from step 1
   gh secret set APPLE_CERTIFICATE_PASSWORD  # the .p12 password from step 1
   gh secret set APPLE_SIGNING_IDENTITY      # e.g. "Developer ID Application: Your Name (TEAMID)"
   gh secret set APPLE_API_ISSUER            # Issuer ID (UUID) from step 2
   gh secret set APPLE_API_KEY               # Key ID (10 chars) from step 2
   gh secret set APPLE_API_KEY_P8            # base64 of the .p8 from step 2
   ```

   Tip: `gh secret set APPLE_API_KEY_P8 < <(base64 -i AuthKey_XXXXXXXXXX.p8)` avoids putting the key through the clipboard.

   To find your signing identity string: `security find-identity -v -p codesigning | grep "Developer ID Application"`.

4. **Store the `.p8` somewhere safe** (a password manager) and delete it from `~/Downloads` — it is not recoverable from Apple.

5. **All-or-nothing:** setting only some of the six secrets fails the release workflow immediately with a clear error, rather than partway through a 10+ minute build.

6. **Revocation:** if CI is compromised, revoke the key in App Store Connect → Integrations. No personal Apple ID password is involved in this flow.

GitHub-hosted runners wipe `$RUNNER_TEMP` between jobs, but the workflow also explicitly deletes the materialized `.p8` in an `always()` cleanup step — which matters most if you ever move to self-hosted runners.

### Cutting a Release

```bash
bun run tauri:release          # patch bump (0.1.0 -> 0.1.1)
bun run tauri:release minor    # 0.1.0 -> 0.2.0
bun run tauri:release major    # 0.1.0 -> 1.0.0
bun run tauri:release 1.2.3    # explicit version
```

The script refuses to run off `main`, on a dirty working tree, behind `origin`, or on an already-existing tag. Watch the build under the repository's **Actions** tab (roughly 15–30 minutes across the three runners).

When the build finishes, **review the draft release and publish it manually**. Publishing is the go-live moment: the updater endpoint points at `releases/latest/download/latest.json`, so every installed app picks up the new version as soon as the release is public.

> **Do not tick "Set as a pre-release" when publishing.** GitHub's `releases/latest` endpoint only resolves the newest full release — a pre-release is never "latest", so the updater endpoint 404s. If a release was published as a pre-release by mistake, fix it with `gh release edit vX.Y.Z --prerelease=false --latest`.

### Keeping the Config in Sync

- **Version bumps** need no secret changes and create no commit on `main` — CI stamps `VITE_APP_VERSION` and the committed version files from the tag.
- **URL or branding changes**: update your local `.env`, then re-run `gh secret set RELEASE_ENV < .env`.
- **Icon changes**: re-run `gh secret set APP_ICON < <(base64 -w 0 app-icon.png)`.
- **Code signing**: by default builds are not notarized (macOS) or Authenticode-signed (Windows) — users see the usual Gatekeeper/SmartScreen warnings. You can opt into signed + notarized macOS builds by setting the optional `APPLE_*` repository secrets described above; Windows certificates can be added later without structural changes.

## Configuration Behavior

### Persistent Storage

User preferences (timing values like `startsIn`, `displayOffIn`, etc.) are saved to `options.json` in the app's data directory. These persist across app restarts and updates.

**Priority order:**
1. **User-saved values** from `options.json` (highest priority)
2. **`.env` defaults** (used on first install or after factory reset)
3. **Hardcoded fallbacks** (if `.env` values aren't set)

**What's persisted:** Timing values (`startsIn`, `displayOffIn`, `requirePassIn`), `runOnBattery`, `debug`, `instanceId`

**What's NOT persisted:** URLs (`saver_url`, `saver_url_debug`, `options_url`) — these always come from `.env` so forks can update URLs without affecting user preferences.

### Factory Reset

Users can reset to `.env` defaults via the UI (Reset button) or by deleting `options.json` from the app's data directory.

Factory reset does two things:
1. Clears `options.json` (Tauri store) and regenerates `instanceId`
2. Resets in-memory state to `.env` defaults

**Browser storage:** Remote pages (screensaver, options) may have written data to `localStorage`, the Cache API, or registered service workers. These are not cleared from the native side. Instead, every remote window has `navigator.id` injected at document-start (set to the current `instanceId`). A page that stores the last-seen ID can detect a mismatch on load and self-clean — the changed `navigator.id` after reset is the signal. See `.hermes/plans/native-storage-cleanup/RETHINK.md` for the concept and implementation notes.

# Development mode
`bun run tauri:dev`

# Production build 
(the script applies --env-file internally; preserves multi-line values like VITE_UPDATER_PUBKEY)

`bun run tauri:build`

## Architecture

### Frontend (`src/`)

Minimal, reactive UI — no framework. Uses a lightweight `Signal` class for state management and reactive effects.

- `main.ts` — Application entry point: initialization, reactive effects, form handling, app identity (`setIdentity`), dialog interactions via `tauri-plugin-dialog`
- `app/reactive.ts` — Generic `Signal<T>` class with `.derive()` and `.effect()` for reactive data flow
- `app/types.ts` — `AppOptions` TypeScript type mirroring the Rust struct
- `app/preview/preview.ts` — Preview window creation and management
- `app/power-monitor/power-monitor.ts` — Bridge to Rust idle-time detection
- `styles.css` — Application styles

### Backend (`src-tauri/src/`)

The Rust backend is the engine — it handles all screensaver lifecycle, window management, power monitoring, and persistence.

- `main.rs` — App entry, Tauri plugin registration (store, dialog, opener)
- `lib.rs` — Core setup: window creation, system tray with dynamic tooltip (from `VITE_APP_NAME`), options CRUD, screensaver engine orchestration, `factory_reset_options` command, `build_init_script` (injects `navigator.id`, a `LiminalScreen/{version} ({appName})` suffix on `navigator.userAgent`/`navigator.appVersion`, and the frozen `navigator.liminalScreen` options object into all remote windows at document-start), and `park_webview_window` (window pooling — see below)
- `screensaver_engine.rs` — Screensaver state machine: monitors idle time, shows/parks fullscreen windows on activation/deactivation, manages multi-display layout
- `display_manager.rs` — Monitor detection and logical coordinate calculation for multi-monitor fullscreen positioning
- `power_monitor.rs` — Platform-specific idle time detection (macOS IOKit, Windows `GetLastInputInfo`, Linux systemd-inhibit + X11 screensaver queries)
- `autoplay_media.rs` — Per-window autoplay permission configuration for WKWebView (macOS) and WebView2 (Windows)
- `speech.rs` + `speech_polyfill.js` — `speechSynthesis` fallback for Linux (WebKitGTK ships no Web Speech API): a JS shim injected into saver/preview windows forwards `speak`/`cancel` to `spd-say` via Tauri commands; inert on macOS/Windows where the native API exists

### Shared Library (`packages/liminal-api/`)

Reusable SDK for fork developers who host their own remote options page, published to npm as [`@liminal-screen/api`](https://www.npmjs.com/package/@liminal-screen/api). Works via `__TAURI__` globals (no npm install required — a CDN `<script>` is enough).

- `src/index.ts` — `LiminalAPI` class: `getOptions`, `setOptions`, `resetOptions`, `previewScreensaver`, `getVersion`, OS-screensaver conflict handling, updater commands, `startAutoSync`, `ask`, `showMessage`
- `src/store.ts` — `createOptionsStore` — signal-based reactive state, synced from backend `options-updated` events
- `src/reactive.ts` — Lightweight `Signal<T>` for remote options page
- `src/types.ts` — `AppOptions`, `SetOptionsPayload`, `CustomOptions`, `OsScreensaverStatus`, `UpdateInfo` types (mirror the Rust structs)
- `src/global.ts` — IIFE/CDN entry point; attaches the public surface to `globalThis.LiminalAPI`
- `docs/` — API spec, integration guide, security/trust model (shipped in the npm tarball)
- `examples/remote-options/` — Reference options page (HTML + JS + service worker) ready to deploy

### Build Scripts (`scripts/`)

- `build-tauri-config.ts` — Reads `.env` and `src-tauri/tauri.conf.json`, then writes a Tauri merge-patch to `src-tauri/.tauri-runtime.conf.json` (gitignored) that overrides `productName`, `version`, `identifier`, the main window `title`, bundle `shortDescription`/`longDescription`, and updater `pubkey`/`endpoints` from env vars. Invoked automatically by the `tauri:dev` / `tauri:build` npm scripts, which pass the generated file to `tauri` via `--config`. Handles multi-line values (PEM keys). Forks never need to edit `tauri.conf.json` — only `.env`.
- `release.ts` — One-command release (`bun run tauri:release [patch|minor|major|x.y.z]`): verifies the tree is clean and `main` is current, bumps the version in `package.json` and the local `.env`, commits, tags `vX.Y.Z`, and pushes. The tag triggers the CI release pipeline — see [Releases (CI/CD)](#releases-cicd).

### Configuration Layers

| Layer | File | Purpose |
|-------|------|---------|
| Build-time identity | `tauri.conf.json` | Structural config (build commands, dev URL, window shape, CSP, bundle icons/category, updater install mode) + obvious placeholders for per-fork fields. Forks do NOT edit this file — see `_DO_NOT_EDIT` note in AGENT.md §7.1 for why this file can't carry a documentation field. |
| Build-time overrides | `src-tauri/.tauri-runtime.conf.json` | Gitignored; generated by `scripts/build-tauri-config.ts` from `.env`; passed to Tauri via `--config` by the `tauri:dev` / `tauri:build` npm scripts. Overrides the placeholders with real per-fork values. |
| Runtime identity | `.env` | `VITE_APP_NAME`, `VITE_APP_DESCRIPTION` — read by Rust backend and forwarded to frontend via `AppOptions` |
| Runtime URLs | `.env` | `VITE_SAVER_URL`, `VITE_SAVER_URL_DEBUG`, `VITE_OPTIONS_URL` |
| Runtime defaults | `.env` | `VITE_DEFAULT_STARTS_IN`, etc. — fallback values for first install |
| User preferences | `options.json` | User's saved timing settings (auto-created, persisted across updates) |

## Technical Details

### Window Pooling

Webviews are never destroyed. wry deliberately over-retains the `WKWebView` on drop (`Drop for InnerWebView` calls `webview.retain()` to avoid a use-after-free), so a destroyed webview and its WebKit helper processes are never released — every create/destroy cycle leaked another set, and memory climbed with each activation.

So each window that can open more than once — one saver per display, plus preview and options — is created at most once and then **parked** instead of closed: media stopped, navigated to `about:blank`, hidden. The next activation re-navigates and re-shows the same webview. Baseline memory is a fixed cost rather than a leak; the helper processes stay in Activity Monitor, but their count no longer grows.

Because a pooled webview keeps the `initialization_script` it was built with — and there is no way to replace that on a live webview — the options snapshot baked in at creation would go stale as soon as the user saved a setting. Each navigation therefore carries the current options in the URL fragment (`#__liminal=…`), which the init script prefers over the baked snapshot and strips (restoring any fragment the URL already had) before the page's own scripts run. The fragment is used rather than a query parameter because fragments are not sent to the server.

### Multi-Monitor Fullscreen

On macOS the saver windows are **not** put into native fullscreen. Native fullscreen gives each window its own Space, which costs an animation each way, permits only one transition at a time, and leaves `hide()` unreliable until it settles — all of which fight window pooling. Instead the saver covers the screen as an overlay, which needs three things together. Each was confirmed necessary by bisection; none substitutes for another:

1. **Accessory activation policy** (`setup_app`). This is the load-bearing one. A Regular (Dock-visible) app is a full participant in activation, so showing a window is an *activation request* — and macOS answers that from inside another app's full-screen Space by switching Spaces or refusing, neither of which puts a saver on screen. Accessory apps float over the active Space instead of competing for it. The cost is no Dock icon and no Cmd-Tab entry, which suits a tray app. `orderFrontRegardless` is **not** a substitute (verified), and is deliberately not used: the plain `show()` leaves the window key, so dismissing keystrokes are swallowed rather than delivered to whatever is underneath.
2. **`canJoinAllSpaces | fullScreenAuxiliary`** collection behavior. `fullScreenAuxiliary` is load-bearing — without it the saver is confined to the desktop Space. Do **not** use `fullScreenNone`: it is the adjacent bit with the opposite meaning (opt out of full-screen entirely), and it produces a saver that runs, plays audio, and reports itself visible with correct bounds while being invisible from inside any full-screen app.
3. **`NSScreenSaverWindowLevel`** (1000) for the window level. Note that a level only orders windows *within* a Space — it does nothing to get the saver into a full-screen Space, which is entirely the job of (1) and (2). Once the saver is in that Space, the full-screen app's own window is at `NSNormalWindowLevel` (0), so 1000 is already far above it. Resist going higher: `CGShieldingWindowLevel()` (2147483628 on macOS 26) also outranks `kCGAssistiveTechHighWindowLevel` (1500) and would occlude VoiceOver, Switch Control and Zoom.

`LIMINAL_SAVER_LEVEL` and `LIMINAL_SAVER_BEHAVIOR` override the last two at runtime (decimal or `0x` hex) for diagnosing display problems without a rebuild. Every activation logs a `SAVER CONFIG …` line with the values actually in effect, including the real `[NSApp activationPolicy]`.

Other platforms still use native fullscreen, staggered by 600 ms, since some window managers handle only one transition at a time.

### Screen-edge hairline (macOS, known)

A ~1px light line can appear at the very edge of the saver on macOS. It is **not** a geometry bug — the window covers the screen exactly and the webview covers the window exactly (the `macOS … frames:` log line reports all three rects; when they match, geometry is ruled out). It is WebKit's opaque white base layer showing through wherever the page doesn't paint the last device pixel.

There are two ways to remove it, and only one of them is acceptable here:

- **Fix it in the saver page (recommended).** Give `html, body` an opaque background — `html, body { margin: 0; background: #000; }`. The white base is only visible where nothing is painted over it, so an opaque page background hides it regardless of whether the page's *content* reaches the edge. This is the real fix and it costs nothing.
- **Enable `macos-private-api` (rejected).** That switches on `wry/transparent`, which turns off the base layer via a private `drawsBackground` KVC key. It works, but it is a private API and therefore a blocker for publishing, so this project deliberately does not enable it. Don't add it to the `tauri` features.

Note that the window's `background_color` cannot fix this: the webview covers 100% of the window, so the window colour is never visible at the edge. It is set anyway to avoid a white flash before the page's first paint.

### Audio Playback

The app uses a layered approach to stop audio cleanly:
1. JavaScript mute + pause (stops media elements)
2. Platform-native `stopLoading` (kills WebKit pipeline)
3. Navigation to `about:blank` (tears down the page and its media)
4. Window hidden and kept for reuse (see Window Pooling)

### Autoplay Configuration

On macOS, autoplay must be configured BEFORE any content loads. The app creates windows with `about:blank`, configures autoplay permissions, then navigates to the real URL.

### Speech Synthesis on Linux

WebKitGTK does not implement `window.speechSynthesis`, so saver content that speaks text would be silent on Linux. The app injects a Web Speech API polyfill into saver and preview windows that forwards utterances to `spd-say`. **Linux users need `speech-dispatcher` installed** (provides the `spd-say` binary; preinstalled on many desktop distributions):

```bash
# Debian/Ubuntu
sudo apt install speech-dispatcher
# Fedora
sudo dnf install speech-dispatcher-utils
# Arch
sudo pacman -S speech-dispatcher
```

Without it, speech is skipped gracefully (utterances fire `error` events). macOS and Windows use their webviews' native speech synthesis — the polyfill steps aside there.

### GDK Backend on Linux

The app defaults to `GDK_BACKEND=x11` on Linux (set in `src-tauri/src/main.rs`, before any GTK/webkit2gtk init) unless the user has already set `GDK_BACKEND` themselves. WebKitGTK has a history of rendering and DMA-BUF bugs under native Wayland, so on Wayland sessions the app runs through XWayland instead. This costs native Wayland niceties (fractional scaling, lower input latency) but avoids a class of crashes/rendering glitches that are otherwise common for WebKitGTK apps on Wayland today. It doesn't affect idle detection, which already branches on `WAYLAND_DISPLAY`/session type independently (see `power_monitor.rs`).

This is a workaround, not a permanent stance — revisit once wry/WebKitGTK's native Wayland support matures. To opt back into Wayland, set `GDK_BACKEND=wayland` in the environment before launching the app.

## License

Licensed under the [Apache License 2.0](LICENSE).

## Credits

Built with [Tauri v2](https://tauri.app/)

Original project by [tomaszatoo](https://github.com/tomaszatoo)
