# Cross-Device Flow · Mac ↔ iPhone (+ Watch) · Master Spec

Status: proposed · Track: parallel (does not block macOS polish) · Relates to:
`mac-mobile-sync-spec.md` (sync layer), `hrv-companion-spec.md` (HRV), roadmap
#14 (smarter reminders), #16/#19 (HRV), iOS v1 (niyora-companion#9)

This is the umbrella spec. It defines the whole user journey across devices and
the shared model both apps speak. The two specs above are the component-level
detail for the sync transport and HRV; this doc is how they fit into one product.

---

## 1. The product in one paragraph

Niyora lives on the user's Mac and iPhone. Each device senses pressure in the way
only it can. The Mac watches the work day (screen time, meetings, context
switching, late hours). The phone watches phone usage and, when an Apple Watch is
present, heart-rate variability. The two devices share one **soul-state** over the
local wifi, never the cloud. When the day reads heavy, the device the user is on
offers a breath. Nothing about the user ever reaches Niyora's servers.

---

## 2. What is already built (grounding)

Do not re-build these. The spec extends them.

**Mac app** (`app/`, Tauri 2 · Rust · React · TS):
- Situational collectors (`src-tauri/src/situational/`) produce a 0-100 index and
  a `DayLabel`: `calm` (80-100) · `normal` (60-79) · `dense` (40-59) · `heavy`
  (0-39), from screen time, meeting density, app-switch rate, keystroke pace,
  after-hours. Polled every 30s.
- Reminder loop (`src-tauri/src/reminder.rs`) fires notifications via
  `mac-notification-sys` (inline action buttons: "Breathe now" / "Snooze").
- My Soul panel (`src/Settings.tsx`), tiers (`src/tiers.ts`: spark · glow · shine
  · radiance · brilliance, by completed-session count), techniques
  (`src/techniques.ts`).
- Opt-in telemetry only (`telemetry.rs`, PostHog EU, allow-listed events). No
  user data, no account.

**iPhone app** (`ios-v1/`, Expo · React Native · TS):
- Expo Router screens: `src/app/index.tsx` (home orb), `session.tsx` (breath),
  `my-soul.tsx`. Tiers + techniques ported to match the Mac exactly.
- Session history in `src/store/session-history.ts` (AsyncStorage, records
  `{ techniqueId, completedAt }`).
- No networking, no HRV, no Screen Time yet. Standalone and offline.

**Existing component specs:** the sync layer (Bonjour + `NWConnection`, derived
state only) and the HRV companion (HealthKit read, pre/post delta).

> Stack decision (2026-06-06): **the canonical iPhone app is the Expo app,
> `niyora-ios`** (not the SwiftUI `niyora-companion`). HRV / Screen Time / sync
> arrive as native modules added to it in waves. Section 9 covers the cost.
>
> **Reuse, do not rebuild from zero.** The retired SwiftUI app
> (`niyora-companion`, mirrored at `niyora/ios/NiyoraCompanion/`) already contains
> working, validated implementations of most of this: camera PPG
> (`PPGCapture.swift`, `PPGSignalProcessor.swift` — a real detrend → bandpass
> [0.7-4.0 Hz] → refractory peak-detect → RMSSD/SDNN pipeline with an SNR gate and
> HR sanity check), Watch HRV (`HealthKitManager.swift`), QR pairing + local sync
> (`QRScannerView.swift`, `PairingFlow.swift`, `MacConnection.swift`,
> `Protocol.swift`, `KeychainStore.swift`, `KnownServerStore.swift`), and the
> before/after measure loop (`MeasurementController.swift`, `HRVSpikeView.swift`).
> Port the **algorithms and the wire protocol** from there into Expo; do not
> reinvent them. This satisfies the "use validated, do not build from zero" rule
> (section 8c) directly from proven in-house code.

---

## 3. The shared model: one soul-state

Both devices compute and exchange the same object. This is the contract.

