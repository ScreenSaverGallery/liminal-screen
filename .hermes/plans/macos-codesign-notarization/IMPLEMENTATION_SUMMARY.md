# Implementation Summary: macOS Code Signing & Notarization

**Plan:** `./PLAN.md`
**Implemented:** 2026-08-16

---

## What changed

Implemented optional, fork-configurable macOS Developer ID signing + notarization using an App Store Connect API key.

### `.github/workflows/release.yml`

- Extended the header comment block with the optional `APPLE_*` repository secrets and the all-or-nothing rule.
- Added a macOS-only guard step that:
  - Prints an informational message when no secrets are set.
  - Prints a success message when all six are set.
  - Fails fast with `::error::` when only some are set, listing the missing secrets.
- Added a macOS-only `.p8` materialization step that:
  - No-ops when `APPLE_API_KEY_P8` is absent.
  - Decodes the base64 secret under `$RUNNER_TEMP/appstoreconnect/AuthKey_<KeyID>.p8`.
  - Sets permissions `700` on the directory and `600` on the key file.
  - Validates the decoded file contains a PEM private key.
  - Exports `APPLE_API_KEY_PATH` to `$GITHUB_ENV` for the build step.
- Added a macOS-only cleanup step with `if: always()` that removes the materialized `.p8` even if the build fails.
- Passed the six `APPLE_*` secrets (but not `APPLE_API_KEY_PATH`, which is derived) into the existing `tauri-apps/tauri-action@v0` build step.

No changes to `src-tauri/tauri.conf.json` — Developer ID identity and notarization credentials are resolved from env vars by `tauri-cli`/`tauri-action`, consistent with the existing fork-agnostic placeholder design.

### `README.md`

- Updated the "Code signing" bullet under **Keeping the Config in Sync** to note the optional macOS signing path and unchanged Windows status.
- Added a new **Optional: macOS Code Signing & Notarization** subsection under **Releases (CI/CD)** with:
  - Prerequisites (Apple Developer Program + Developer ID Application certificate).
  - `.p12` export + base64 encoding instructions.
  - App Store Connect API key creation instructions (Developer role, one-time `.p8` download, Issuer/Key IDs).
  - The six `gh secret set` commands.
  - Security notes: store the `.p8` safely, all-or-nothing validation, revocation path, self-hosted runner cleanup note.

### `AGENT.md`

- Added a one-line note in §7.1 explaining that the `APPLE_*` secrets are CI-only, optional, and never written to `.env` or baked into the binary.

### `PLAN.md`

- Status updated from `Draft` to `Implemented`.

---

## What was not changed

- No `src-tauri/entitlements.plist` was added. Per the plan, hardened-runtime entitlements are only needed if Phase 3 manual validation shows the Accessibility-based lock feature (AppleScript/System Events) regresses under hardened runtime. That empirical check has not been done yet.
- No support for Apple ID / app-specific-password notarization auth was added. The implementation uses App Store Connect API key auth only, as decided in the plan.

---

## Verification status

Static checks performed during implementation:

- Workflow YAML indentation and structure reviewed.
- Guard and materialize shell scripts passed `bash -n` syntax checks.
- No syntax or schema-affecting changes to `tauri.conf.json`.

Manual validation still required (requires an Apple Developer Program membership and a test release):

- [ ] No `APPLE_*` secrets set → unsigned macOS build, no regression.
- [ ] All six secrets set → signed + notarized + stapled macOS bundle.
- [ ] Partial secret set → guard step fails fast with clear message.
- [ ] Malformed `APPLE_API_KEY_P8` → materialization step fails with PEM error.
- [ ] `.p8` never appears in the checkout or uploaded bundle.
- [ ] Accessibility-based lock still works in signed+notarized build.

---

## Follow-up work

1. Run the manual validation checklist above on a fork that owns the necessary Apple Developer credentials.
2. If the lock feature regresses under hardened runtime, add `src-tauri/entitlements.plist` with `com.apple.security.automation.apple-events` and reference it via `bundle.macOS.entitlements` in `tauri.conf.json` (or the generated merge-patch).
3. Open the upstream PR against `ScreenSaverGallery/liminal-screen`.
