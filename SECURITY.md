# Security policy

## Reporting a vulnerability

If you find a security issue in Niyora, please **do not open a public GitHub issue**. Email the details to:

**neha@niyora.com**

Include:

- A description of the vulnerability and its potential impact
- Steps to reproduce
- The version of Niyora affected (see "About" or the build number)
- Your name (for credit, if you'd like)

We aim to acknowledge reports within 72 hours and to ship a fix within 30 days for critical issues. Once a fix is released, we'll credit reporters in the release notes unless you'd prefer to remain anonymous.

## Scope

In scope:

- The Niyora macOS app (Rust backend, React frontend)
- The signed release builds distributed via niyora.com / GitHub Releases
- The local data files written under `~/Library/Application Support/com.niyora.breathing/`

Out of scope:

- The niyora.com landing page lives in a separate repo ([neha-prasad-ux/niyora-web](https://github.com/neha-prasad-ux/niyora-web)) and is hosted on Cloudflare Pages. Report site-only issues there.
- Issues in third-party dependencies (please report upstream)

## What Niyora does and doesn't do

Niyora reads passive system signals (screen idle time, frontmost app, mic device state, system-wide event counters) to time reminders. None of this is stored or transmitted. All session data is written locally to your app's container.

The app makes outbound network requests in exactly three cases, all to first-party endpoints with no telemetry attached:

1. **Update checks** — a small JSON fetch to `downloads.niyora.com/latest.json` on launch, every ~24h while running, and when you click "Check for Updates" in the tray menu. Release builds only.
2. **Update downloads** — when (1) finds a new version, the signed binary is fetched from `downloads.niyora.com`. Manual / launch-time checks prompt you first; the periodic check downloads silently.
3. **Anonymous analytics** — only if you opted in on the onboarding consent slide. See [telemetry.rs](src-tauri/src/telemetry.rs) for the exact event list.

If you find a way to make Niyora exfiltrate user data or violate any of these guarantees, that's a critical issue. Please report it.