```
SoulState {
  label:  "calm" | "normal" | "dense" | "heavy"   // the canonical 4 states
  index:  0..100                                   // finer score behind the label
  source: "mac" | "phone" | "self" | "hrv"         // what produced this reading
  ts:     ISO-8601 timestamp                        // when it was computed
}
```

Rules:
- **Each device always has its own soul-state** from its own signals. It works
  alone.
- When paired and near, devices exchange their latest `SoulState`. The displayed
  state is **reconciled** (section 6).
- **A human self-report beats any inference.** A recent `source: "self"` wins.
- **HRV enriches, it does not override.** An HRV reading nudges the index
  (section 8), it is not a standalone label.
- The wire payload is derived state only. Raw inputs (screen-time minutes,
  meeting lists, keystroke counts, raw HRV samples) never cross the wire.

---

## 4. User flow A · Mac first (primary)

The expected path. The Mac is the richer device and the natural first install.

| Step | What the user does | What the app does | Design notes |
|---|---|---|---|
| A1 | Downloads `.dmg` from niyora.com, opens | Standard install | Smoother installer is roadmap #2 |
| A2 | Completes Mac onboarding | Existing onboarding, then a new final screen | See A-final below |
| A3 | Sees "Carry Niyora with you", points iPhone camera at a **QR code** | QR opens the App Store listing on the phone | One scan. You cannot push an install; a QR to the App Store is the lever |
| A4 | Taps Get on the App Store, opens the app | Phone launches, browses for `_niyora._tcp` on the wifi | First launch is fully usable even if pairing is skipped |
| A5 | Confirms a short **pairing code** shown on the Mac | Devices store a shared secret in the keychain on both ends | Code-confirmed once. A stranger on the same wifi cannot pair |
| A6 | Nothing | Steady state: synced, both notify, both offer breath | Section 6-7 |

**A-final (Mac onboarding last screen):** a calm screen with the QR, one line of
copy ("Niyora, in your pocket too."), and a clear skip. No instructional
paragraph. Pairing can also be reached later from Mac Settings, so skipping is
safe.

---

## 5. User flow B · iPhone first (fallback)

Someone discovers the App Store listing before the Mac app. Must work, but the
Mac is web-distributed (not on the Mac App Store), so the handoff is a link, not a
QR-to-store.

| Step | What the user does | What the app does | Design notes |
|---|---|---|---|
| B1 | Installs from App Store, opens | Fully standalone: self-report + (opt-in) Screen Time + (opt-in) HRV | Never blocks on a Mac |
| B2 | Later sees "Niyora is calmer with your Mac" | Shows a link / QR to niyora.com/mac | Soft, dismissible, not day-one |
| B3 | Opens niyora.com/mac on the Mac, installs | Mac install as in flow A | |
| B4 | Pairs as in A5 | Same pairing | |

Design rule: flow B never makes the phone feel broken without a Mac. The Mac is an
upgrade, not a requirement.

---

## 6. Sync + reconciliation (steady state)

Transport detail lives in `mac-mobile-sync-spec.md`. Summary and the
reconciliation logic that belongs here:

- **Transport:** Bonjour discovery + `NWConnection` (iOS) / mDNS + TCP (Mac).
  Local network only. No cloud, no account.
- **Exchange:** on connect and on change, each device sends its latest
  `SoulState`. Small, idempotent.
