# Roadmap

Living doc. User feedback queued for triage. Score impact (1 low, 5 high) and effort (1 small, 5 large) before scheduling.

## v0.1.x post-launch feedback (2026-05-07)

| # | Item | Source | Impact | Effort | Status | Notes |
|---|---|---|---|---|---|---|
| 1 | Windows / PC build | Multiple users | 5 | 5 | Done | Windows cert, signing pipeline, test machine all in place. |
| 2 | Smoother install flow (`.dmg` with Applications drop target, or `.pkg`) | Onboarding observation | 4 | 2 | Done (#13) | `create-dmg` with styled background. |
| 3 | Blur / dull background during session | User request | 3 | 2 | Not started | Full-screen NSPanel dimmer behind the orb. Privacy check first. |
| 4 | Visual hierarchy in session text | Onboarding observation | 4 | 1 | Done | Phase label + persistent technique instruction below it in `BreathingSession.tsx`. |
| 5 | App findable in tray (icon hidden when menu bar full) | Onboarding observation | 5 | 2 | Done | Global toggle shortcut registered in `main.rs`; state-aware tray icon. |
| 6 | Auto-open panel on first launch + "where I live" walkthrough | Onboarding observation | 5 | 2 | Done (#13) | "I live up here" pointer on first onboarding slide. |
| 7 | Visible "Niyora is active" indicator | User feedback | 4 | 2 | Done | State-aware tray icon (template body + colored electron). |
| 8 | Music selection + delayed playback | User request | 3 | 2 | Not started | Dropdown icon in session UI to pick a track or "Random". Selection persists. Music starts only after user clicks Begin (not on panel open). |
| 9 | Background update checks (tray app rarely relaunched) | Self-identified | 4 | 2 | Done (#10) | In-app auto-updates via `tauri-plugin-updater`. |
| 10 | Show app version in-app | User request | 2 | 1 | Done | "Version X" line in the My Soul panel footer, via `getVersion()` from `@tauri-apps/api/app`. |
| 11 | Ask users to opt in to telemetry | User request | 4 | 3 | Done | Consent slide added to onboarding; opt-in choice persisted; `telemetry.rs` forwards an allow-listed set of anonymous events to PostHog EU, with a per-event meta key allow-list (`session_recorded` carries technique name/kind/completed; `reminder_fired` drops `score`/`label`; pss4 data never sent). Real key wired in. In-app privacy copy updated (My Soul footer + `updater.rs`). Marketing-site `/privacy/` page update tracked separately in the niyora-web repo. |

## Triage notes

- #10 (version in-app) and #11 (telemetry opt-in) shipped. Remaining open items: #3 (background blur) and #8 (music selection).
- Item 3 (background blur) needs a privacy review. A full-screen dimmer changes what other apps can see; confirm it doesn't capture or leak anything.
- #11 follow-up (not blocking): the marketing-site `/privacy/` page must be updated to mention opt-in analytics. Owned by niyora-web.

## Later / parked

- **Crash / error reporting** — deferred to a proper Sentry integration (Tauri SDK, catches native + Rust crashes with symbolication). Not done as part of #11; a Rust-only panic hook was rejected as too partial. Until this lands, consent copy must not claim crash reports are collected.
