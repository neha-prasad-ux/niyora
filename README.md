# Niyora

A macOS menu bar app for mindful breathing exercises, built with [Tauri v2](https://v2.tauri.app/) + React + TypeScript.

## What it does

Niyora lives quietly in your macOS menu bar. Click the icon and a small popover appears with a guided **box breathing** exercise — 4 seconds inhale, 4 seconds hold, 4 seconds exhale, 4 seconds hold. Four rounds, about two minutes, and you're back to your day feeling calmer.

It also gently nudges you every 2 hours during work hours (9 AM - 6 PM) with a notification reminder to breathe.

## V1 Features

- **Menu bar only** — no dock icon, no distracting windows
- **Box breathing exercise** — 4-phase guided breathing with visual countdown
- **4 rounds** (~2 minutes) per session
- **Periodic reminders** — macOS notifications every 2 hours during work hours
- **Minimal design** — dark background, clean typography, zero clutter

## Tech Stack

| Layer | Technology |
|-------|-----------|
| App framework | [Tauri v2](https://v2.tauri.app/) |
| Backend | Rust |
| Frontend | React + TypeScript |
| Build tool | Vite |
| Tray positioning | tauri-plugin-positioner |

## Prerequisites

To build and run Niyora locally, you'll need:

- **macOS** (this is a macOS-only app)
- **Rust** (install via [rustup](https://rustup.rs/))
- **Node.js** 18+ and **pnpm** (or npm/yarn)
- **Xcode Command Line Tools** (`xcode-select --install`)

## Getting Started

```bash
# Clone the repo
git clone https://github.com/neha-prasad-ux/niyora.git
cd niyora

# Install frontend dependencies
pnpm install

# Run in development mode
pnpm tauri dev

# Build for production
pnpm tauri build
```

## Project Structure

```
niyora/
├── src/                    # React frontend
│   ├── App.tsx             # Box breathing UI
│   ├── App.css             # Styles
│   ├── main.tsx            # React entry point
│   └── index.html          # HTML template
├── src-tauri/              # Tauri/Rust backend
│   ├── src/
│   │   └── main.rs         # Tray icon, popover, notifications
│   ├── icons/              # App and tray icons
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── package.json            # Node dependencies
├── vite.config.ts          # Vite bundler config
└── tsconfig.json           # TypeScript config
```

## How It Works

1. **Tray icon** — Tauri registers a system tray icon on launch
2. **Click to open** — Left-clicking the icon toggles a small popover window (300x400px) positioned just below the menu bar
3. **Breathing guide** — The React UI walks you through 4 rounds of box breathing with a visual countdown
4. **Auto-hide** — Click "Done" or complete the exercise and the popover closes
5. **Reminders** — A background thread sends macOS notifications every 2 hours during 9 AM - 6 PM

## License

MIT
