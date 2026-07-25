# Liminal Screen API Security

## Overview

`@liminal-screen/api` is a thin, unauthenticated bridge over Tauri IPC. It holds
no secrets and performs no authentication of its own — every trust decision is
made by the Rust backend and by the fork that decides which URL to load.

> **Note:** earlier drafts of this document described a shared-secret
> authentication layer (`configureSecurity()`, `generateAuthToken()`). That
> layer was removed; those methods do not exist. Authentication, if a fork
> needs it, belongs at the fork level — see [Adding your own auth](#adding-your-own-auth).

## Trust model

The options page is **remote content running inside your application**. Whoever
controls `VITE_OPTIONS_URL` can call every command this library exposes.

What the library can do:

- read and write user options (`getOptions`, `setOptions`, `resetOptions`)
- trigger a screensaver preview
- read the app version
- read, disable, and restore the **OS-native** screensaver setting
- check for and install application updates

What it cannot do:

- run arbitrary commands — only the commands registered in
  `tauri::generate_handler![...]` are reachable
- read or write arbitrary files, spawn processes, or access the network
  outside the webview's normal browser sandbox
- change identity fields — `saverUrl`, `saverUrlDebug`, `optionsUrl`,
  `appName`, `appDescription`, `notificationUrl`,
  `notificationCheckIntervalSecs` and `instanceId` come from the fork's `.env`
  and the backend ignores user-submitted values for them

## Backend guarantees

1. **Allow-listed commands.** The webview can only invoke commands the app
   explicitly registers. Everything else fails at the IPC boundary.
2. **Server-side validation.** `set_options` validates timing values (e.g.
   `startsIn` minimum) and rejects out-of-range input — client-side validation
   is a convenience, not a control.
3. **Identity fields are read-only.** They are re-applied from `.env` on every
   write, so a hostile options page cannot repoint the screensaver URL.
4. **Capability-gated plugins.** `ask()` and `showMessage()` only reach native
   dialogs when the Tauri capability file grants `dialog:allow-ask` and
   `dialog:allow-message`. Without those permissions they fall back to
   `confirm()` / `alert()`.
5. **Opt-in notifications.** `notificationsEnabled` defaults to `false` and no
   notification is shown while it is false.
6. **Login item owned by the OS.** `autostart` is applied through the OS login
   item; the backend reports back what the OS accepted, which may differ from
   what was requested.

## Deployment recommendations

Because the options URL is the trust boundary, the meaningful controls are the
ones around hosting it:

1. **Serve over HTTPS.** Plain HTTP lets a network attacker replace your
   options page and thereby control the settings above.
2. **Pin the URL to a host you control.** Don't point `VITE_OPTIONS_URL` at
   third-party hosting you can't lock down, and avoid user-supplied redirects.
3. **Set a Content Security Policy** on the options page so injected content or
   a compromised dependency can't call the API on your users' behalf.
4. **Pin CDN dependencies.** If you load the library from a CDN, pin an exact
   version (`https://unpkg.com/@liminal-screen/api@0.2.0/dist/liminal-api.global.js`)
   rather than a floating tag, and consider Subresource Integrity.
5. **Review anything you inline.** Analytics snippets and tag managers on the
   options page run with the same IPC access as your own code.
6. **Treat `instanceId` as a pseudonymous identifier.** It is stable per
   installation until a factory reset. Don't ship it to third parties without
   telling your users.

## OS screensaver changes

`disableOsScreensaver()` mutates a system setting outside your app:

- **macOS** — `defaults -currentHost write com.apple.screensaver idleTime 0`
- **Windows** — `SystemParametersInfoW(SPI_SETSCREENSAVEACTIVE, FALSE)`
- **Linux (GNOME)** — `gsettings set org.gnome.desktop.session idle-delay 0`

The prior value is saved so `restoreOsScreensaver()` can undo it. **Always ask
before calling it** — `ask()` exists for exactly this — and surface a restore
affordance whenever `getSavedOsScreensaverIdle()` returns non-null. Silently
disabling a user's screensaver is the kind of behaviour that gets an app
uninstalled.

## Updates

`installUpdate()` downloads and installs a new application build, then
restarts. Update artifacts are verified against the updater's public key by the
Tauri updater plugin — the library only triggers the flow. Gate it behind an
explicit user action.

## Adding your own auth

If your deployment needs the options page itself to be restricted, do it at the
hosting layer rather than in the IPC bridge — the bridge runs on the user's
machine, so any secret shipped to it is readable by the user:

- put the page behind SSO / basic auth / an allow-listed network
- serve a per-installation URL and validate it server-side
- gate sensitive fork-specific fields behind your own backend, and keep
  `customOptions` to non-sensitive values

Anything embedded in the page is client-side and cannot be treated as a secret.

## Reporting a vulnerability

Report security issues via
<https://github.com/tomaszatoo/liminal-screen/issues>, or privately through the
repository's security advisory page if the issue is sensitive.
