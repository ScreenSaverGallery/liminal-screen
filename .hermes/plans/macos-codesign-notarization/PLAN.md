# Plan: macOS Code Signing & Notarization (Optional, Fork-Configurable)

**Created:** 2026-08-16
**Updated:** 2026-08-16 — switched notarization auth from Apple ID + app-specific password to an **App Store Connect API key**
**Status:** Implemented

---

## Problem / Context

`README.md` already flags the gap explicitly (line ~169):

> **Code signing**: builds are not notarized (macOS) or Authenticode-signed (Windows) — users see the usual Gatekeeper/SmartScreen warnings. Apple/Windows certificates can be added to the workflow later without structural changes.

This plan implements that "later," scoped to **macOS only**, following the official guide: https://v2.tauri.app/distribute/sign/macos/

Goal: let any fork that owns an Apple Developer Program membership opt into Developer ID signing + notarization by setting repository secrets, while a fork that does **not** set them gets the exact unsigned build CI produces today. The change must be safe enough that upstream (ScreenSaverGallery) can merge it as a PR without forcing every downstream fork to own a paid Apple account or touch their config.

**Notarization auth: App Store Connect API key.** Tauri supports two notarization credential sets, and this plan uses the API-key one (`APPLE_API_ISSUER` / `APPLE_API_KEY` / `APPLE_API_KEY_PATH`) rather than the Apple ID one (`APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`). Rationale:

- **No expiry / rotation churn.** App-specific passwords are tied to a human Apple ID; they break when that person changes their password, enables/disables 2FA devices, or leaves the org. API keys are team-scoped and live until explicitly revoked.
- **No personal credential in CI.** An app-specific password is a credential on a *person's* Apple ID; an API key is an org artifact that an Admin can revoke from App Store Connect without touching anyone's personal account.
- **Least privilege.** The key can be minted with the Developer role, scoped to notarization, instead of handing CI an authenticator for a full Apple ID.
- **Cost:** one extra moving part — the `.p8` private key is a *file*, not a string, so CI must materialize it from a base64 secret to a path before the build. That's a single extra workflow step (below).

---

## Current State

- `.github/workflows/release.yml` builds macOS (`universal-apple-darwin`), Windows, and Linux in one matrixed `build` job via `tauri-apps/tauri-action@v0`.
- No `APPLE_*` env vars are referenced anywhere in the workflow. tauri-cli's macOS signing step has no identity to resolve, so it falls back to an ad-hoc/unsigned `.app` + `.dmg` — today's actual behavior.
- Two secret-based patterns already exist and set the precedent to follow:
  - `RELEASE_ENV` — the fork's `.env` contents (identity, URLs, updater pubkey), written to `.env` on the runner. **This is the closest precedent for the `.p8` handling below**: a secret materialized into a file on the runner by a dedicated step.
  - `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` — minisign key for **updater artifact** signing (unrelated to macOS codesign; already optional-by-convention in the sense that a fork without updates configured just doesn't set `VITE_UPDATER_*`, but the CI secrets themselves are effectively required today since the step doesn't guard on their absence).
- `src-tauri/tauri.conf.json` has no `bundle.macOS.signingIdentity` or entitlements block. This is expected — Tauri resolves the Developer ID identity from the `APPLE_SIGNING_IDENTITY` env var at build time, not from the config file, so no structural config change is needed for baseline signing.
- `scripts/build-tauri-config.ts` / the `.env` merge-patch system only covers `VITE_*` app-identity vars — Apple signing credentials are a different concern (CI-runner-only, never baked into the app binary) and should NOT be routed through that system or through `RELEASE_ENV`.

---

## Design Constraints

1. **Optional by default.** Zero `APPLE_*` secrets set → workflow behaves exactly as today: unsigned `.dmg`/`.app`, no new required secret, no error.
2. **Fork-scoped.** Secrets live in the fork's own repo (Settings → Secrets and variables → Actions), never committed, never inherited from or shared with upstream.
3. **Fail fast on partial configuration.** If a fork sets some but not all of the required secrets, the workflow must error immediately with a clear message — not fail deep inside `tauri-cli`/`notarytool` with a cryptic codesign error after 10+ minutes of build time.
4. **No `tauri.conf.json` structural change required.** Signing identity + notarization creds are pure env vars consumed by `tauri-cli`/`tauri-action` during `tauri build`; this keeps the base config fork-agnostic per the existing placeholder pattern (AGENT.md §7.1).
5. **The `.p8` never touches the repo checkout.** Write it under `$RUNNER_TEMP` with `0600`, pass the absolute path via `APPLE_API_KEY_PATH`, and delete it in an `if: always()` cleanup step. Writing it into the working tree risks it being picked up by the bundler as a resource, showing up in `git status`, or being cached by `rust-cache` — none of which are acceptable for a private key.
6. **Match existing house style**: a header-comment block in `release.yml` documenting new secrets (like the existing `RELEASE_ENV`/`TAURI_SIGNING_PRIVATE_KEY` block), and a guard step in the same spirit as "Verify build environment."

