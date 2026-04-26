# Niyora

**Calm in 60 seconds.** A privacy-first macOS menu-bar app that nudges you to take a breath when you've been at the screen too long, then guides you through a one-minute practice grounded in published breathwork research.

For founders, sales, devs, PMs, designers — anyone whose brain needs a reset between the next thing.

## Why Niyora

Most wellness apps require accounts, send your data to a server, and expect you to remember to open them. Niyora is different on every count:

- **Nothing leaves your Mac.** No accounts. No tracking. No analytics. No telemetry. The app makes zero outbound network requests of its own.
- **It nudges you, not the other way around.** A background loop watches passive signals (continuous screen time, time of day, after-hours work) and sends a soft system notification when you need a break. You don't have to remember.
- **Open source.** Verify the privacy promise yourself.

## What it does

- Lives quietly in your macOS menu bar
- Click the icon to start a guided one-minute practice
- 14 practices total: 7 pranayama breathing techniques, 7 mindfulness moments
- Smart reminders during work hours (9am–6pm) based on screen-time signals
- Two notification action buttons: **Breathe now** opens the panel; **Snooze 30 min** pushes the next reminder
- A "My Soul" panel tracks your practice over time and unlocks more advanced techniques as you go (Spark → Glow → Shine → Radiance → Brilliance)
- Optional weekly PSS-4 mental-health check-in (Cohen 1983, validated 4-item perceived stress scale) with a sparkline showing your scores over time

## Practices included

**Breathing:** Box Breath, Ocean Breath (Ujjayi), Cooling Breath (Sheetali), Alternate Nostril (Naadishodhana), Left Nostril, Belly Breath (Diaphragmatic), Wind Down (4-7-8).

**Mindfulness:** Be Kind to Yourself (self-compassion), Let It Drift (CBT thought defusion), Bring Someone to Mind (gratitude), Hold Yourself (somatic), Kind Words (affirmation), Five Senses (5-4-3-2-1 grounding), Soft Gaze (Trataka).

## Install

### Easy way (recommended)

Download the latest signed `.dmg` from [niyora.com](https://niyora.com) or the [Releases](https://github.com/neha-prasad-ux/niyora/releases) page. Drag Niyora into Applications. Done.

### Build from source

You'll need:

- **macOS** (Apple Silicon or Intel)
- **Rust** via [rustup](https://rustup.rs/)
- **Node.js** 18+ and **pnpm**
- **Xcode Command Line Tools** (`xcode-select --install`)

```bash
git clone https://github.com/neha-prasad-ux/niyora.git
cd niyora
pnpm install

# Run the dev build (live reload, devtools, debug menu)
pnpm tauri dev

# Build a release .app bundle
pnpm tauri build
```

The release bundle lands in `src-tauri/target/release/bundle/macos/Niyora.app`.

## How it works

| Layer | Tech |
|---|---|
| App framework | [Tauri 2](https://v2.tauri.app/) |
| Window style | macOS NSPanel (via [tauri-nspanel](https://github.com/ahkohd/tauri-nspanel)) — a true menu-bar popover |
| Backend | Rust |
| Frontend | React 18 + TypeScript |
| Build tool | Vite |
| Notifications | [`mac-notification-sys`](https://crates.io/crates/mac-notification-sys) (used directly for inline action buttons) |
| Storage | Append-only JSONL files in `~/Library/Application Support/com.niyora.breathing/` |

### Repository structure

```
niyora/
├── src/                          # React frontend
│   ├── App.tsx                   # Top-level routing (main / settings / mood / pss4 / onboarding)
│   ├── BreathingSession.tsx      # Canvas-based session UI, particle animations, pause logic
│   ├── Settings.tsx              # "My Soul" panel
│   ├── Onboarding.tsx            # First-launch flow
│   ├── PssFour.tsx               # Weekly check-in
│   ├── PostSessionMood.tsx       # After-breath mood capture
│   ├── techniques.ts             # All 14 practices + tier-unlock metadata
│   ├── tiers.ts                  # Spark → Brilliance progression
│   ├── useSnapshot.ts            # Situational snapshot hook + colour helpers
│   ├── useSessionStats.ts        # Lifetime session count
│   └── DevControls.tsx           # Dev-only stress-tier preview (tree-shaken in prod)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs               # Tray, panel, event wiring
│   │   ├── reminder.rs           # Background reminder loop + macOS notifications
│   │   ├── situational/          # Passive signal collection (idle, mic, app switches, keystrokes)
│   │   ├── sessions.rs           # Local session log (JSONL)
│   │   ├── analytics.rs          # Local event log (JSONL)
│   │   ├── config.rs             # App data dir + config persistence
│   │   └── onboarding.rs         # First-launch state
│   └── tauri.conf.json
├── docs/                         # Landing page + privacy policy + terms (GitHub Pages)
└── tests/e2e/                    # Playwright UI tests
```

## Privacy

Read the full [privacy policy](https://niyora.com/privacy/). The short version:

- **Local data only.** Sessions, events, config — all written to your app's container at `~/Library/Application Support/com.niyora.breathing/`. Never read off-device.
- **No accounts, no telemetry, no analytics.**
- **No outbound network requests** from the app. Fonts are bundled.
- The app reads passive system signals (screen idle time, frontmost app bundle ID, mic device state, system-wide event counters) to time reminders. None of this is recorded, logged, or transmitted — it's all in-memory and used only to compute the reminder interval.

## Contributing

PRs welcome — see [CONTRIBUTING.md](./CONTRIBUTING.md). Security issues: see [SECURITY.md](./SECURITY.md).

## License

[MIT](./LICENSE) — Copyright © 2026 Neha Prasad.
