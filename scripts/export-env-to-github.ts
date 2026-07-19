#!/usr/bin/env bun
// scripts/export-env-to-github.ts
//
// CI only: appends every VITE_* variable from `.env` to $GITHUB_ENV so that
// SUBSEQUENT workflow steps see them as real environment variables.
//
// Why this exists: the Rust backend bakes fork identity (saver URLs, timing
// defaults, app name, ...) into the release binary at COMPILE TIME via
// `option_env!`, which reads the process environment — NOT the `.env` file.
// Writing `.env` to disk is enough for Vite and for the tauri.conf merge-patch,
// but without this export the compiled binary silently falls back to
// `about:blank` / built-in defaults (the local-build equivalent is
// `set -a; source .env; set +a` — see README).
//
// Uses $GITHUB_ENV heredoc syntax so multi-line values (PEM keys) survive.

import { appendFileSync, existsSync, readFileSync } from "fs";
import { parseEnv } from "./parse-env";

const GITHUB_ENV = process.env.GITHUB_ENV;
if (!GITHUB_ENV) {
  console.error("[export-env] ERROR: $GITHUB_ENV is not set — this script only runs in GitHub Actions.");
  process.exit(1);
}
if (!existsSync(".env")) {
  console.error("[export-env] ERROR: no .env file — materialize it from the RELEASE_ENV secret first.");
  process.exit(1);
}

const env = parseEnv(readFileSync(".env", "utf-8"));
const DELIMITER = "__LIMINAL_ENV_EOF__";
const exported: string[] = [];

for (const [key, value] of Object.entries(env)) {
  if (!key.startsWith("VITE_")) continue;
  if (value.includes(DELIMITER)) {
    console.error(`[export-env] ERROR: value of ${key} contains the heredoc delimiter.`);
    process.exit(1);
  }
  appendFileSync(GITHUB_ENV, `${key}<<${DELIMITER}\n${value}\n${DELIMITER}\n`);
  exported.push(key);
}

if (!env.VITE_SAVER_URL) {
  console.error("[export-env] ERROR: VITE_SAVER_URL is missing — the saver would ship as about:blank.");
  process.exit(1);
}

console.log(`[export-env] Exported to build environment: ${exported.join(", ")}`);