---

## The Secret Set

Six repository secrets, all-or-nothing:

| Secret | What it is |
|--------|-----------|
| `APPLE_CERTIFICATE` | base64 of the **Developer ID Application** `.p12` (cert + private key) |
| `APPLE_CERTIFICATE_PASSWORD` | password set when exporting that `.p12` |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_API_ISSUER` | App Store Connect **Issuer ID** (UUID above the keys table) |
| `APPLE_API_KEY` | App Store Connect **Key ID** (10-char, e.g. `2X9R4HXF34`) |
| `APPLE_API_KEY_P8` | base64 of the downloaded `AuthKey_<KeyID>.p8` |

`APPLE_API_KEY_PATH` is **not** a secret — it's derived on the runner by the materialization step and exported to the build environment.

Note there is **no `APPLE_TEAM_ID` secret** in this design: `notarytool`'s API-key auth is `--key` / `--key-id` / `--issuer`, and the team is implied by the key. The team ID still appears inside `APPLE_SIGNING_IDENTITY`, which is where codesign needs it.

---

## Proposed Changes

### 1. `.github/workflows/release.yml`

**a) Extend the header comment** to document the new *optional* secrets alongside the existing required ones, making clear which is which, and noting the all-or-nothing rule.

**b) Add a guard step**, scoped to the `macos-latest` leg only, before the `.p8` materialization:

```yaml
      - name: Check macOS signing configuration
        if: matrix.platform == 'macos-latest'
        shell: bash
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_API_ISSUER: ${{ secrets.APPLE_API_ISSUER }}
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
          APPLE_API_KEY_P8: ${{ secrets.APPLE_API_KEY_P8 }}
        run: |
          required=(APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY \
                    APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_P8)
          set_count=0
          missing=()
          for var in "${required[@]}"; do
            if [ -n "${!var}" ]; then
              set_count=$((set_count + 1))
            else
              missing+=("$var")
            fi
          done

          if [ "$set_count" -eq 0 ]; then
            echo "No APPLE_* signing secrets set — building an unsigned macOS bundle (Gatekeeper will warn on install)."
          elif [ "$set_count" -eq "${#required[@]}" ]; then
            echo "All APPLE_* signing secrets present — building a signed + notarized macOS bundle."
          else
            echo "::error::Partial macOS signing configuration — set ALL of ${required[*]}, or none of them. Missing: ${missing[*]}" >&2
            exit 1
          fi
```

**c) Add a `.p8` materialization step**, macOS leg only, which no-ops when the secret is absent so unconfigured forks are unaffected:

```yaml
      - name: Materialize App Store Connect API key
        if: matrix.platform == 'macos-latest'
        shell: bash
        env:
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
          APPLE_API_KEY_P8: ${{ secrets.APPLE_API_KEY_P8 }}
        run: |
          if [ -z "$APPLE_API_KEY_P8" ]; then
            echo "No App Store Connect API key configured — skipping (unsigned build)."
            exit 0
          fi

          key_dir="$RUNNER_TEMP/appstoreconnect"
          mkdir -p "$key_dir"
          chmod 700 "$key_dir"
          key_path="$key_dir/AuthKey_${APPLE_API_KEY}.p8"

          # `base64 -D` is the macOS spelling; the runner is always macOS here.
          printf '%s' "$APPLE_API_KEY_P8" | base64 -D > "$key_path"
          chmod 600 "$key_path"

          if ! grep -q "BEGIN PRIVATE KEY" "$key_path"; then
            echo "::error::APPLE_API_KEY_P8 did not decode to a PEM private key — re-run: base64 -i AuthKey_XXXX.p8 | pbcopy" >&2
            exit 1
          fi

          echo "APPLE_API_KEY_PATH=$key_path" >> "$GITHUB_ENV"
          echo "App Store Connect API key written to \$RUNNER_TEMP (key id ${APPLE_API_KEY})."
