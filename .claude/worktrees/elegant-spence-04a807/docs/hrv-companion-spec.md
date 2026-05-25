# HRV Companion App · Spec & Architecture Plan

Status: proposed · Track: parallel (does not block macOS polish) · Roadmap: item #16

## Why this exists

Niyora claims breathing reduces stress. Right now we cannot prove it. Heart Rate
Variability (HRV) is the most credible consumer biomarker for nervous-system
state: it rises as the body shifts toward "rest and digest." If we can show a
user their HRV before a session vs after, we turn a claim into evidence.

The blocker: HealthKit, the only sanctioned way to read HRV, does not exist on
macOS. The Niyora Mac app cannot read it. So HRV requires a small companion app
on a device that *does* have HealthKit: an iPhone (which automatically receives
HRV samples its paired Apple Watch records).

## What we are building

Two pieces:

1. **Niyora Companion (iOS app).** Reads HRV from HealthKit, listens for session
   windows from the Mac, computes a before/after delta, sends the result back.
2. **Mac-side contract.** A local endpoint on the Niyora Mac app that announces
   itself, sends session windows, and receives HRV results. Plus a "My Soul"
   view that shows the impact.

A watchOS app is explicitly *out of scope for v1*. The Watch already writes HRV
into HealthKit on its own schedule; the iPhone can read it. Building a watchOS
target to take on-demand readings is a real accuracy upgrade but doubles the
surface area. Park it.

## Privacy stance (non-negotiable)

This product's promise is "no cloud, no user data leaves the device." The
companion app must hold that line:

- HRV samples never leave the user's devices. Sync is **local network only**
  (Bonjour / `NWConnection`), Mac to iPhone, same wifi.
- No analytics SDK, no crash reporter that ships data off-device, no account.
- HealthKit access is **read-only** and scoped to exactly one type: HRV (SDNN).
  Nothing else. (Resting heart rate and other stress-correlated signals exist,
  but v1 deliberately stays single-metric for a tight privacy ask and a simpler
  story.)
- The Mac stores only derived numbers (a delta and a timestamp), never raw
  HealthKit data.
- If the phone is not reachable, the feature degrades silently. It never blocks
  a breathing session and never nags.

## Data flow

```
  [Apple Watch] --HRV--> [iPhone HealthKit]
                               |
                               | (3) read HRV in window
                               v
  [Mac: Niyora] --(1) advertise via Bonjour-->  [iPhone: Niyora Companion]
  [Mac: Niyora] --(2) send session window----->
  [Mac: Niyora] <--(4) receive {pre, post, delta}--
       |
       v
  (5) store delta + show in "My Soul"
```

1. **Discovery.** Mac app advertises a Bonjour service (e.g.
   `_niyora._tcp`). Companion app browses for it. First-time pairing shows a
   short code on the Mac that the user confirms on the phone, so a stranger on
   the same wifi cannot connect.
2. **Session window.** When a user finishes a breathing session, the Mac sends
   the companion a window: `{ session_id, start, end }`. The "pre" baseline is
   the 5 minutes before `start`; the "post" is the 5 minutes after `end`.
