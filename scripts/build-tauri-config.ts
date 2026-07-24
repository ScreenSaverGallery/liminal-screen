#!/usr/bin/env bun
// scripts/build-tauri-config.ts
//
// Reads `.env` and `src-tauri/tauri.conf.json`, then writes a Tauri merge-patch
// config to `src-tauri/.tauri-runtime.conf.json` (gitignored) that overrides
// productName, version, identifier, the main window title, bundle descriptions,
// and the updater pubkey/endpoints from environment variables.
//
// The generated file is passed to the Tauri CLI via `--config` in package.json
// (see the `pretauri` lifecycle hook). Tauri merges it into the base
// `tauri.conf.json` using JSON Merge Patch (RFC 7396), so forks never need to
// touch `tauri.conf.json` — only `.env`.
//
// Why this exists: Tauri v2 does NOT support `${{ env.VAR }}` substitution in
// `tauri.conf.json` (the parser is a plain `serde_json::from_str`). The base
// config therefore keeps safe hardcoded defaults, and this script produces
// the per-fork overrides that the CLI applies via `--config`.
//
// Idempotent and safe to run on every `tauri` invocation. If `.env` is missing
// or no relevant vars are set, an empty `{}` merge-patch is written (a no-op
// when merged), so the base config is used as-is.

import { existsSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";
import { parseEnv } from "./parse-env";

const ROOT = process.cwd();
const ENV_PATH = join(ROOT, ".env");
const TAURI_CONF_PATH = join(ROOT, "src-tauri", "tauri.conf.json");
const OUT_PATH = join(ROOT, "src-tauri", ".tauri-runtime.conf.json");

function main(): void {
  // No .env? Emit an empty merge-patch so `--config` is still valid.
  if (!existsSync(ENV_PATH)) {
    console.warn(
      `[build-tauri-config] No .env at ${ENV_PATH}; writing empty merge-patch.`,
    );
    writeFileSync(OUT_PATH, "{}\n");
    return;
  }

  const env = parseEnv(readFileSync(ENV_PATH, "utf-8"));
  const tauriConf = JSON.parse(readFileSync(TAURI_CONF_PATH, "utf-8"));

  // Build the merge-patch. Only include keys that have a non-empty env value
  // so we don't clobber sensible defaults in the base config with empty strings.
  const patch: Record<string, unknown> = {};

  // productName + main window title (RFC 7396 replaces arrays, so we must
  // preserve the full window object when overriding `title`).
  if (env.VITE_APP_NAME) {
    patch.productName = env.VITE_APP_NAME;
    const windows = tauriConf?.app?.windows;
    if (Array.isArray(windows)) {
      patch.app = {
        windows: windows.map((w: Record<string, unknown>) =>
          w.label === "main" ? { ...w, title: env.VITE_APP_NAME } : w,
        ),
      };
    }
  }

  // Bundle descriptions
  if (env.VITE_APP_DESCRIPTION) {
    patch.bundle = {
      ...(patch.bundle as Record<string, unknown> | undefined),
      shortDescription: env.VITE_APP_DESCRIPTION,
      longDescription: env.VITE_APP_DESCRIPTION,
    };
  }

  // Version (must be valid semver — let Tauri's schema validator enforce it)
  if (env.VITE_APP_VERSION) {
    patch.version = env.VITE_APP_VERSION;
  }

  // Bundle identifier (must match reverse-domain pattern)
  if (env.VITE_APP_IDENTIFIER) {
    patch.identifier = env.VITE_APP_IDENTIFIER;
  }

  // Updater pubkey + endpoint
  const updaterPatch: Record<string, unknown> = {};
  if (env.VITE_UPDATER_PUBKEY) {
    updaterPatch.pubkey = env.VITE_UPDATER_PUBKEY;
  }
  if (env.VITE_UPDATER_ENDPOINT) {
    updaterPatch.endpoints = [env.VITE_UPDATER_ENDPOINT];
  }
  if (Object.keys(updaterPatch).length > 0) {
    patch.plugins = { updater: updaterPatch };
  }

  if (!env.VITE_SAVER_URL) {
    console.error(
      "[build-tauri-config] ERROR: VITE_SAVER_URL is missing from .env — the release binary would ship with about:blank as the saver URL.",
    );
    process.exit(1);
  }

  writeFileSync(OUT_PATH, JSON.stringify(patch, null, 2) + "\n");

  const keys = Object.keys(patch);
  if (keys.length === 0) {
    console.log(
      `[build-tauri-config] No relevant vars in .env; wrote empty merge-patch.`,
    );
  } else {
    console.log(
      `[build-tauri-config] Wrote ${OUT_PATH} (overrides: ${keys.join(", ")})`,
    );
  }
}

main();
