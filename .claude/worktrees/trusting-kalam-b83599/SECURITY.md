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

Niyora makes no outbound network requests from the app. It reads passive system signals (screen idle time, frontmost app, mic device state, system-wide event counters) to time reminders, but none of this is stored or transmitted. All session data is written locally to your app's container.

If you find a way to make Niyora exfiltrate user data or violate any of these guarantees, that's a critical issue — please report it.
