# Contributing to Niyora

Thanks for your interest. A few notes before you dive in.

## Project values

These shape what we accept and reject:

1. **Privacy is a hard constraint.** No analytics, no telemetry, no outbound network requests from the app. PRs that add these will be closed.
2. **No accounts, no logins, no cloud sync.** All state lives in `~/Library/Application Support/com.niyora.breathing/`.
3. **Calm in 60 seconds.** Practices stay short, instructions stay clear, friction stays minimal.
4. **Wearable integrations are out of scope.** No Oura, Apple Health, Whoop, etc.

If you're unsure whether a feature fits, open an issue first.

## Setup

```bash
git clone https://github.com/neha-prasad-ux/niyora.git
cd niyora
pnpm install
pnpm tauri dev
```

Requires Rust (via [rustup](https://rustup.rs/)), Node 18+, pnpm, and Xcode Command Line Tools.

## Running tests

```bash
# Frontend Playwright tests
pnpm test:visual

# Backend Rust unit tests
cd src-tauri && cargo test --bin niyora
```

## Code conventions

- **TypeScript:** strict mode on. No `any` unless absolutely necessary, with a comment explaining why.
- **Rust:** keep modules focused. Public items get a `///` doc comment. Use `#[cfg(debug_assertions)]` to gate dev-only code paths so they tree-shake out of release builds.
- **CSS:** classes scoped by feature prefix (`.soul-*`, `.picker-*`, `.onboarding-*`, etc.). Avoid global selectors.
- **No em dashes** in user-facing copy. Use periods, commas, or middle dots (`·`).

## What kinds of contributions help

- Bug fixes (especially edge cases in the situational signal collectors)
- New pranayama / mindfulness techniques (with a citation to the underlying research)
- Translations / localisation
- Visual polish — animations, accessibility, dark mode tweaks
- Tests — particularly Rust unit tests for the situational scoring logic
- Documentation improvements

## What we'll likely decline

- Anything that introduces network requests, accounts, telemetry, or wearable integrations
- Adding heavy dependencies for small wins
- Refactors with no functional improvement

## Submitting a PR

- Open against the `main` branch
- Keep PRs focused — one feature or fix per PR
- Run `pnpm exec tsc --noEmit` and `cargo check --manifest-path src-tauri/Cargo.toml` before pushing
- Write a clear description: what changed, why, how to test

## Questions

Open a GitHub issue or email **neha@niyora.com**.
