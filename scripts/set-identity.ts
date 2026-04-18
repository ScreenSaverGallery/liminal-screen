#!/usr/bin/env bun
// scripts/set-identity.ts
// Reads .env and updates tauri.conf.json with VITE_APP_NAME and VITE_APP_DESCRIPTION.
// Called automatically via predev/prebuild lifecycle hooks.
//
// Does NOT touch `identifier` — that must be changed manually by fork developers.

import { readFileSync, writeFileSync, existsSync } from "fs";
import { resolve } from "path";

const root = resolve(import.meta.dir, "..");
const envPath = resolve(root, ".env");
const configPath = resolve(root, "src-tauri/tauri.conf.json");

if (!existsSync(envPath)) {
  console.log("[set-identity] No .env file found — skipping tauri.conf.json patching");
  process.exit(0);
}

// Parse .env (supports quoted and unquoted values)
const envContent = readFileSync(envPath, "utf-8");
const env: Record<string, string> = {};
for (const line of envContent.split("\n")) {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("#")) continue;
  const sep = trimmed.indexOf("=");
  if (sep === -1) continue;
  const key = trimmed.slice(0, sep).trim();
  let val = trimmed.slice(sep + 1).trim();
  // Strip surrounding quotes
  if (
    (val.startsWith('"') && val.endsWith('"')) ||
    (val.startsWith("'") && val.endsWith("'"))
  ) {
    val = val.slice(1, -1);
  }
  env[key] = val;
}

const name = env.VITE_APP_NAME;
const desc = env.VITE_APP_DESCRIPTION;

if (!name && !desc) {
  console.log("[set-identity] No VITE_APP_NAME or VITE_APP_DESCRIPTION in .env — skipping");
  process.exit(0);
}

// Update tauri.conf.json
const config = JSON.parse(readFileSync(configPath, "utf-8"));

if (name) {
  config.productName = name;
  if (config.app?.windows?.[0]) {
    config.app.windows[0].title = name;
  }
  console.log(`[set-identity] productName → "${name}"`);
}

if (desc) {
  config.bundle.shortDescription = desc;
  config.bundle.longDescription = desc;
  console.log(`[set-identity] description → "${desc}"`);
}

writeFileSync(configPath, JSON.stringify(config, null, 2) + "\n");
console.log("[set-identity] tauri.conf.json updated");