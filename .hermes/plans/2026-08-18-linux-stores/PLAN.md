# Plan: Distribute Liminal Screen via Flathub and Snap Store

**Date:** 2026-08-18  
**Status:** Draft  

---

## Goal

Improve Linux user trust and discoverability by publishing the app through two curated distribution channels that provide automatic signature verification and clean storefront metadata:

1. **Flathub** (Flatpak)
2. **Snap Store** (snap)

A GitHub-released `.deb` / `.AppImage` is unsigned in practice and will always look "unverified" in App Center / GNOME Software. Publishing to Flathub / Snap Store is the only way to get the OS-level trust UX users expect.

---

## Constraints

- The app is a **tray-only / background** screensaver. It needs Wayland/X11, D-Bus idle/screen-saver access, notification access, and tray icon support.
- It relies on **WebKitGTK 4.1**, so the Flatpak runtime / snap base must ship a compatible version or we must bundle it.
- **macOS / Windows releases stay unchanged** — this plan adds Linux-specific build targets, not replacements.
- Store publication is **manual per release** at first; CI automation is Phase 2.

---

## Phase 1: Flatpak / Flathub

### 1.1 Create AppStream MetaInfo

Path: `src-tauri/flatpak/<identifier>.metainfo.xml`

Use the **reverse-domain ID** from `VITE_APP_IDENTIFIER` (e.g. `gallery.screensaver.liminal-screen`). This file feeds the Flathub listing and GNOME Software / KDE Discover.

Required fields:
- `<id>` — same as `VITE_APP_IDENTIFIER`
- `<name>` — `VITE_APP_NAME`
- `<developer><name>` — `VITE_APP_PUBLISHER`
- `<summary>` — short description
- `<description>` — longer description
- `<url type="homepage">`
- `<content_rating type="oars-1.1">`
- `<releases>` — must be updated every release
- `<update_contact>`
- `<branding>` with primary light/dark colors
- `<screenshots>`

### 1.2 Add Metainfo to the Debian bundle

In `src-tauri/tauri.conf.json` under `bundle.linux.deb.files`:

```json
"/usr/share/metainfo/<identifier>.metainfo.xml": "flatpak/<identifier>.metainfo.xml"
```

This ensures the `.deb` also carries the metadata, which is required/recommended by the Tauri docs and also helps GNOME Software / KDE Discover when the user installs the `.deb` directly.

### 1.3 Create Flatpak manifest

Path: `flatpak/<identifier>.yml`

Approach: **download the pre-built `.deb` from GitHub releases**, then extract it inside the Flatpak build. This avoids rebuilding Rust/WebKit inside Flatpak, which is slow and fragile. The Flatpak sandbox only needs:

- `org.gnome.Platform` runtime
- `.deb` download URL + sha256 per arch
- `.metainfo.xml` copy
- desktop file / icon install

This is the pattern Tauri now documents under "Distribute > Flathub > Prerequisites" as the recommended way for existing `.deb` releases.

Required `finish-args` for a screensaver app (derived from `power_monitor.rs` and `notification_service.rs`):

```yaml
finish-args:
  # Windowing / display
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri
  # Tray icon
  - --talk-name=org.kde.StatusNotifierWatcher
  # Idle detection (GNOME/Mutter on X11 and Wayland)
  - --talk-name=org.gnome.Mutter.IdleMonitor
  # Screen lock + idle time (KDE and other desktops via freedesktop spec)
  - --talk-name=org.freedesktop.ScreenSaver
  # Display blank / DPMS control on GNOME
  - --talk-name=org.gnome.Mutter.DisplayConfig
  # Notifications
  - --talk-name=org.freedesktop.Notifications
  # Sleep inhibition (systemd-logind)
  - --system-talk-name=org.freedesktop.login1
  # Audio for saver/preview media
  - --socket=pulseaudio
  # Fetch screensaver content and notification feed
  - --share=network
  # Wayland black-webview workaround, optional
  - --env=WEBKIT_DISABLE_COMPOSITING_MODE=1
```

Tray icon in Flatpak: use Tauri API to change the tray temp dir, or grant `--filesystem=xdg-run/tray-icon:create`. Prefer the `temp_dir_path` API if it works in Flatpak.

### 1.4 Add Flatpak build script

`scripts/build-flatpak.ts`:
- Read `.env`
- Download the target `.deb` for each arch
- Compute / verify sha256
- Emit the final manifest with real URLs/versions
- Invoke `flatpak-builder` locally (optional — primarily used to test before Flathub PR)

### 1.5 Add CI job for local Flatpak testing

New matrix entry in `.github/workflows/release.yml`? Not in Phase 1. Keep local manual for now to avoid blocking release pipeline.

### 1.6 Submit to Flathub

