# CLAUDE.md

Notes for Claude Code working on this repo.

> See [AGENTS.md](./AGENTS.md) for the rules that apply to **any** AI agent. This file adds Claude Code-specific tips on top of those.

## Quick orientation

- **What this is**: the Niyora macOS app. Tauri 2 + Rust + React + TypeScript.
- **What this is not**: the marketing site. That's [neha-prasad-ux/niyora-web](https://github.com/neha-prasad-ux/niyora-web). If a request is about copy, the orb visual, or DNS, you're in the wrong repo.
- **Why it matters**: this is a privacy product. Every change has to pass the "does this leak anything?" sniff test before it ships.

## Default workflow

1. **Read** [README.md](./README.md), [AGENTS.md](./AGENTS.md), [DESIGN.md](./DESIGN.md), [CONTRIBUTING.md](./CONTRIBUTING.md) before nontrivial changes.
2. **Branch** off the active dev branch (`feat/v1-scaffold` until v1 ships, then `main`).
3. **Verify locally**:
   - `pnpm exec tsc --noEmit`
   - `cargo check --manifest-path src-tauri/Cargo.toml`
   - `pnpm tauri dev` for any UI change — actually click around, don't just rely on type checks
4. **PR** with a tight description (what + why + how to test).
5. **Merge** after review.

## House rules (Claude-specific)

These are on top of [AGENTS.md](./AGENTS.md), not instead of it.

- **Terse by default.** One-sentence answers. Ask a focused question rather than explaining at length.
- **No em dashes in any output the user will see** — that includes commit messages, code comments, copy strings, and PR descriptions. Use periods, commas, or middle dots (`·`).
- **No "I think we should also…" tangents.** Stick to the requested change. If you spot something else worth fixing, mention it in one line at the end and let the user decide.
- **Don't generate documentation files unless asked.** This repo has the doc files it needs.
- **Don't add comments to explain code that already reads clearly.** A comment must justify *why*, not narrate *what*.
- **Don't add error handling for cases that can't happen.** Trust internal invariants; only validate at boundaries.

## Things that are easy to get wrong

- **The reminder loop polls the situational collectors every 30s** (was 5s, fixed for energy). If you touch `APP_POLL_INTERVAL` in `src-tauri/src/situational/collectors.rs`, you're changing battery life. Confirm the rationale.
- **`tauri.conf.json` is the source of truth for bundle ID, signing identity, entitlements.** Don't rename `com.niyora.breathing` — it changes the data dir path and breaks existing users' sessions.
- **The picker overlay needs `z-index: 11`** to sit above other settings UI. If you see it appearing under something, that's the regression.
- **Dev-only code uses `import.meta.env.DEV`** (frontend) or `#[cfg(debug_assertions)]` (Rust). Anything gated this way must NOT ship in release builds. Verify with `pnpm tauri build` if you change a dev gate.
- **Notifications use `mac-notification-sys` directly** (not Tauri's notification plugin). The plugin doesn't support inline action buttons; we need them for "Breathe now" / "Snooze 30 min." Don't switch back to the plugin.
- **The frontend renders inside an NSPanel popover, not a normal Tauri window.** This affects focus behavior, click-outside-to-close, and CSS `position: fixed` quirks. If something feels weird about layout, suspect this.

## Project context (from memory)

- **Founder**: Neha Prasad. Non-developer; prefers options/pushback over silent compliance.
- **Strategy**: privacy-first stress prevention for high-stress professionals. No wearables. No cloud. 9-step roadmap.
- **Wave 4 (planned)**: gamification of My Soul tiers based on practice history (not technique difficulty).
- **Pranayama variety**: multiple breathing techniques, randomly selected per session.

## What lives where

| You want to change… | Open this |
|---|---|
| The breathing canvas / particle animation | `src/BreathingSession.tsx` |
| A breathing or mindfulness technique | `src/techniques.ts` |
| Soul tier definitions | `src/tiers.ts` |
| The "My Soul" panel | `src/Settings.tsx` |
| First-launch onboarding | `src/Onboarding.tsx` |
| Reminder timing logic | `src-tauri/src/reminder.rs` |
| Passive signal collectors | `src-tauri/src/situational/` |
| Tray menu / panel wiring | `src-tauri/src/main.rs` |
| Local data dir / config | `src-tauri/src/config.rs` |
| App icons / audio | `public/icons/`, `public/audio/` |
| Bundle ID / signing / entitlements | `src-tauri/tauri.conf.json` |
| Conventions for agents | this file or `AGENTS.md` |

## When you're unsure

- Stop and ask. The user is a non-developer founder; one focused question is worth a lot more than a wall of context.
- For anything affecting privacy, notifications, or signing/notarising, default to the more conservative option.
- If you can't run a UI change locally, say so explicitly — don't claim a fix works because the type checker is happy.
