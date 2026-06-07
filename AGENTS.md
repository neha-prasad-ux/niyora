# AGENTS.md

Conventions for AI agents (Claude Code, Cursor, Copilot, ChatGPT, etc.) working on this repository.

This file follows the [agents.md](https://agents.md) convention.

## What this repo is

The Niyora macOS menu-bar app. Tauri 2 (Rust backend) + React 18 + TypeScript frontend. Distributed as a notarised, Developer ID signed `.dmg`. See [README.md](./README.md) for the full overview.

## What this repo is not

- The marketing site. That lives at [neha-prasad-ux/niyora-web](https://github.com/neha-prasad-ux/niyora-web).
- A backend service. Niyora has no server. Don't propose one.
- A wearable integration. We don't talk to Oura, Whoop, Apple Health, etc.

## Hard constraints

These are not preferences. They are non-negotiable. PRs that violate them will be closed.

1. **Three egress types only; everything else is prohibited.** (1) Update-check: a plain version-JSON fetch to `downloads.niyora.com` with no personal data attached (see `updater.rs`). (2) Update binary download from the same host when a newer version is found. (3) Opt-in analytics: anonymous events forwarded to PostHog EU only when the user explicitly consented on the onboarding slide; allow-listed event types only; stress data never leaves the device (see `telemetry.rs`). No remote config. No Sentry. No cloud sync.
2. **No accounts, no logins, no cloud sync.** Sessions live in `~/Library/Application Support/com.niyora.breathing/` as append-only JSONL.
3. **Bundled assets only.** Fonts, sounds, icons all bundled with the app.
4. **No keystroke logging.** The situational signal collectors count system-wide events but never read content.
5. **macOS and Windows are shipped.** iOS lives in the separate repo `niyora-ios`. No web app.

## How to work

1. **Read** [README.md](./README.md), this file, [CLAUDE.md](./CLAUDE.md), [DESIGN.md](./DESIGN.md), and [CONTRIBUTING.md](./CONTRIBUTING.md) before making nontrivial changes.
2. **Branch** from `feat/v1-scaffold` (active dev branch) or `main` once `feat/v1-scaffold` lands. Use prefixes `feat/`, `fix/`, `chore/`, `docs/`.
3. **Verify locally**:
   - `pnpm exec tsc --noEmit` (TypeScript)
   - `cargo check --manifest-path src-tauri/Cargo.toml` (Rust)
   - `pnpm tauri dev` to spot-check UI changes
4. **Open a PR**. Description: what changed, why, how to test. No marketing language.
5. **Merge** after the user reviews.

## House style

- **Terse responses to the user.** One sentence default. Ask a focused question over giving a wall of context.
- **No em dashes (—) in user-facing copy.** Use periods, commas, or middle dots (`·`).
- **No AI attribution in commits.** A global `commit-msg` hook strips Claude/AI lines automatically. Don't write them in the first place.
- **No comments narrating obvious code.** Only write a comment when the *why* would surprise a future reader.
- **No new heavy dependencies for small wins.** Bundle size matters; cold-launch time matters more.

## Tech stack

| Layer | Tech |
|---|---|
| App framework | [Tauri 2](https://v2.tauri.app/) |
| Window style | macOS NSPanel via [tauri-nspanel](https://github.com/ahkohd/tauri-nspanel) |
| Backend | Rust (modules in `src-tauri/src/`) |
| Frontend | React 18 + TypeScript |
| Build | Vite |
| Notifications | [`mac-notification-sys`](https://crates.io/crates/mac-notification-sys) (used directly for inline action buttons) |
| Storage | Append-only JSONL in `~/Library/Application Support/com.niyora.breathing/` |
| Tests | Playwright (`tests/e2e/`), Cargo unit tests (Rust) |

## Folders

- `src/` — React frontend (see [README.md](./README.md) for the full file map)
- `src-tauri/src/` — Rust backend
  - `main.rs` — tray, panel, event wiring
  - `reminder.rs` — background reminder loop + notifications
  - `situational/` — passive signal collectors (idle, mic, app switches, keystrokes — counts only)
  - `sessions.rs` — local session log
  - `analytics.rs` — local event log (still no network)
  - `config.rs` — config persistence
  - `onboarding.rs` — first-launch state
- `tests/e2e/` — Playwright tests with snapshot baselines
- `public/audio/`, `public/icons/` — bundled audio and icon assets

## Useful commands

```bash
pnpm install
pnpm tauri dev                 # local dev with live reload + devtools
pnpm tauri build               # release .app bundle (signed if certs present)

pnpm exec tsc --noEmit         # type check
pnpm test:visual               # Playwright snapshot tests

cd src-tauri && cargo check    # Rust compile check
cd src-tauri && cargo test     # Rust unit tests
```

## Privacy posture (summary)

- No network requests of any kind from the app.
- Passive signals (screen idle, frontmost app bundle ID, mic device state, system event counters) are read in-memory and used only to compute reminder timing. Never logged. Never transmitted.
- Sessions, events, config files live entirely on the user's Mac.

## When you're unsure

- Stop and ask. The maintainer is a non-developer founder. They prefer one focused question over an explanation.
- For anything affecting privacy or notification behavior, default to the more conservative option.