```

The emptiness check lives **inside the script**, not in `if:`, on purpose: GitHub does not expose the `secrets` context to `jobs.<id>.steps.<id>.if` (available there: `github, needs, strategy, matrix, job, runner, env, vars, steps, inputs`), so `if: secrets.APPLE_API_KEY_P8 != ''` is a workflow-parse error, not a false condition. The alternative — hoisting the secret into a job-level `env:` and testing `env.APPLE_API_KEY_P8 != ''` — works, but puts the key material in the environment of *every* step in the job including third-party actions, which is a worse trade than one `exit 0`.

Two more details worth keeping:

- **The filename is `AuthKey_<KeyID>.p8`, not an arbitrary name.** Tauri's docs say `APPLE_API_KEY_PATH` is authoritative, but for backward compatibility the bundler still falls back to the `altool` lookup (`./private_keys`, `~/private_keys`, `~/.private_keys`, `~/.appstoreconnect/private_keys`, filename `AuthKey_<APPLE_API_KEY>.p8`) when the path var is unset. Matching the conventional filename means the build works under either code path, which insulates the workflow from a tauri-cli version bump. The Tauri docs explicitly warn the fallback "might change in the future," so we set the path var *and* name the file conventionally.
- **`$RUNNER_TEMP`, not the checkout.** Also deliberately *not* `./private_keys`, which the fallback lookup would find — keeping the key outside the working tree means `swatinem/rust-cache` and the bundler can never see it.

**d) Add a cleanup step** at the end of the job:

```yaml
      - name: Remove App Store Connect API key
        if: always() && matrix.platform == 'macos-latest'
        shell: bash
        run: rm -rf "$RUNNER_TEMP/appstoreconnect"
```

`if: always()` so the key is removed even when the build fails — GitHub does clean `RUNNER_TEMP` between jobs on hosted runners, but the explicit removal is what makes this safe if a fork moves to self-hosted runners.

**e) Pass the signing secrets into the existing "Build and upload release" step's `env:` block**, unconditionally (same pattern already used for `TAURI_SIGNING_PRIVATE_KEY`, which is also cross-platform-declared but only meaningful where relevant):

```yaml
      - name: Build and upload release
        uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_API_ISSUER: ${{ secrets.APPLE_API_ISSUER }}
          APPLE_API_KEY: ${{ secrets.APPLE_API_KEY }}
        with: ...