3. **HRV read.** Companion queries HealthKit for HRV (SDNN) samples in the pre
   and post windows. HRV is sparse: the Watch may log only a few samples an
   hour. If a window has zero samples, the result is `unavailable` (see "Honest
   gaps" below).
4. **Result.** Companion sends back `{ session_id, pre_ms, post_ms, delta_ms,
   sample_counts, status }`.
5. **Display.** Mac stores the delta against the session and surfaces it in
   "My Soul": per-session and as a trend.

## The sync layer

- **Transport:** Bonjour for discovery + `NWConnection` (Network.framework) for
  a direct TCP link. No HTTP server, no third-party networking library.
- **Pairing:** one-time, code-confirmed. Store a shared secret in the Keychain
  on both ends; reject unpaired peers.
- **When devices are apart:** the Mac queues unsent session windows (cap the
  queue, e.g. last 50). When the companion reconnects, it drains the queue,
  reads historical HealthKit data for those windows, and backfills. HealthKit
  keeps history, so a delayed read is still accurate.
- **Failure is normal, not exceptional.** Phone off, different wifi, app not
  launched — all expected. The Mac UI shows "waiting for your phone" rather
  than an error.

## Honest gaps (design for them, do not hide them)

HRV is a noisy, sparse signal. A spec that pretends otherwise will ship a
feature that lies to users.

- **Sparse samples.** A 5-minute window may contain zero HRV samples. Show
  "not enough data this time," never a fabricated number.
- **Single sessions are noisy.** One before/after delta means little. The
  honest unit of impact is a **trend over many sessions** — e.g. average post
  vs pre delta over the last 30 sessions. Lead with the trend; show the single
  delta as secondary, with a clear "one reading, take it lightly" framing.
- **Confounders.** Caffeine, exercise, posture, time of day all move HRV more
  than one breathing session will. We cannot control for these. The copy must
  not over-claim. "Your HRV tended to rise after sessions" is honest;
  "breathing raised your HRV by X" is not.
- **No Watch, no feature.** Users without an Apple Watch get nothing from this.
  The companion app must detect that and say so plainly rather than showing an
  empty screen.

## Build milestones

Each milestone is independently demoable. Sequence assumes one iOS developer.

| # | Milestone | Outcome |
|---|---|---|
| 1 | iOS app skeleton + HealthKit permission flow | App installs, asks for HRV read access, shows whether access was granted. |
| 2 | HealthKit HRV read for an arbitrary window | Given a start/end, app prints HRV samples and a window average. Proves the data path. |
| 3 | Mac-side: Bonjour advertise + session-window send | Mac announces itself and emits a window on session end. Verifiable with a network sniffer. |
| 4 | Companion: discover + pair + receive window | Phone finds the Mac, pairs with a code, receives a real session window. |
| 5 | End-to-end: window in, delta out | Finish a session on the Mac, see `{pre, post, delta}` arrive back. The core loop works. |
| 6 | Offline queue + backfill | Sessions done with the phone away are reconciled when it reconnects. |
| 7 | "My Soul" impact view on the Mac | Trend chart + latest delta, with the honest-gaps framing. |
| 8 | Polish: pairing UX, no-Watch state, App Store assets | Shippable. |

Milestones 1-2 are the de-risking spike: if HealthKit HRV reads are flakier
than expected, we learn it before building the sync layer.

## What the founder needs to have in place

This is the part that gates *starting*, not just finishing:

- An **Apple Developer Program** membership that covers iOS (the standard $99/yr
  membership does).
- **Xcode** on a Mac.
- A **physical iPhone + Apple Watch** for testing. The simulator cannot produce
  HRV.
- An **iOS developer** to build it, or a decision to learn Swift. This is a
  separate skill set from the Tauri/Rust/React stack the Mac app uses.

## Open decisions (resolve before milestone 3)

1. **Pairing model.** Code-confirmed once (proposed) vs. QR vs. trust-any-peer
   on the local network. Proposed is the privacy-safe default.
2. **Where the delta lives.** On the Mac only (proposed) vs. mirrored on the
   phone. Mac-only keeps one source of truth.
3. **Window size.** 5 min pre / 5 min post (proposed). Larger windows catch
   more samples but blur the attribution to the session.
4. **Trend math.** Simple mean of deltas vs. something that weights recent
   sessions. Start simple; revisit with real data.
5. **App Store positioning.** Is the companion a standalone listing, or do we
   wait until it is polished enough to not confuse users who find it alone?

## Explicitly out of scope for v1

- watchOS app with on-demand HRV readings.
- Any cloud sync or account.
- HRV-driven changes to reminder timing or technique selection (that is a later
  roadmap item once we trust the data).
- Android / Wear OS equivalents.
