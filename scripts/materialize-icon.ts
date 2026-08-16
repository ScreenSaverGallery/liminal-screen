#!/usr/bin/env bun
// scripts/materialize-icon.ts
//
// Ensures `app-icon.png` exists and generates `src-tauri/icons/*` before a build.
//
// Forks keep their own `app-icon.png` in the project root (gitignored). In CI,
// the icon is injected via the APP_ICON repository secret as a base64-encoded
// PNG and decoded to `app-icon.png`.
//
// After the source icon is available, this runs `bun run tauri icon` to generate
// the platform icon set. If neither a local icon nor the secret exists, it fails
// fast so the build does not die later with missing icon errors.

import { execSync } from "child_process";
import { existsSync, writeFileSync } from "fs";
import { join } from "path";

const ROOT = process.cwd();
const ICON_PATH = join(ROOT, "app-icon.png");

function fail(message: string): never {
  console.error(`[materialize-icon] ERROR: ${message}`);
  process.exit(1);
}

function materializeSourceIcon(): void {
  if (existsSync(ICON_PATH)) {
    console.log("[materialize-icon] Using existing app-icon.png");
    return;
  }

  const encoded = process.env.APP_ICON;
  if (!encoded) {
    fail(
      "No app-icon.png found and APP_ICON secret is not set.\n" +
        "Provide a base64-encoded 1024x1024+ PNG:\n" +
        "  base64 -w 0 app-icon.png | gh secret set APP_ICON",
    );
  }

  let buffer: Buffer;
  try {
    buffer = Buffer.from(encoded, "base64");
  } catch {
    fail("APP_ICON secret is not valid base64.");
  }

  // A valid 1024x1024 PNG is ~10 KB+; anything smaller is almost certainly
  // wrong. tauri icon will do its own validation, but this catches obvious
  // mis-encoded secrets early.
  if (buffer.length < 1024) {
    fail("Decoded APP_ICON is too small to be a valid icon.");
  }

  writeFileSync(ICON_PATH, buffer);
  console.log(`[materialize-icon] Decoded APP_ICON secret to ${ICON_PATH}`);
}

function generateIcons(): void {
  console.log(
    "[materialize-icon] Generating src-tauri/icons/* with `bun run tauri icon`...",
  );
  try {
    execSync("bun run tauri icon", { stdio: "inherit" });
  } catch {
    fail(
      "`bun run tauri icon` failed. Make sure app-icon.png is a valid 1024x1024+ PNG.",
    );
  }
}

function main(): void {
  materializeSourceIcon();
  generateIcons();
  console.log("[materialize-icon] Done.");
}

main();
