#!/usr/bin/env bun
// scripts/release.ts
//
// One-command release: `bun run tauri:release [patch|minor|major|x.y.z]`
// (defaults to `patch`).
//
// 1. Verifies the working tree is clean, the branch is `main`, and local
//    `main` is up to date with origin.
// 2. Bumps the version in package.json (the committed source of truth for
//    releases) and in the local, gitignored `.env` (to keep dev in sync).
// 3. Commits, tags `vX.Y.Z`, and pushes.
//
// The tag push triggers `.github/workflows/release.yml`, which builds the
// macOS/Windows/Linux bundles and publishes a draft GitHub release. The
// release env config (URLs, updater pubkey, branding) is NOT committed —
// CI reads it from the RELEASE_ENV repository secret and stamps the version
// from the tag. Set it once with:
//
//   gh secret set RELEASE_ENV < .env

import { execSync } from "child_process";
import { existsSync, readFileSync, writeFileSync } from "fs";
import { join } from "path";

const ROOT = process.cwd();
const PKG_PATH = join(ROOT, "package.json");
const ENV_PATH = join(ROOT, ".env");
const CARGO_TOML_PATH = join(ROOT, "src-tauri", "Cargo.toml");
const CARGO_LOCK_PATH = join(ROOT, "src-tauri", "Cargo.lock");

const SEMVER_RE = /^(\d+)\.(\d+)\.(\d+)$/;
const RELEASE_BRANCH = "main";

function fail(message: string): never {
  console.error(`[release] ERROR: ${message}`);
  process.exit(1);
}

function git(cmd: string): string {
  return execSync(`git ${cmd}`, { encoding: "utf-8" }).trim();
}

function bump(current: string, kind: string): string {
  const match = current.match(SEMVER_RE);
  if (!match) fail(`Current version "${current}" is not valid semver.`);
  const [major, minor, patch] = match.slice(1).map(Number);
  switch (kind) {
    case "major":
      return `${major + 1}.0.0`;
    case "minor":
      return `${major}.${minor + 1}.0`;
    case "patch":
      return `${major}.${minor}.${patch + 1}`;
    default:
      if (!SEMVER_RE.test(kind)) {
        fail(`Expected patch|minor|major or an explicit x.y.z, got "${kind}".`);
      }
      return kind;
  }
}

/**
 * Bump the crate version in src-tauri/Cargo.toml and its Cargo.lock entry.
 * CARGO_PKG_VERSION is baked into the binary (used in the injected
 * `navigator.userAgent` suffix and `navigator.liminalScreen.version`), so a
 * stale crate version means remote saver pages see the wrong app version.
 */
function setCargoVersion(version: string): void {
  const toml = readFileSync(CARGO_TOML_PATH, "utf-8");
  const nameMatch = toml.match(/^name\s*=\s*"([^"]+)"/m);
  const versionRe = /^(version\s*=\s*)"[^"]*"/m;
  if (!nameMatch || !versionRe.test(toml)) {
    fail(`Could not find package name/version in ${CARGO_TOML_PATH}.`);
  }
  writeFileSync(CARGO_TOML_PATH, toml.replace(versionRe, `$1"${version}"`));

  // Keep Cargo.lock in sync so builds with --locked don't fail and the diff
  // stays honest. The lock entry is `name = "<crate>"` followed by `version`.
  const lock = readFileSync(CARGO_LOCK_PATH, "utf-8");
  const lockRe = new RegExp(
    `(name = "${nameMatch[1]}"\\nversion = )"[^"]*"`,
  );
  if (!lockRe.test(lock)) {
    fail(`Could not find crate "${nameMatch[1]}" in ${CARGO_LOCK_PATH}.`);
  }
  writeFileSync(CARGO_LOCK_PATH, lock.replace(lockRe, `$1"${version}"`));
}

/** Replace the VITE_APP_VERSION line in a .env-style file. */
function setEnvVersion(path: string, version: string): void {
  const content = readFileSync(path, "utf-8");
  const re = /^VITE_APP_VERSION=.*$/m;
  if (!re.test(content)) fail(`No VITE_APP_VERSION line found in ${path}.`);
  writeFileSync(path, content.replace(re, `VITE_APP_VERSION="${version}"`));
}

function main(): void {
  const kind = process.argv[2] ?? "patch";

  // --- Preflight checks -----------------------------------------------------
  // The pubkey CI uses lives in the RELEASE_ENV secret, which we cannot read
  // from here — the workflow hard-fails on a missing/placeholder pubkey. The
  // local .env usually mirrors the secret, so a placeholder here is worth a
  // heads-up before spending 20 minutes of CI time.
  if (existsSync(ENV_PATH)) {
    const env = readFileSync(ENV_PATH, "utf-8");
    const pubkey = env.match(/^VITE_UPDATER_PUBKEY="?([^"\n]*)"?$/m)?.[1] ?? "";
    if (!pubkey || /\s/.test(pubkey)) {
      console.warn(
        `[release] WARNING: VITE_UPDATER_PUBKEY in your local .env looks like a\n` +
          `[release] placeholder. CI uses the RELEASE_ENV secret, but make sure that\n` +
          `[release] secret holds a real pubkey (gh secret set RELEASE_ENV < .env).`,
      );
    }
  }

  const branch = git("rev-parse --abbrev-ref HEAD");
  if (branch !== RELEASE_BRANCH) {
    fail(`Releases must be cut from "${RELEASE_BRANCH}" (currently on "${branch}").`);
  }
  if (git("status --porcelain") !== "") {
    fail("Working tree is not clean — commit or stash your changes first.");
  }

  console.log("[release] Fetching origin...");
  git("fetch origin --tags");
  const behind = git(`rev-list --count HEAD..origin/${RELEASE_BRANCH}`);
  if (behind !== "0") {
    fail(`Local ${RELEASE_BRANCH} is ${behind} commit(s) behind origin — pull first.`);
  }

  // --- Compute and apply the bump -------------------------------------------
  const pkg = JSON.parse(readFileSync(PKG_PATH, "utf-8"));
  const next = bump(pkg.version, kind);
  const tag = `v${next}`;

  if (git(`tag -l ${tag}`) !== "") fail(`Tag ${tag} already exists.`);

  console.log(`[release] ${pkg.version} -> ${next}`);
  pkg.version = next;
  writeFileSync(PKG_PATH, JSON.stringify(pkg, null, 2) + "\n");
  setCargoVersion(next);
  if (existsSync(ENV_PATH)) setEnvVersion(ENV_PATH, next);

  // --- Commit, tag, push -----------------------------------------------------
  git("add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock");
  git(`commit -m "release: ${tag}"`);
  git(`tag -a ${tag} -m "release: ${tag}"`);
  console.log(`[release] Pushing ${RELEASE_BRANCH} and ${tag}...`);
  git(`push origin ${RELEASE_BRANCH} ${tag}`);

  // Works for both SSH and HTTPS remotes, including SSH host aliases
  // (e.g. git@github.com-tomaszatoo:tomaszatoo/liminal-screen.git).
  const repo =
    git("remote get-url origin").match(/([^/:]+\/[^/:]+?)(\.git)?$/)?.[1] ?? "";
  console.log(`[release] Done. Watch the build: https://github.com/${repo}/actions`);
  console.log(`[release] The release is created as a DRAFT — publish it manually`);
  console.log(`[release] once you have checked the artifacts (publishing makes it`);
  console.log(`[release] visible to the auto-updater via releases/latest).`);
}

main();