- Fork `github.com/flathub/flathub`
- Create branch off `new-pr`
- Add `flatpak/<identifier>.yml`, `.metainfo.xml`, maybe flathub.json
- Open PR against `new-pr`
- Respond to review; after merge, Flathub signs and publishes

---

## Phase 2: Snap Store

### 2.1 Create `snap/snapcraft.yaml`

Path: `snap/snapcraft.yaml`

Use the same approach as Flatpak: dump the pre-built `.deb` instead of rebuilding inside snapcraft. This requires `build-packages` / `stage-packages` plus `override-build` that runs `dpkg -x` and fixes up the `.desktop` icon path.

Snap-specific concerns:
- `confinement: strict` is required for Snap Store.
- Use the `gnome` extension to get WebKitGTK / GTK / Wayland / X11 plugs automatically.
- Tray icons work under the gnome extension, but we must test indicator visibility on Ubuntu.
- The app name registered in the Snap Store must match `name:` in the YAML.

### 2.2 Local snap build & test

```bash
sudo snap install core22
sudo snap install snapcraft --classic
snapcraft
snap install --dangerous ./liminal-screen_*.snap
snap run liminal-screen
```

### 2.3 Register and publish

- Register the app name at https://snapcraft.io/register-snap
- `snapcraft login`
- `snapcraft upload --release=stable ./liminal-screen_*.snap`

---

## Phase 3: CI / Release Integration (Future)

After the manual store publishing workflow is proven, add CI steps:

1. Release workflow builds `.deb` as today.
2. New job generates Flatpak manifest from the just-uploaded release `.deb` URL and sha256, builds the `.flatpak` bundle, and opens / updates a Flathub PR via the flathubbot pattern.
3. New job builds `.snap` from the `.deb` and uploads to the Snap Store.

This is non-trivial because both stores require human review/PR steps. Snap can be fully automated after name registration; Flathub still needs a PR.

---

## Files to Add

| Path | Purpose |
|---|---|
| `src-tauri/flatpak/<identifier>.metainfo.xml` | AppStream metadata for Flathub + Debian |
| `flatpak/<identifier>.yml` | Flatpak manifest (downloads the release `.deb`) |
| `scripts/build-flatpak.ts` | Generate final manifest + local build helper |
| `snap/snapcraft.yaml` | Snap package definition |
| `.github/workflows/release.yml` (later) | Add Flatpak/snap CI jobs |

---

## Files to Modify

| File | Change |
|---|---|
| `src-tauri/tauri.conf.json` | Add metainfo to `bundle.linux.deb.files` |
| `README.md` | Document Linux store installs |
| `TODO.md` | Add tracking item |

---

## Open Questions

1. Should the Flatpak/snap builds reuse the GitHub-release `.deb`, or do we want a fully reproducible source build in Flathub?  
   **Recommendation:** Reuse `.deb`. Faster to ship, less CI complexity, and the app already builds reproducibly via GitHub Actions. Source builds in Flatpak require vendoring npm + cargo, which is fragile.

2. **What exact D-Bus names does the app need for idle detection, screen lock, display blank, notifications, and sleep inhibition?**
   - Audit result:
     - Idle: `org.gnome.Mutter.IdleMonitor` (GNOME Wayland/X11), `org.freedesktop.ScreenSaver` (KDE idle time + lock)
     - Lock: `loginctl`, `org.freedesktop.ScreenSaver.Lock`, `xdg-screensaver`, `gnome-screensaver-command`
     - Blank: `org.gnome.Mutter.DisplayConfig` (GNOME), `kscreen-doctor`, `xset`
     - Sleep inhibition: `systemd-inhibit` → `org.freedesktop.login1`
     - Notifications: `notify-rust` → `org.freedesktop.Notifications`
   - These are all declared in the Flatpak finish-args; `systemd-inhibit`, `loginctl`, `gsettings`, `xset`, and `kscreen-doctor` must be exposed via PATH from the host or replaced with D-Bus-only fallbacks where possible.

3. **Does the tray icon work inside Flatpak / snap without extra filesystem holes?**
   Need runtime testing on Ubuntu 24.04+ (Wayland).

4. Is the `VITE_APP_IDENTIFIER` a valid Flatpak app ID?  
   Flatpak IDs must be reverse-domain with at least one dot. `gallery.screensaver.liminal-screen` is okay; `com.example.app` style is preferred. We should not change the identifier mid-lifetime, so use the existing one.

---

## Success Criteria

- [ ] App accepted and published on Flathub.
- [ ] App accepted and published on Snap Store.
- [ ] Linux users can install via `flatpak install flathub <id>` or `snap install <name>`.
- [ ] Storefronts show app name, publisher, icon, and description correctly.
- [ ] Core features work under both sandboxed runtimes: idle detection, saver window, display off, lock, notifications, tray menu.
