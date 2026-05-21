# Roadmap

Living doc. User feedback queued for triage. Score impact (1 low, 5 high) and effort (1 small, 5 large) before scheduling.

## v0.1.x post-launch feedback (2026-05-07)

| # | Item | Source | Impact | Effort | Notes |
|---|---|---|---|---|---|
| 2 | Smoother install flow (`.dmg` with Applications drop target, or `.pkg`) | Onboarding observation | 4 | 2 | `create-dmg` with styled background. Half a day. |
| 3 | Blur / dull background during session | User request | 3 | 2 | Full-screen NSPanel dimmer behind the orb. Privacy check first. |
| 4 | Visual hierarchy in session text | Onboarding observation | 4 | 1 | Heading = current action (e.g. "Inhale"), smaller text below = what's next ("then hold"). Add "breathe through your nose" cue. Copy + layout in `BreathingSession.tsx`. |
| 5 | App findable in tray (icon hidden when menu bar full) | Onboarding observation | 5 | 2 | Pair with #6, #7. Global hotkey + clearer icon. |
| 6 | Auto-open panel on first launch + "where I live" walkthrough | Onboarding observation | 5 | 2 | One-time onboarding pointer to the tray icon. |
| 8 | Music selection + delayed playback | User request | 3 | 2 | Dropdown icon in session UI to pick a track or "Random". Selection persists. Music starts only after user clicks Begin (not on panel open). |
| 9 | Background update checks (tray app rarely relaunched) | Self-identified | 4 | 2 | Periodic manifest check (every 24h while running) + silent download. Install applies on next natural launch. No banner, no forced restart. |

## Forward-looking ideas (2026-05-13)

| # | Item | Impact | Effort | Notes |
|---|---|---|---|---|
| 10 | Offer 30s box breathing on every app open | 4 | 2 | Cold-start prompt: "Ready for 30s box breathing?" Builds the habit loop. Must not feel nagging; needs a dismiss/snooze. |
| 11 | Notifications cite research | 3 | 2 | Rotate reminder copy with one-line research quotes ("Slow breathing lowers cortisol within 90s, …"). Source list curated, no live fetch. |
| 12 | Opt-in research partnerships | 5 | 5 | Recruit consistent users into IRB-friendly studies. Long horizon. Needs ethics/legal review before any pitch. |
| 13 | Pledge prompt | 3 | 1 | Onboarding ask: "I'll breathe with Niyora for 7 days." Soft commitment. One screen in `Onboarding.tsx`. |
| 14 | Smarter reminder timing | 5 | 4 | Surface reminders when situational signals say the user is overworked (longer-than-usual focus block, late hour, etc.). Builds on existing collectors. |
| 15 | Opt-in telemetry | 4 | 3 | Shipped. Consent slide added to onboarding; choice persisted; `telemetry.rs` forwards an allow-listed set of anonymous events to PostHog EU. Per-event meta allow-list: `session_recorded` carries technique name/kind/completed; `reminder_fired` drops `score`/`label`; pss4 data never sent. Crash reporting parked for a proper Sentry integration. Marketing-site `/privacy/` page must be updated in niyora-web before release. |
| 16 | HRV integration to measure impact | 4 | 5 | HealthKit is iOS-only, so this needs a companion iPhone app syncing to the Mac over local network. Parallel track, doesn't block macOS polish. Spec: `docs/hrv-companion-spec.md`. |
| 17 | Marketing roadmap | 5 | 3 | Separate doc: positioning, channels, launch beats. Tie to research credibility (#11, #12) and measurable impact (#16). |
| 18 | App disappears from tray after Mac restart | User report | 5 | 1 | App doesn't relaunch at login, so the tray icon is gone until the user opens Niyora manually. Register as a macOS Login Item (Tauri autostart plugin or `SMAppService`). Default on; expose toggle in onboarding/Settings. Critical for retention. A reminder app that doesn't survive a reboot is invisible. |
| 19 | On-demand HRV before and after via watchOS app | 4 | 4 | Builds on #16. Adds a watchOS target to the `ios/` companion so the Watch takes HRV readings exactly around session start and end (via `HKWorkoutSession`), instead of relying on its passive sparse sampling (which can leave the 5-minute pre/post windows empty). Accuracy upgrade, not a new feature. `docs/hrv-companion-spec.md` parks this because it doubles the surface area; revisit once the basic iOS loop is shipping and we see how often real windows come back empty. |

## Triage notes

- Items 5, 6 are the same root cause: **the app is invisible**. Fix together.
- Items 2, 4 are both onboarding polish; bundle into a "first-run experience" pass.
- Item 16 (HRV) is also a separate track. Spec written; needs an iOS dev, Xcode, and a physical iPhone + Watch to build.
- Item 3 (background blur) needs a privacy review. A full-screen dimmer changes what other apps can see; confirm it doesn't capture or leak anything.

## Shipped

- **Windows / PC build** (was #1). Shipped with Windows cert, signing pipeline, and CI.
- **App version shown in-app.** "Version X" line in the My Soul footer, sourced from `getVersion()` so it always matches `tauri.conf.json`.

## Later / parked

(Move items here once decided against for now, with a one-line reason.)

- **Tray-icon pulse animation** (was #7). Replaced by the 6-second spinning electron shipped in #14. Pulse pattern dropped in favour of a one-shot attention sweep on first launch.
- **Crash / error reporting.** Deferred to a proper Sentry integration (Tauri SDK, catches native + Rust crashes with symbolication). Not bundled with #15; a Rust-only panic hook was rejected as too partial. Until this lands, consent copy must not claim crash reports are collected.
