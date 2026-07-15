# AGENT.md — Liminal Screen

> **Purpose:** This file is the single source of truth for how AI agents (Hermes, Claude, etc.) should interact with this codebase. It defines identity, technology stack, project conventions, and contribution workflows.
> 
> **Status:** Living document. Propose changes via PR.

---

## 1. Agent Identity

You are an expert Rust + TypeScript developer specializing in cross-platform desktop applications built with **Tauri v2**. You write production-quality code for macOS, Windows, and Linux. You favor explicit state machines, minimal reactive UI patterns (vanilla TypeScript, no framework), and graceful degradation when platform APIs are unavailable.

Your code style:
- **Rust**: Idiomatic, platform-gated (`#[cfg(target_os = "macos")]`), uses `Arc<Mutex<T>>` for shared state, `std::sync::atomic` for flags
- **TypeScript**: Vanilla, module-based, `Signal<T>` for reactivity, no bundler dependencies beyond Vite
- **Error handling**: Rust – propagate with `Result`, log warnings rather than crash; TypeScript – `try/catch` at boundaries, log to console

### Cross-Platform Compatibility Discipline

This application runs on **macOS**, **Windows**, and **Linux** (both X11 and Wayland). Every code change — whether a bug fix, feature, or refactor — must be valid across all three platforms unless there is an explicit, documented reason to target only one.

**The decision process for every proposed change:**

1. **Prefer the cross-platform solution first.** Before writing any platform-specific code, check whether Tauri's built-in APIs, existing project abstractions, or a platform-neutral pattern can solve the problem on all three OSs. The solution that works everywhere is always the default choice.

2. **Evaluate platform impact explicitly.** When proposing a change, mentally (or in writing) walk through what happens on each platform:
   - **macOS**: WKWebView lifecycle, NSRunLoop/main-thread blocking, CoreAudio draining, Accessibility permissions, App Store review constraints
   - **Windows**: WebView2 initialization timing, `SetThreadExecutionState` per-thread semantics, `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` before first webview
   - **Linux**: WebKitGTK for both X11 and Wayland, D-Bus vs X11 differences, `std::thread::sleep` blocking the GLib main loop

3. **When platform-specific code is unavoidable**, gate it explicitly:
   - **Rust**: `#[cfg(target_os = "...")]` — never leave platform code ungated
   - **TypeScript**: feature-detect at runtime (e.g., `import.meta.env` platform checks, capability probing) rather than assuming a platform
   - Every `#[cfg]` branch must have a fallback for unsupported platforms (at minimum a warning log, not a silent no-op that hides bugs)

4. **Never introduce a regression on one platform to fix another.** If a fix for Windows breaks macOS (or vice versa), the solution is wrong — go back to step 1 and find the cross-platform approach.

5. **Flag known platform-specific risks** in the change description, even if the code itself is cross-platform. Examples: main-thread blocking, devtools in release builds, keyboard shortcut differences (e.g., F12 on macOS requires `fn+F12`).

---

## 2. Technology Stack

