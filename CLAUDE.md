# CLAUDE.md

Notes for Claude Code working on this repo.

> See [AGENTS.md](./AGENTS.md) for the rules that apply to **any** AI agent. This file adds Claude Code-specific tips on top of those.

## Quick orientation

- **What this is**: the Niyora macOS app. Tauri 2 + Rust + React + TypeScript.
- **What this is not**: the marketing site. That's [neha-prasad-ux/niyora-web](https://github.com/neha-prasad-ux/niyora-web). If a request is about copy, the orb visual, or DNS, you're in the wrong repo.
- **Why it matters**: this is a privacy product. Every change has to pass the "does this leak anything?" sniff test before it ships.

## Branching and PR discipline (hard rule)

**Never commit to `main`. Never push to `main`. No exceptions.** Not for typo fixes. Not for one-line doc edits. Not for version bumps. Not for "trivial" hotfixes. Not when production is on fire. Not when the change is "obviously safe." Not when the user says "just push it" in passing. If you find yourself about to commit on `main`, stop and branch first.

The workflow is always:

1. `git symbolic-ref --short HEAD` to confirm you are NOT on `main`. If you are, create a branch first.
2. `git checkout -b <prefix>/<slug>` using prefixes `feat/`, `fix/`, `hotfix/`, `chore/`, or `docs/`.
3. Commit on the branch.
4. `git push -u origin <branch>`.
5. Open a PR with `gh pr create`. Description: what changed, why, how to test.
6. The maintainer reviews and merges. You do not merge your own PRs.

**Hotfixes follow the same flow.** A `hotfix/` branch + a PR that the maintainer merges in under a minute is still strictly better than a direct push. The review step is the whole point. Skipping it for "speed" is what causes silent divergence.

**Why this rule exists.** A direct commit to local `main` (`7781b86`, "state-aware tray icon") was forgotten, never opened as a PR, and silently diverged from `origin/main` for over a month while seven other PRs landed. The work was eventually superseded by PR #14 and discarded as wasted effort. Branch protection on GitHub would catch this server-side, but is not available on the free plan for private repos, so this rule is the substitute. Treat it like a server-side check that lives in your head.

**Recovering from a wrong-place commit.** If you have already committed on `main` by mistake: do NOT lose work with a bare reset.

```bash
git branch <prefix>/<slug>        # save the commit on a new branch
git reset --hard origin/main      # clean up main
git checkout <prefix>/<slug>      # continue work on the branch
```

## Release discipline (hard rule)

**Every release ships to staging first. Never push a clean semver tag without a `-beta` tag on the same commit first. No exceptions.** Not for "obviously safe" UI tweaks. Not for one-line copy changes. Not for urgent prod hotfixes. The two-tag flow is the whole release contract.

The release channel is decided by the tag shape (see [.github/workflows/release.yml](./.github/workflows/release.yml)):

- **Hyphenated tag** (`v0.4.0-beta.1`, `v0.4.0-rc.1`) → STAGING. Builds with `src-tauri/tauri.staging.conf.json`: bundle ID `com.niyora.breathing.staging`, product name "Niyora Staging", updater feed at `staging.json`. Installs alongside production. Real users never see it.
- **Clean semver tag** (`v0.4.0`) → PRODUCTION. Builds with the production identifier; updater feed at `latest.json`. Existing users auto-update.

The workflow is always:

1. After PRs merge to `main`, on `main`, bump `package.json` and `src-tauri/tauri.conf.json` versions, commit, push.
2. `git tag vX.Y.Z-beta.1 && git push origin vX.Y.Z-beta.1`. Wait for CI (~20 min). Install from `https://downloads.niyora.com/Niyora-Staging-X.Y.Z-beta.1-AppleSilicon.dmg` (or `Windows-x64.exe`).
3. Smoke-test the change end-to-end on staging. Watch for ~24 to 48h.
4. Promote by tagging the SAME commit with the clean version: `git tag vX.Y.Z <staging-commit> && git push origin vX.Y.Z`. CI rebuilds with the production identity and updates `latest.json`.

If staging breaks, bump to `-beta.2` on a fix commit and restart the clock. No clean version goes out until staging is healthy. If a beta sits un-promoted for more than ~2 days, either promote it or write down why it stalled. Staging is not a graveyard.

**Why this rule exists.** The bundle ID, signing identity, and data dir for the production app belong to real users' sessions. The staging build is structurally identical code with a different identity, so a green staging build is a green production build minus the audience. Skipping staging skips the only checkpoint that catches "ships but immediately breaks for everyone" before it does.

## Default workflow

1. **Read** [README.md](./README.md), [AGENTS.md](./AGENTS.md), [DESIGN.md](./DESIGN.md), [CONTRIBUTING.md](./CONTRIBUTING.md) before nontrivial changes.
2. **Branch** per the hard rule above. Off `main` (or `feat/v1-scaffold` if that branch is still active).
3. **Verify locally**:
   - `pnpm exec tsc --noEmit`
   - `cargo check --manifest-path src-tauri/Cargo.toml`
   - `pnpm tauri dev` for any UI change. Actually click around, don't just rely on type checks.
4. **PR** with a tight description (what + why + how to test).
5. **Merge** after review. The maintainer merges; you do not.

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
| Staging build identity (separate bundle ID) | `src-tauri/tauri.staging.conf.json` |
| Release pipeline (staging vs prod tagging) | `.github/workflows/release.yml` |
| Conventions for agents | this file or `AGENTS.md` |

## When you're unsure

- Stop and ask. The user is a non-developer founder; one focused question is worth a lot more than a wall of context.
- For anything affecting privacy, notifications, or signing/notarising, default to the more conservative option.
- If you can't run a UI change locally, say so explicitly — don't claim a fix works because the type checker is happy.
