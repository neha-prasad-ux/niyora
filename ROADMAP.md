# Roadmap

Living doc. User feedback queued for triage. Score impact (1 low, 5 high) and effort (1 small, 5 large) before scheduling.

## v0.1.x post-launch feedback (2026-05-07)

| # | Item | Source | Impact | Effort | Notes |
|---|---|---|---|---|---|
| 1 | Windows / PC build | Multiple users | 5 | 5 | Big TAM unlock. Needs Windows cert, signing pipeline, test machine. Weeks of work. |
| 3 | Blur / dull background during session | User request | 3 | 2 | Full-screen NSPanel dimmer behind the orb. Privacy check first. |

## Done

- #2 Smoother install flow (styled `.dmg`) · shipped in 0.2.0
- #4 Visual hierarchy in session text
- #5 Tray-icon discoverability (global hotkey + clearer icon)
- #6 Auto-open panel on first launch + walkthrough · shipped in 0.2.0
- #7 Visible "Niyora is active" indicator (state-aware tray icon)
- #9 Background update checks (24h-throttled, silent install on next launch)

## Triage notes

- Item 1 (Windows) is a separate track. Don't let it block macOS polish.
- Item 3 (background blur) needs a privacy review. A full-screen dimmer changes what other apps can see; confirm it doesn't capture or leak anything.

## Later / parked

- **#8 Music selection + delayed playback** · nice-to-have, not pulling user demand right now.
- **Tray-icon pulse animation** (was #7). Replaced by the 6-second spinning electron shipped in #14. Pulse pattern dropped in favour of a one-shot attention sweep on first launch.
