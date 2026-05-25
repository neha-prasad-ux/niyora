# Roadmap

Living doc. User feedback queued for triage. Score impact (1 low, 5 high) and effort (1 small, 5 large) before scheduling.

## v0.1.x post-launch feedback (2026-05-07)

| # | Item | Source | Impact | Effort | Notes |
|---|---|---|---|---|---|
| 1 | Windows / PC build | Multiple users | 5 | 5 | Big TAM unlock. Needs Windows cert, signing pipeline, test machine. Weeks of work. |
| 2 | Smoother install flow (`.dmg` with Applications drop target, or `.pkg`) | Onboarding observation | 4 | 2 | `create-dmg` with styled background. Half a day. |
| 3 | Blur / dull background during session | User request | 3 | 2 | Full-screen NSPanel dimmer behind the orb. Privacy check first. |
| 4 | Visual hierarchy in session text | Onboarding observation | 4 | 1 | Heading = current action (e.g. "Inhale"), smaller text below = what's next ("then hold"). Add "breathe through your nose" cue. Copy + layout in `BreathingSession.tsx`. |
| 5 | App findable in tray (icon hidden when menu bar full) | Onboarding observation | 5 | 2 | Pair with #6, #7. Global hotkey + clearer icon. |
| 6 | Auto-open panel on first launch + "where I live" walkthrough | Onboarding observation | 5 | 2 | One-time onboarding pointer to the tray icon. |
| 7 | Visible "Niyora is active" indicator | User feedback | 4 | 2 | Subtle tray-icon pulse animation. Triggers: (a) ~5s pulse at app launch / day start, (b) pulse on reminder fire until dismissed, (c) slow breath-synced pulse during session, (d) static when idle. macOS template icon, no colour. |
| 8 | Music selection + delayed playback | User request | 3 | 2 | Dropdown icon in session UI to pick a track or "Random". Selection persists. Music starts only after user clicks Begin (not on panel open). |
| 9 | Background update checks (tray app rarely relaunched) | Self-identified | 4 | 2 | Periodic manifest check (every 24h while running) + silent download. Install applies on next natural launch. No banner, no forced restart. |

## Triage notes

- Items 5, 6, 7 are the same root cause: **the app is invisible**. Fix together.
- Items 2, 4 are both onboarding polish; bundle into a "first-run experience" pass.
- Item 1 (Windows) is a separate track. Don't let it block macOS polish.
- Item 3 (background blur) needs a privacy review. A full-screen dimmer changes what other apps can see; confirm it doesn't capture or leak anything.

## Later / parked

(Move items here once decided against for now, with a one-line reason.)
