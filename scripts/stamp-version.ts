#!/usr/bin/env bun
// scripts/stamp-version.ts
//
// Stamps a version into the committed-but-build-time version files:
//   - package.json            (.version)
//   - src-tauri/Cargo.toml    (the single [package] version line)
//   - src-tauri/Cargo.lock    (the `liminal-screen` crate entry)
//
// Used by the release CI workflow to set these files' versions from the git
// tag *on the runner*, so releases no longer need a version-bump commit on
// `main`. This keeps a fork's `main` a clean fast-forward of upstream.
//
// Cross-platform (Bun) — the same logic release.ts historically used to apply
// the bump locally, now applied in CI from the tag. Run as:
//
//   bun run scripts/stamp-version.ts <x.y.z>

import { readFileSync, writeFileSync } from "fs";
import { join } from "path";

const ROOT = process.cwd();
const PKG_PATH = join(ROOT, "package.json");
const CARGO_TOML_PATH = join(ROOT, "src-tauri", "Cargo.toml");
const CARGO_LOCK_PATH = join(ROOT, "src-tauri", "Cargo.lock");

const SEMVER_RE = /^(\d+)\.(\d+)\.(\d+)$/;

function fail(message: string): never {
  console.error(`[stamp-version] ERROR: ${message}`);
  process.exit(1);
}

/** Bump the crate version in src-tauri/Cargo.toml and its Cargo.lock entry.
 * Mirrors the exact regex semantics release.ts used, so CARGO_PKG_VERSION
 * (baked into the binary) always equals the tag. */
function setCargoVersion(version: string): void {
  const toml = readFileSync(CARGO_TOML_PATH, "utf-8");
  const nameMatch = toml.match(/^name\s*=\s*"([^"]+)"/m);
  const versionRe = /^(version\s*=\s*)"[^"]*"/m;
  if (!nameMatch || !versionRe.test(toml)) {
    fail(`Could not find package name/version in ${CARGO_TOML_PATH}.`);
  }
  writeFileSync(CARGO_TOML_PATH, toml.replace(versionRe, `$1"${version}"`));

  const lock = readFileSync(CARGO_LOCK_PATH, "utf-8");
  const lockRe = new RegExp(
    `(name = "${nameMatch[1]}"\\nversion = )"[^"]*"`,
  );
  if (!lockRe.test(lock)) {
    fail(`Could not find crate "${nameMatch[1]}" in ${CARGO_LOCK_PATH}.`);
  }
  writeFileSync(CARGO_LOCK_PATH, lock.replace(lockRe, `$1"${version}"`));
}

function main(): void {
  const version = process.argv[2];
  if (!version) fail("Usage: bun run scripts/stamp-version.ts <x.y.z>");
  if (!SEMVER_RE.test(version)) fail(`"${version}" is not valid x.y.z semver.`);

  const pkg = JSON.parse(readFileSync(PKG_PATH, "utf-8"));
  pkg.version = version;
  writeFileSync(PKG_PATH, JSON.stringify(pkg, null, 2) + "\n");

  setCargoVersion(version);

  console.log(
    `[stamp-version] Stamped ${version} into package.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock`,
  );
}

main();