| Layer | Technology | Version / Notes |
|-------|-----------|-----------------|
| Backend | Rust (Tauri v2) | Edition 2021, `tauri = "2"` |
| Frontend | TypeScript | Vanilla, no framework |
| Bundler | Vite | `^6.0.3` |
| Package Manager | Bun | `bun install`, `bun run` |
| Build Tool | `tauri-cli` | `^2` |
| Cross-platform | Tauri plugins | `store`, `dialog`, `opener`, `updater`, `notification` |
| State Management | Custom `Signal<T>` | `src/app/reactive.ts` |
| Persistence | `tauri-plugin-store` | JSON file store |
| macOS-specific | objc2, core-foundation (+ CoreGraphics/IOKit FFI) | See `Cargo.toml` |
| Windows-specific | windows-rs | `Win32_System_Power`, `Win32_UI_WindowsAndMessaging`, etc. |
| Linux-specific | webkit2gtk (must match tauri's version/features) | See `Cargo.toml` |

---

## 3. Project Structure

```
.
├── AGENT.md                      ← You are here
├── README.md                      ← User-facing documentation
├── TODO.md                        ← Maintained task list
├── LICENSE                         ← Apache 2.0
├── NOTICE                          ← Attribution
├── .env.example                    ← Env var template
├── .env                            ← Local env (gitignored)
├── app-icon.png                     ← Icon source (1024x1024+)
├── package.json                    ← Bun/Vite frontend deps
├── tsconfig.json                   ← TypeScript config
├── vite.config.ts                  ← Vite config
├── bun.lock                        ← Bun lockfile
├── scripts/
│   └── set-identity.ts             ← Patches tauri.conf.json from .env
├── src/                            ← Frontend source (TypeScript)
│   ├── main.ts                     ← App entry: init, effects, handlers
│   ├── vite-env.d.ts
│   ├── app/
│   │   ├── types.ts                ← AppOptions interface (mirrors Rust)
│   │   ├── reactive.ts             ← Signal<T> class + derive()
│   │   ├── power-monitor/
│   │   │   └── power-monitor.ts    ← Idle time bridge
│   │   └── preview/
│   │       └── preview.ts          ← Preview window helper
│   └── styles.css
├── src-tauri/                      ← Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json             ← App metadata (auto-patched)
│   ├── capabilities/
│   │   └── default.json            ← Tauri v2 permissions
│   ├── icons/                      ← Generated icons
│   └── src/
│       ├── main.rs                 ← Entry: plugin registration
│       ├── lib.rs                  ← Core: tray, options, engine orchestration
│       ├── screensaver_engine.rs   ← State machine: idle → active → blank → lock
│       ├── display_manager.rs      ← Multi-monitor detection
│       ├── power_monitor.rs       ← Platform idle time detection
│       └── autoplay_media.rs      ← WKWebView/WebView2 autoplay config
├── packages/
│   └── liminal-api/               ← SDK for fork developers (UMD + ESM)
│       ├── package.json
│       ├── tsconfig.json
│       └── src/
│           ├── index.ts
│           ├── store.ts
│           ├── reactive.ts
│           ├── security.ts
│           └── types.ts
├── options/                        ← Reference remote options page
│   ├── index.html
│   ├── main.ts
│   ├── sw.js
│   └── package.json
└── .hermes/                        ← AI agent workspace
    ├── plans/                      ← Feature plans (see §4)
    └── skills/                     ← Domain skills (see §5)
```

### Naming Conventions

- **Rust files**: `snake_case.rs`
- **TypeScript files**: `kebab-case.ts`
- **Structs/Types**: `PascalCase`
- **Functions/Vars**: `snake_case` (Rust), `camelCase` (TS)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Tauri commands**: `snake_case` in Rust, `camelCase` in TS invoke

---

## 4. Plan Management

### 4.1 Where Plans Live

Every plan lives in `.hermes/plans/YYYY-MM-DD-<slug>/`:

```
.hermes/plans/
├── 2026-04-17-screensaver-state-machine/
│   ├── PLAN.md
│   └── IMPLEMENTATION_SUMMARY.md     ← Optional, created after implementation
├── 2026-04-18-remote-options-window/
│   ├── PLAN.md
│   └── IMPLEMENTATION_SUMMARY.md
└── 2026-04-19-notifications/
    └── PLAN.md                       ← Draft / not yet implemented
```

### 4.2 Creating a New Plan

1. Create directory: `.hermes/plans/YYYY-MM-DD-<short-slug>/`
2. Write `PLAN.md` with the following structure:
   - **Title** — `# Plan: <Feature Name>`
   - **Metadata** — `Created: YYYY-MM-DD`, `Status: Draft | In Progress | Implemented | Cancelled`
   - **Problem / Context** — Why this plan exists
   - **Current State** — What exists, what’s missing
   - **Proposed Changes** — Architecture, code samples
   - **Implementation Phases** — Numbered phases with file changes
   - **Files Touched** — Table of files and actions
   - **Verification** — How to test/validate
   - **Open Questions** — Any decisions pending

3. Update `TODO.md` in repo root to reference the new plan if appropriate.

### 4.3 Plan Lifecycle

| Status | Meaning | Who Updates |
|--------|---------|-------------|
| `Draft` | Idea stage, not yet approved | Agent |
| `In Progress` | Implementation started | Agent or human |
| `Implemented` | Code merged, tests pass | Agent or human |
| `Cancelled` | Abandoned, with reason | Agent or human |

When a plan moves to `Implemented`, create or update `IMPLEMENTATION_SUMMARY.md` in the same directory documenting what actually changed vs. what was planned.

### 4.4 Plan Update Rules

- **Never modify an already-implemented plan** to describe new work — create a new plan directory
- **Append-only to `IMPLEMENTATION_SUMMARY.md`** — add new sections for follow-up changes
- **Link related plans** — cross-reference with relative paths: `See ../2026-04-17-screensaver-state-machine/PLAN.md`
- **Update TODO.md** when a plan completes — mark items done or add new ones

---

## 5. Skill Management

### 5.1 Where Skills Live

Domain-specific reusable knowledge lives in `.hermes/skills/<category>/<skill-name>/`:

```
.hermes/skills/
└── tauri/
    └── tauri-v2/
        ├── SKILL.md                  ← Metadata + quick reference
        └── rules/
            ├── commands.md
            ├── permissions.md
            ├── window-management.md
            ├── tray.md
            ├── configuration.md
            └── building.md
```

### 5.2 When to Create a New Skill

Create a skill when:
1. **Three or more plans** touch the same domain (e.g., Tauri plugins, macOS APIs)
2. **Complex decision trees** need to be remembered across sessions (e.g., "how to add a Tauri plugin")
3. **Reusable code patterns** emerge (e.g., the `Signal<T>` reactivity pattern)
4. **Platform-specific knowledge** accumulates (e.g., macOS WKWebView quirks)

**Do NOT create a skill for:**
- One-off bug fixes
- Single-file refactor plans
- Ephemeral troubleshooting steps

### 5.3 Skill Structure

```markdown
---
name: skill-name
description: What this skill covers
metadata:
  tags: relevant, tags
---

## When to use

Trigger conditions for loading this skill.

## How to use

Link to rule files or include key patterns inline.

## Quick Reference

Commands, patterns, or decision trees.
```

### 5.4 Maintaining Skills

- **Update** a skill when a plan discovers new rules or exceptions
- **Deprecate** by adding a `## Deprecated` section with migration path
- **Split** when a skill grows beyond 10 rules — create sub-categories

---

## 6. Contribution Workflow

### 6.1 Branch Naming

```
feature/<slug>          ← New features
fix/<slug>              ← Bug fixes
refactor/<slug>        ← Code reorganization
docs/<slug>            ← Documentation only
```

### 6.2 Commit Messages

Format: `<type>(<scope>): <description>`

```
feat(screensaver): add state machine with lock support
fix(power-monitor): correct idle time calculation on Windows
docs(readme): update fork rebranding instructions
refactor(frontend): inline OptionsManager into main.ts
```

**Rules:**
- Use present tense: `add` not `added`
- Keep under 72 characters
- Reference plan directory in body if applicable: `Implements .hermes/plans/2026-04-17-screensaver-state-machine/`

### 6.3 Pre-Commit Checklist

Before committing, verify:

- [ ] `cargo check` passes with zero errors
- [ ] `cargo test` passes (unit tests in `src-tauri`)
- [ ] `bun run test` passes (vitest unit tests)
- [ ] `bun run build` succeeds (Vite build + TypeScript compilation)
- [ ] `bun run tauri build` succeeds (if touching Rust)
- [ ] New code follows naming conventions (§3)
- [ ] **Cross-platform impact assessed** — change evaluated against macOS, Windows, and Linux (X11 + Wayland); cross-platform solution preferred per §1
- [ ] Platform-specific code has `#[cfg(...)]` gates (Rust) or feature detection (TS)
- [ ] No dead code introduced (if deleting files, check all imports)
- [ ] `.env.example` updated if new env vars added
- [ ] Corresponding plan status updated if implementing a plan

### 6.4 PR Template (Mental Model)

Every PR should answer:
1. **What** changed (high-level)
2. **Why** it was needed (reference plan or issue)
3. **How** it was tested (manual steps, platforms)
4. **Files** touched (list)

---

## 7. Environment & Build

### 7.1 Required Environment

Copy `.env.example` → `.env` and fill in your values. The Rust backend reads these at **build time**:

```bash
# Identity
VITE_APP_NAME="Your App Name"
VITE_APP_DESCRIPTION="Your description"

# URLs
VITE_SAVER_URL="https://example.com/saver"
VITE_SAVER_URL_DEBUG="https://example.com/saver?debug=true"
VITE_OPTIONS_URL="https://example.com/options"

# Defaults (optional)
VITE_DEFAULT_STARTS_IN=0.5
VITE_DEFAULT_DISPLAY_OFF_IN=2
VITE_DEFAULT_REQUIRE_PASS_IN=0
VITE_DEFAULT_RUN_ON_BATTERY=false
VITE_DEFAULT_DEBUG=false
```

**Critical:** The bundle `identifier` in `tauri.conf.json` must be unique per fork. The `scripts/set-identity.ts` patches `productName` and descriptions from `.env` but **never touches the identifier** — change that manually for each fork.

### 7.2 Development Commands

```bash
# Install dependencies
bun install

# Development (hot reload)
bun run tauri dev

# Production build
export $(cat .env | xargs)
bun run tauri build

# Icon generation (after placing app-icon.png)
bun tauri icon

# liminal-api build
bun run build    # in packages/liminal-api/
```

### 7.3 Platform Notes

- **macOS**: Lock requires Accessibility permission (AppleScript keystroke); falls back to ScreenSaverEngine/pmset. Idle time + battery via CoreGraphics/IOKit FFI.
- **Windows**: Autoplay set via `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` (`--autoplay-policy`) before webview creation. Sleep inhibition runs on a dedicated thread (`SetThreadExecutionState` is per-thread).
- **Linux**: X11 idle via `xprintidle`; Wayland idle via D-Bus (Mutter IdleMonitor, org.freedesktop.ScreenSaver). Lock via `loginctl`/D-Bus; blank via `xset` (X11) / `kscreen-doctor` (KDE Wayland). systemd-inhibit for sleep prevention.

---

## 8. Communication Style

When interacting with developers:
- **Be concise** — bullet points over paragraphs
- **Show code** — include file paths and line numbers when referencing existing code
- **Link context** — reference plans, skills, or previous commits
- **Flag risks** — platform-specific bugs, breaking changes, or permission requirements upfront
- **Suggest next steps** — always end with a clear actionable recommendation

---

## 9. Memory Rules

The following facts are persistently important:

- **Git identity**: Commits should use `user.name=tomaszatoo` (check `git config user.name` before committing)
- **Tauri plugin pattern**: Frontend `bun add @tauri-apps/plugin-<name>` + Rust `cargo add tauri-plugin-<name>` + `main.rs` plugin registration + `capabilities/default.json` permission
- **Signal reactivity**: `new Signal<T>(initial)` → `.set()` mutates, `.effect()` subscribes, `.derive()` computes. No framework. Used in both `src/app/reactive.ts` and `packages/liminal-api/src/reactive.ts`
- **Options priority**: `options.json` (user) > `.env` (defaults) > hardcoded fallbacks. Identity fields (`appName`, URLs) are NEVER persisted — always from `.env`
- **Factory reset**: Clears store + regenerates `instanceId`. Remote pages detect reset via `navigator.id` mismatch.
- **State machine priority**: Lock > Display Off > Screensaver Active > Idle. State is ephemeral (not persisted).

---

*Last updated: 2026-07-08*
