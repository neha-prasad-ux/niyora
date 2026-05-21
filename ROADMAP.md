# Roadmap

Living doc. Items move left to right: **Idea → In Progress → Done → Next update → Released**. **Parked** is a side track for ideas decided against, with a reason. Item numbers are stable IDs; they don't change when an item moves between columns.

- **Done**: merged on `main`, no release plan yet.
- **Next update**: queued for the upcoming release.
- **Released**: live to users.

Score impact (1 low, 5 high) and effort (1 small, 5 large) before promoting from Idea to In Progress.

## Idea

| # | Item | Impact | Effort | Notes |
|---|---|---|---|---|
| 2 | Smoother install flow (`.dmg` with Applications drop target, or `.pkg`) | 4 | 2 | Source: onboarding observation. `create-dmg` with styled background. Half a day. |
| 3 | Blur / dull background during session | 3 | 2 | Source: user request. Full-screen NSPanel dimmer behind the orb. Privacy check first. |
| 4 | Visual hierarchy in session text | 4 | 1 | Source: onboarding observation. Heading = current action ("Inhale"), smaller text below = what's next ("then hold"). Add "breathe through your nose" cue. Copy + layout in `BreathingSession.tsx`. |
| 5 | App findable in tray (icon hidden when menu bar full) | 5 | 2 | Source: onboarding observation. Pair with #6. Global hotkey + clearer icon. |
| 6 | Auto-open panel on first launch + "where I live" walkthrough | 5 | 2 | Source: onboarding observation. One-time onboarding pointer to the tray icon. |
| 8 | Music selection + delayed playback | 3 | 2 | Source: user request. Dropdown icon in session UI to pick a track or "Random". Selection persists. Music starts only after user clicks Begin (not on panel open). |
| 9 | Background update checks (tray app rarely relaunched) | 4 | 2 | Source: self-identified. Periodic manifest check (every 24h while running) + silent download. Install applies on next natural launch. No banner, no forced restart. |
| 11 | Notifications cite research | 3 | 2 | Rotate reminder copy with one-line research quotes ("Slow breathing lowers cortisol within 90s, …"). Source list curated, no live fetch. |
| 12 | Opt-in research partnerships | 5 | 5 | Recruit consistent users into IRB-friendly studies. Long horizon. Needs ethics/legal review before any pitch. |
| 13 | Pledge prompt | 3 | 1 | Onboarding ask: "I'll breathe with Niyora for 7 days." Soft commitment. One screen in `Onboarding.tsx`. |
| 14 | Smarter reminder timing | 5 | 4 | Surface reminders when situational signals say the user is overworked (longer-than-usual focus block, late hour, etc.). Builds on existing collectors. |
| 16 | HRV integration to measure impact | 4 | 5 | HealthKit is iOS-only, so this needs a companion iPhone app syncing to the Mac over local network. Parallel track, doesn't block macOS polish. Spec: `docs/hrv-companion-spec.md`. |
| 17 | Marketing roadmap | 5 | 3 | Separate doc: positioning, channels, launch beats. Tie to research credibility (#11, #12) and measurable impact (#16). |
| 18 | App disappears from tray after Mac restart | 5 | 1 | Source: user report. App doesn't relaunch at login, so the tray icon is gone until the user opens Niyora manually. Register as a macOS Login Item (Tauri autostart plugin or `SMAppService`). Default on; expose toggle in onboarding/Settings. Critical for retention. A reminder app that doesn't survive a reboot is invisible. |
| 19 | On-demand HRV before and after via watchOS app | 4 | 4 | Builds on #16. Adds a watchOS target to the `ios/` companion so the Watch takes HRV readings exactly around session start and end (via `HKWorkoutSession`), instead of relying on its passive sparse sampling. Accuracy upgrade, not a new feature. `docs/hrv-companion-spec.md` parks this until the basic iOS loop is shipping and we see how often real windows come back empty. |

## In Progress

| # | Item | Impact | Effort | Notes |
|---|---|---|---|---|
| _none_ | Pick the next item from Idea when starting work. | | | |

## Done

Merged on `main`, no release decision yet.

| # | Item | Notes |
|---|---|---|
| _none_ | | |

## Next update

Queued for the upcoming release. Promote here from Done once decided.

| # | Item | Notes |
|---|---|---|
| _none_ | | |

## Released

Live to users.

| # | Item | Notes |
|---|---|---|
| 1 | Windows / PC build | Released with Windows cert, signing pipeline, and CI. |
| 10 | Box breathing prompt on app open | Released, with deltas from the original spec: triggers on the day's **first** panel open (not every open), forces **Box Breath**, and runs **60s** (not 30s). Wiring: `claim_first_open_today` in `src-tauri/src/main.rs`, `isFirstOpenToday` plumbed into `BreathingSession.tsx`. Copy: "Start your day with a breath. / 60s goes a long way." |
| 15 | Opt-in telemetry | Consent slide in onboarding; choice persisted; `telemetry.rs` forwards an allow-listed set of anonymous events to PostHog EU. Per-event meta allow-list: `session_recorded` carries technique name/kind/completed; `reminder_fired` drops `score`/`label`; PSS-4 data never sent. Marketing-site `/privacy/` page in niyora-web must be kept in sync. |
| · | App version shown in-app | "Version X" line in the My Soul footer, sourced from `getVersion()` so it always matches `tauri.conf.json`. |

## Parked

| # | Item | Reason |
|---|---|---|
| 7 | Tray-icon pulse animation | Replaced by the 6-second spinning electron shipped as part of #14. Pulse pattern dropped in favour of a one-shot attention sweep on first launch. |
| · | Crash / error reporting | Deferred to a proper Sentry integration (Tauri SDK, catches native + Rust crashes with symbolication). Not bundled with #15; a Rust-only panic hook was rejected as too partial. Until this lands, consent copy must not claim crash reports are collected. |

## Triage notes

- Items 5, 6 are the same root cause: **the app is invisible**. Fix together.
- Items 2, 4 are both onboarding polish; bundle into a "first-run experience" pass.
- Item 16 (HRV) is a separate track. Spec written; needs an iOS dev, Xcode, and a physical iPhone + Watch to build.
- Item 3 (background blur) needs a privacy review. A full-screen dimmer changes what other apps can see; confirm it doesn't capture or leak anything.
