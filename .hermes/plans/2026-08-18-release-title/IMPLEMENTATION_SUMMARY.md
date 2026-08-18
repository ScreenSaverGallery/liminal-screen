# Implementation Summary: Dynamic GitHub Release Title

**Status:** Implemented  
**Completed:** 2026-08-18

---

## What Changed

The GitHub release title created by `.github/workflows/release.yml` was hardcoded to `"Liminal Screen ${{ github.ref_name }}"`, which was incorrect for forks (e.g. **ScreenSaverGallery**).

The workflow now reads `VITE_APP_NAME` from the materialized `.env` (sourced from the `RELEASE_ENV` repository secret) and uses it as the release title. If the env var is missing, it falls back to `"Liminal Screen"`.

---

## Files Changed

- `.github/workflows/release.yml`
  - Added a step in the "Materialize .env" job that extracts `VITE_APP_NAME` and writes it to `$GITHUB_ENV`.
  - Changed `releaseName` from `"Liminal Screen ${{ github.ref_name }}"` to `"${{ env.VITE_APP_NAME || 'Liminal Screen' }} ${{ github.ref_name }}"`.

---

## Validation

- YAML syntax is valid.
- The `VITE_APP_NAME` extraction uses the same regex style already used for `VITE_UPDATER_PUBKEY` and `VITE_APP_VERSION` in the same step.
- Fallback keeps the title sensible if a fork's `.env` does not contain `VITE_APP_NAME`.

---

## Follow-up

Next time a release is cut with `bun run tauri:release`, the draft GitHub release should appear as **"ScreenSaverGallery vX.Y.Z"** (or whatever `VITE_APP_NAME` is set to in `RELEASE_ENV`).