- **Reconciliation (what the user sees on each device):**
  1. If a `source: "self"` reading exists within the freshness window, use it.
  2. Else if an HRV-enriched reading exists and is fresh, use it.
  3. Else use the **more pressured** of {my own state, the peer's state}, as long
     as the peer's is fresh. (Pressure prevention errs toward offering calm.)
  4. Else use my own state.
- **Freshness window:** a peer reading older than the window is ignored and the
  device falls back to its own. Proposed 90 minutes; revisit with use.
- **Apart is normal.** Phone off, different wifi, app closed: each device shows
  its own state. The UI shows last-known peer state with its age, never an error.

---

## 7. Notifications (no nagging in stereo)

Both devices can notify on a heavy state. When **paired and both reachable**, they
must not both fire for the same moment.

- **Active-device rule:** the device the user is currently active on owns the
  notification; the other stays silent. "Active" = most recent foreground / input
  within a short window.
- **If neither is clearly active** (e.g. both idle), the **phone** owns it, since
  it is the device most likely on the user's person.
- **Cooldown is shared:** a breath taken on either device resets the reminder
  clock on both. Finishing a session on the phone should not leave the Mac about
  to nag.
- Reuse the Mac's existing reminder cadence and copy; the phone mirrors it.

---

## 8. HRV on the phone (two sources)

HRV makes the soul-state physiological, not just behavioural. The phone has two
ways to get it, and they are complementary, not competing.

### 8a. Phone camera (everyone, on-demand)
A fingertip over the rear camera with the flash on. The app reads the colour
pulses frame by frame (photoplethysmography / PPG), detects beats, derives HRV.
No Watch, no extra hardware: this works on any iPhone, which removes the
"no Watch, no feature" gap entirely.
- **On-demand, active.** The user holds still with a finger on the lens for ~1 to
  3 minutes. Framed as a calm reading taken **before and after a session**, so the
  friction becomes part of the ritual rather than a chore.
- **Heart rate is reliable; HRV from a camera is the least accurate source.** A
  30-60fps camera plus finger movement limits beat-timing precision. Good for
  *relative* before/after trends, never a clinical figure. The copy must say so.
- **Privacy:** frames are processed on-device in real time and never stored or
  sent. Needs only a camera permission, not HealthKit.
- Produces a pre/post delta (the evidence story) and an on-demand reading.
  `source: "hrv"`.

### 8b. Apple Watch via HealthKit (Watch owners, passive)
When a paired Watch is present, HealthKit already holds HRV samples it recorded.
- **Ambient enrichment.** Whenever fresh samples exist, fold them into the phone's
  `SoulState` with no user action: low HRV vs the user's own rolling baseline
  nudges the index toward `heavy`, high toward `calm`.
- **More accurate than the camera, but sparse** (the Watch logs only a few samples
  an hour) and Watch-only.
- Also supports the before/after delta around a session (existing
  `hrv-companion-spec.md`).

### Honest gaps (apply to both)
- HRV is noisy. A reading may fail or a window may have zero samples: show "not
  enough this time", never a fabricated number.
- Baseline beats absolutes. Enrich against the user's own rolling baseline, not
  population norms. The camera and Watch baselines are tracked separately (their
  noise floors differ); do not mix the two sources into one number.
- A single reading means little. Lead with the trend over many sessions, show one
  delta as secondary with a "take it lightly" framing.
- Raw HRV and camera frames never leave the phone. Only the derived `SoulState`
  (label + index) may sync to the Mac.

### 8c. Camera HRV implementation directive (read before building)

**Do not build the signal processing from zero. Reuse validated, published
algorithms.** Camera fingertip HRV is a solved research problem; the accuracy
comes from using methods that have been validated against ECG, not from inventing
new ones. The job is assembly and faithful porting, not invention.

What is validated, and what to reuse:
- **Heart rate from fingertip PPG is highly accurate** against ECG (r ≈ 0.997,
  error ≈ 1 bpm). Counting beats is the reliable part.
  ([validation study](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC5368348/))
- **HRV (RMSSD) is achievable** with the accepted bar being under ~20 ms RMSSD
  error, reached with a still finger, contact, and the flash on. **Fingertip
  contact PPG is more accurate than face-video (rPPG) for HRV, so use the
  finger-on-lens method, not the camera-pointed-at-face method.**
  ([real-time camera HR/HRV](https://arxiv.org/pdf/1909.01206))

The pipeline, two layers, both with proven open source:

| Layer | Job | Reuse (do not reinvent) | License note |
|---|---|---|---|
| Capture | Camera frames → raw PPG waveform (mean red-channel per frame, flash on) | `react-native-vision-camera` frame processor for the capture; reference [PPGbetter](https://github.com/JanBancerewicz/PPGbetter) and [OpenPPG](https://medium.com/@bgallois/open-source-ppg-heart-rate-monitoring-empowering-users-with-raw-data-access-8baced61f3c8) for the technique | Permissive / reference only |
| HRV math | Waveform → beat detection → RMSSD/SDNN | Port the algorithm from [HeartPy](https://github.com/paulvangentcom/heartrate_analysis_python) or [pyHRV](https://github.com/PGomes92/pyhrv) (peak detection, inter-beat intervals, time-domain HRV) | HeartPy / pyHRV are MIT-family. **AVOID [pyVHR](https://github.com/phuselab/pyVHR): it is GPL-3.0 and incompatible with a closed-source app.** Use the MIT-family toolkits as the algorithm reference. |

Hard rules for the implementer (Alfred):
1. **Validated methods only.** Use the documented HeartPy/pyHRV algorithms for
   peak detection and RMSSD. Do not hand-roll a new peak detector.
2. **Verify the licence before copying any code.** Reference GPL projects for
   understanding; copy code only from MIT-family sources.
3. **Apply the standard noise handling** (band-pass filter, motion rejection) the
   papers describe; motion artefacts are the main error source.
4. **Calibrate against ground truth** before shipping: compare RMSSD output to a
   known-good source (a chest strap or a validated app) and confirm the error is
   within the ~20 ms bar. If it is not, fix the pipeline, do not relax the claim.
5. **Be honest in the UI.** Present a relative before/after trend, never a clinical
   number. Show "not enough this time" on a failed reading.

---

## 9. Technical requirements

### Mac side (Rust / Tauri)
- Add a sync service: mDNS advertise (`_niyora._tcp`) + a TCP listener for
  `SoulState` exchange and pairing. New module under `src-tauri/src/sync/`.
- Pairing: code generation + shared-secret storage in the macOS Keychain.
- Wire the situational `DayLabel`/index into a `SoulState` producer.
- Notification coordination: extend `reminder.rs` to respect the active-device
  rule and shared cooldown.
- My Soul: show peer presence and (later) the synced/HRV-enriched state.
- Keep the privacy posture: no new outbound network except local-network peer.

### iPhone side (Expo / React Native) · the real cost
The current app is plain Expo. Every capability below needs a **native module and
a custom dev build (EAS), not Expo Go.** This is the main engineering lift.

| Capability | iOS framework | What it needs in Expo | Entitlement / Info.plist |
|---|---|---|---|
| Local-network sync | Network.framework (`NWConnection`) + Bonjour | Custom native module (or a vetted TCP/mDNS RN lib) | `NSLocalNetworkUsageDescription`, `NSBonjourServices` (`_niyora._tcp`) |
| Screen Time signal | DeviceActivity + FamilyControls | Custom native module **plus a DeviceActivityMonitor app-extension target**; config plugin to add the target | **FamilyControls entitlement (requested from Apple)**, `NSUserTracking…` not needed |
| HRV via camera | Camera + real-time frame processing (PPG) | `react-native-vision-camera` frame processor, or a custom native module; on-device beat detection + RMSSD/SDNN math | `NSCameraUsageDescription`; no HealthKit needed for the raw read |
| HRV via Watch | HealthKit | `react-native-health` or custom module | HealthKit entitlement, `NSHealthShareUsageDescription`; read-only, scoped to HRV (SDNN) + baseline data |

Implications to plan around:
- **Expo Go is out** the moment any of these lands. Move to EAS dev/build client.
  TestFlight/EAS is already set up (see iOS TestFlight memory).
- **FamilyControls needs Apple's approval** of the entitlement request before the
  Screen Time signal can ship. Start that request early; it gates the milestone.
- **The DeviceActivity extension** is a second build target. Plan the Expo config
  plugin work; it is the fiddliest piece.
- **App-switch rate and notification volume stay Mac-only.** iOS exposes no API
  for them. Do not promise them on the phone.
- Persist `SoulState` and HRV-derived values locally (extend the existing
  AsyncStorage store). Nothing leaves the device except the derived `SoulState`
  to a paired Mac.

### Shared
- One `SoulState` schema, versioned on the wire so the two apps can evolve.
- Pairing secret in each platform's secure store (Keychain).

---

## 10. Build sequence (milestones)

Each is independently demoable. Native iOS modules are the long pole; sequence so
the cheap wins land first and the entitlement request runs in parallel.

| # | Milestone | Outcome |
|---|---|---|
| 0 | iOS: move to EAS dev client | App runs as a custom dev build, ready for native modules |
| 1 | Mac onboarding QR + niyora.com/mac handoff | Both install paths work end to end (no sync yet) |
| 2 | Sync layer: advertise + pair + exchange `SoulState` | Mac and phone show each other's state when near |
| 3 | Reconciliation + freshness | One coherent state per device; self-report wins |
| 4 | Notification coordination | No double-notify when paired; shared cooldown |
| 5 | iOS self-report check-in | One-tap feeling feeds + syncs `SoulState` |
| 6 | iOS Screen Time signal (DeviceActivity) | Phone usage + late-night buckets enrich the phone state (gated on Apple entitlement) |
| 7 | iOS HRV via camera (PPG) | Anyone can take a before/after reading with a finger on the lens. No Watch needed |
| 8 | iOS HRV via Watch (HealthKit) | When a Watch is present, HRV also enriches the phone state passively |
| 9 | HRV before/after impact, unified | Pre/post delta per session from whichever source is available (the evidence story) |
| 10 | Data visualisation v1 | Trends across both devices in My Soul (section 11) |

Milestones 1-5 need no special Apple entitlement and deliver the core "synced
across my devices" promise. Camera HRV (7) needs only a camera permission, so it
is the cheapest enrichment win and reaches every user; Screen Time (6) and Watch
HRV (8) need Apple entitlements. 6-9 are the enrichment layer; 10 is the payoff.

---

## 11. Future · data visualisation

Once the soul-state is shared and enriched, the data becomes worth showing. Out of
scope to build now, in scope to design toward:

- **One soul timeline.** A calm, honest view of how the day/week felt, blending
  behavioural state, self-report, and (when present) HRV, across both devices.
- **Impact over time.** HRV before/after as a trend, not a single number, with the
  honest framing ("tended to rise after sessions", never "breathing raised your
  HRV by X").
- **Tier + practice woven in.** The existing My Soul tiers (spark → brilliance)
  shown alongside how practice tracked against pressure.
- **Still no cloud.** Visualisation reads only on-device and synced-local data.
  The chart is computed where the data lives.

Design principle for all of it: show rhythm and trend, never a precise stress
"score" the data cannot honestly support.

---

## 12. Open decisions
1. Freshness window length (proposed 90 min).
2. Active-device detection: how recent is "active", and the tie-break when both
   are idle (proposed: phone owns it).
3. Self-report shape: pre-session, post-session, or both.
4. HRV baseline math: rolling window length, how strongly HRV nudges the index,
   and keeping the camera and Watch baselines separate.
4b. Camera HRV reading length (proposed 1-3 min) and whether it is offered as a
   standalone reading, only around sessions, or both.
5. Whether self-report and HRV-enriched state also write to the Mac's My Soul
   trend, or stay phone-local (one source of truth vs simplicity).
6. Timing of the FamilyControls entitlement request (recommend: start before
   milestone 2, since approval is the long pole).

## 13. Explicitly out of scope
- Any cloud sync, account, or server-side data (local network only, decided).
- Calendar / EventKit as a signal (dropped).
- App-switch rate / notification volume on iPhone (no API exists).
- Reading exact Screen Time minute counts (Apple seals them; buckets are enough).
- watchOS app with on-demand HRV (parked, roadmap #19).
- Surveillance-style inference on either platform.
