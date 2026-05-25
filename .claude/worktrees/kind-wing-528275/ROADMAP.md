# Roadmap

Living doc. User feedback queued for triage. Score impact (1 low, 5 high) and effort (1 small, 5 large) before scheduling.

## v0.1.x post-launch feedback (2026-05-07)

| # | Item | Source | Impact | Effort | Notes |
|---|---|---|---|---|---|
| 1 | Windows / PC build | Multiple users | 5 | 5 | Big TAM unlock. Needs Windows cert, signing pipeline, test machine. Weeks of work. |
| 3 | Blur / dull background during session | User request | 3 | 2 | Full-screen NSPanel dimmer behind the orb. Privacy check first. |
| 4 | Visual hierarchy in session text | Onboarding observation | 4 | 1 | Heading = current action (e.g. "Inhale"), smaller text below = what's next ("then hold"). Add "breathe through your nose" cue. Copy + layout in `BreathingSession.tsx`. |
| 5 | App findable in tray (icon hidden when menu bar full) | Onboarding observation | 5 | 2 | Global hotkey shipped (⌘⌥⇧N); still need a clearer icon and a fallback when registration fails on managed Macs. |
| 7 | Visible "Niyora is active" indicator | User feedback | 4 | 2 | Subtle tray-icon pulse animation. Triggers: (a) ~5s pulse at app launch / day start, (b) pulse on reminder fire until dismissed, (c) slow breath-synced pulse during session, (d) static when idle. macOS template icon, no colour. |
| 8 | Music selection + delayed playback | User request | 3 | 2 | Dropdown icon in session UI to pick a track or "Random". Selection persists. Music starts only after user clicks Begin (not on panel open). |

## Triage notes

- Items 5, 7 are the same root cause: **the app is invisible**. Fix together.
- Item 1 (Windows) is a separate track. Don't let it block macOS polish.
- Item 3 (background blur) needs a privacy review. A full-screen dimmer changes what other apps can see; confirm it doesn't capture or leak anything.

## Done

| # | Item | Shipped in | Notes |
|---|---|---|---|
| 2 | Styled `.dmg` install flow | v0.2.0 (b306ac3) | `create-dmg` background + Applications drop target. |
| 6 | Auto-open panel on first launch | v0.2.0 (b306ac3) | Surfaces panel ~900ms after first launch so the user doesn't hunt for the tray icon. |
| 9 | Background update checks | v0.1.2 (9499bcf) + 24h loop follow-up | Manifest check at launch+5s and every 24h while running. **Partial vs original spec**: still uses an interactive "Update now / Later" notification, not silent download + install-on-next-launch. Revisit if users complain about the prompt. |

## Later / parked

(Move items here once decided against for now, with a one-line reason.)
