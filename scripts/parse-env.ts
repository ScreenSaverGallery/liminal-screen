// scripts/parse-env.ts
//
// Shared quote-aware .env parser used by build-tauri-config.ts (merge-patch
// generation) and export-env-to-github.ts (CI build environment).

export type EnvMap = Record<string, string>;

/**
 * Parse a .env file with quote-aware, multi-line support.
 *
 * Handles:
 *  - Comments (`#`) and blank lines
 *  - Bare values: `KEY=value`
 *  - Single-line quoted values: `KEY="value with spaces"` (surrounding quotes stripped)
 *  - Multi-line quoted values (e.g. PEM keys): the value continues across lines
 *    until the closing quote is found.
 *
 * This mirrors the behavior of the `dotenv`/`dotenvy` crates used by the Rust
 * backend and Tauri's own tooling, so what the runtime sees matches what this
 * script produces.
 */
export function parseEnv(content: string): EnvMap {
  const env: EnvMap = {};
  const lines = content.split("\n");
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    i++;

    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("#")) continue;

    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    const key = line.slice(0, eq).trim();
    if (!/^\w+$/.test(key)) continue;
    let value = line.slice(eq + 1);

    // Multi-line quoted value: opening `"` on this line, no closing `"` yet.
    // Accumulate subsequent lines until a line closes the quote. Quote state
    // is tracked by parity: a line with an ODD number of unescaped quotes
    // flips the state (a continuation line with no quotes at all leaves the
    // value open — e.g. the middle lines of a PEM block).
    if (value.startsWith('"') && quoteCount(value) % 2 === 1) {
      const parts = [value];
      while (i < lines.length) {
        parts.push(lines[i]);
        const closes = quoteCount(lines[i]) % 2 === 1;
        i++;
        if (closes) break;
      }
      value = parts.join("\n");
    }

    // Strip a single pair of surrounding double quotes.
    if (value.length >= 2 && value.startsWith('"') && value.endsWith('"')) {
      value = value.slice(1, -1);
    }

    env[key] = value;
  }

  return env;
}

/** Number of unescaped `"` characters in the line. */
function quoteCount(s: string): number {
  let count = 0;
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '"' && (i === 0 || s[i - 1] !== "\\")) count++;
  }
  return count;
}