```

`APPLE_API_KEY_PATH` is deliberately absent from this block — it arrives via `$GITHUB_ENV` from step (c), so it's set only on the macOS leg and only when signing is actually configured.

Referencing an unset repository secret in GitHub Actions evaluates to an empty string, not an error — so on forks without these secrets, `APPLE_SIGNING_IDENTITY` etc. reach the step as `""`, `tauri-cli`'s macOS codesign path sees no identity, and behavior is unchanged from today. The Windows/Linux matrix legs receive these env vars too (they're declared once per step, not per-platform), but `tauri-cli`'s macOS-only codesign logic never runs there — harmless no-ops, same as `TAURI_SIGNING_PRIVATE_KEY` already being present on non-relevant legs today.

### 2. `src-tauri/tauri.conf.json`

No changes required for baseline Developer ID signing + notarization — identity comes from env, not config.

Flagged as an **open question** (see below): whether hardened-runtime entitlements are needed for the AppleScript/System Events automation the Accessibility-based lock feature relies on (AGENT.md §7.3). If Phase 3 validation shows the notarized build breaks lock, add `src-tauri/entitlements.plist` with `com.apple.security.automation.apple-events` and reference it via `bundle.macOS.entitlements` — but do not add this speculatively.

### 3. Documentation

- **`README.md`**: update the "Code signing" line (~169) to reflect the new optional path, and add a subsection under the release/forking docs (near the existing `RELEASE_ENV` / `TAURI_SIGNING_PRIVATE_KEY` secret setup instructions, ~lines 123-147):

  **"Optional: macOS code signing & notarization"**

  1. **Prerequisites:** a paid Apple Developer Program membership and a **Developer ID Application** certificate (not "Apple Development" or "Mac App Distribution" — those are for Xcode/App Store, not direct distribution).

  2. **Export the certificate.** In Keychain Access, export the cert + its private key as a `.p12` (set a password when prompted), then base64-encode it:
     ```bash
     base64 -i DeveloperIDApplication.p12 | pbcopy
     ```

  3. **Create an App Store Connect API key.** In [App Store Connect](https://appstoreconnect.apple.com) → **Users and Access** → **Integrations** → **App Store Connect API** → **Team Keys**, click **+**, name it (e.g. `notarization-ci`), and give it the **Developer** role — that is sufficient for notarization; don't grant Admin. Then:
     - Download the `AuthKey_XXXXXXXXXX.p8`. **Apple lets you download it exactly once** — if you lose it, revoke the key and make a new one. (The download link may only appear after reloading the page.)
     - Copy the **Key ID** (the 10-character string in the row).
     - Copy the **Issuer ID** (the UUID shown above the keys table).
     - Base64-encode the key file:
       ```bash
       base64 -i AuthKey_XXXXXXXXXX.p8 | pbcopy
       ```

  4. **Set the six repository secrets:**
     ```bash
     gh secret set APPLE_CERTIFICATE           # base64 from step 2
     gh secret set APPLE_CERTIFICATE_PASSWORD  # the .p12 password from step 2
     gh secret set APPLE_SIGNING_IDENTITY      # e.g. "Developer ID Application: Your Name (TEAMID)"
     gh secret set APPLE_API_ISSUER            # Issuer ID (UUID) from step 3
     gh secret set APPLE_API_KEY               # Key ID (10 chars) from step 3
     gh secret set APPLE_API_KEY_P8            # base64 of the .p8 from step 3
     ```
     Tip: `gh secret set APPLE_API_KEY_P8 < <(base64 -i AuthKey_XXXXXXXXXX.p8)` avoids putting the key through the clipboard.

     To find your signing identity string: `security find-identity -v -p codesigning | grep "Developer ID Application"`.

  5. **Store the `.p8` somewhere safe** (password manager) and delete it from `~/Downloads` — it is not recoverable from Apple.

  6. **All-or-nothing:** setting only some of the six secrets fails the release workflow immediately with a clear error (from the guard step in Phase 1), rather than partway through a 10+ minute build.

  7. **Doing nothing is fine.** Leaving all six unset (the default for a new fork) keeps producing today's unsigned build.

  8. **Revocation:** if CI is compromised, revoke the key in App Store Connect → Integrations. No personal Apple ID password is involved anywhere in this flow.

- **`.github/workflows/release.yml` header comment**: extend the existing "Required repository secrets" block with a new "Optional repository secrets (macOS signing + notarization)" section listing the six vars, noting that `APPLE_API_KEY_PATH` is derived on the runner, and linking this plan / the Tauri doc.

### 4. `AGENT.md`

Optional, low-priority: add one line to §7.1 or §6.3 noting that macOS codesign secrets exist and are optional. Not required for this plan to land.

---

## Implementation Phases

**Phase 1 — Workflow changes**
- Add the guard step, the `.p8` materialization step, the cleanup step, and the `APPLE_*` env passthrough to `.github/workflows/release.yml`.
- Extend the header comment block.

**Phase 2 — Documentation**
- Update `README.md`'s code-signing line and add the setup subsection.

**Phase 3 — Manual validation** (requires the user's own Apple Developer account; cannot be done by the agent)
- Confirm the **unsigned path** still works: with no `APPLE_*` secrets set, cut a test release and verify the macOS bundle is the same shape as before, and that the materialization/cleanup steps are skipped rather than failing (regression check — this is the critical "don't break existing forks" check).
- Confirm the **signed path**: set all six secrets on the user's own fork, cut a test release, then verify:
  - `codesign --verify --deep --strict --verbose=2 /path/to/App.app` passes
  - `codesign -d --entitlements - --verbose=2 /path/to/App.app` shows the hardened runtime flag (`flags=0x10000(runtime)` in `codesign -dv`)
  - `spctl -a -vvv --type install /path/to/App.dmg` (or `--type exec` for the `.app`) reports "accepted" / `source=Notarized Developer ID`
  - `xcrun stapler validate /path/to/App.app` reports the ticket is stapled
  - `xcrun notarytool history --key <p8> --key-id <APPLE_API_KEY> --issuer <APPLE_API_ISSUER>` shows the submission as **Accepted** (this doubles as a check that the API-key credentials themselves are valid)
  - The Accessibility-based lock feature (AppleScript keystroke via System Events, AGENT.md §7.3) still works in the signed+notarized build — this is the one behavior most likely to regress under hardened runtime.
- Confirm the **partial-config guard**: set only `APPLE_CERTIFICATE` and verify the workflow fails fast at the guard step with a clear message, before the expensive build/notarize steps run.
- Confirm the **bad-base64 guard**: temporarily set `APPLE_API_KEY_P8` to garbage and verify the materialization step errors with the "did not decode to a PEM private key" message rather than failing later inside `notarytool`.
- Confirm **no key leakage**: the job log shows no `.p8` contents, and `git status` on the runner (or the uploaded bundle contents) contains no key file.

**Phase 4 — Upstream PR**
- Once Phase 3 passes, open the PR against `ScreenSaverGallery/liminal-screen` — the change is purely additive and backward-compatible, so it should be mergeable without requiring maintainers to own Apple Developer credentials themselves.

---

## Files Touched

| File | Change |
|------|--------|
| `.github/workflows/release.yml` | Header comment extended; guard step (macOS leg only); `.p8` materialization step exporting `APPLE_API_KEY_PATH`; `if: always()` cleanup step; `APPLE_*` secrets passed to the `tauri-action` build step |
| `README.md` | Update code-signing line; add "Optional: macOS code signing & notarization" subsection with App Store Connect API key setup |
| `src-tauri/entitlements.plist` (maybe) | Only if Phase 3 validation shows the lock feature breaks under hardened runtime — not added speculatively |

---

## Verification

- [ ] No `APPLE_*` secrets set → release workflow succeeds, macOS bundle is unsigned exactly as before; the materialization step is skipped, not failed (no regression for forks that don't opt in)
- [ ] All six `APPLE_*` secrets set → macOS leg produces a signed, notarized, **stapled** `.app`/`.dmg`
- [ ] Notarization submission authenticates via the App Store Connect API key — no Apple ID or app-specific password anywhere in the workflow
- [ ] Partial secret set (e.g. only `APPLE_CERTIFICATE`) → guard step fails fast with a clear `::error::` listing exactly which secrets are missing
- [ ] Malformed `APPLE_API_KEY_P8` → materialization step fails with an actionable message before the build starts
- [ ] The `.p8` lands only under `$RUNNER_TEMP` with mode `0600`, never in the checkout, never in the bundle, and is removed by the cleanup step even on build failure
- [ ] Windows/Linux legs unaffected — no `APPLE_*`-related steps run there
- [ ] Accessibility-based lock (AppleScript/System Events) still functions in the signed+notarized build
- [ ] Documented `gh secret set` / `base64` commands in the README work as written

---

## Open Questions

1. **Should the Apple ID method be supported as a fallback?** This plan implements the App Store Connect API key path only, for the reasons in Problem/Context. Supporting both (`APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` as an alternate credential set) would mean the guard step has to validate "either group A complete, or group B complete, or neither" — noticeably more complex for a fallback nobody has asked for. Decision: **API key only for v1**; a fork that insists on Apple ID auth can add the three env vars to the build step itself, since tauri-cli accepts either. Revisit if a fork actually hits a blocker minting a team key (e.g. an individual-account membership without App Store Connect Admin access — worth confirming during Phase 3 whether individual developer accounts can create Team Keys at all).
2. **Hardened runtime entitlements for the lock feature.** Notarization requires the hardened runtime, which restricts some capabilities by default. AppleScript/System Events automation (used for the Accessibility-based lock, AGENT.md §7.3) is generally unaffected by hardened runtime (that's more of an App Sandbox concern), but this should be confirmed empirically in Phase 3 rather than assumed. If it breaks, the fix is a small `entitlements.plist` with `com.apple.security.automation.apple-events`, not a redesign.
3. **Guard step placement.** Keeping the guard inline in the `macos-latest` matrix leg (rather than a separate job) was chosen for simplicity — the secrets are only relevant to that leg, and cross-job `needs:` between matrix legs adds complexity not justified here.
4. **Cleanup step and `RUNNER_TEMP` on self-hosted runners.** The `if: always()` cleanup covers the normal failure path, but a job cancelled hard enough (runner killed) can leave the `.p8` on disk. On GitHub-hosted runners this is moot — the VM is destroyed. Worth a one-line note in the README for anyone running this on self-hosted macOS runners. Not a blocker.
