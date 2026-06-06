# Mac ↔ Mobile Sync · Enriching the Mobile Soul-State · Spec

Status: proposed · Track: parallel (does not block macOS polish) · Relates to: roadmap #14 (smarter reminder timing), iOS v1 (niyora-companion#9)

## Why this exists

The Mac app already knows when a user is under pressure. The situational
collectors (`src-tauri/src/situational/`) compute a 0-100 index and a
`DayLabel` (calm · normal · dense · heavy) from screen time, meeting density,
app-switch rate, keystroke pace, and after-hours work. That signal already
drives reminder timing and the "My Soul" message on the Mac.

The iPhone app knows almost nothing. iOS v1 is Breath + My Soul only. It cannot
read the same signals (iOS sandboxes hard: Screen Time data cannot be exported,
there is no equivalent to the Mac's app-switch or keystroke collectors). So the
phone shows a generic experience while the Mac, sitting on the user's desk, holds
a live read on their day.

This feature closes that gap two ways:

1. **Carry the Mac's soul-state to the phone.** When the two devices are
   together, the Mac shares its current `DayLabel` so the phone's reminders and
   My Soul reflect the real day instead of a default.
2. **Let the phone add a consented self-report.** The phone contributes the one
   signal the Mac cannot get: how the user actually feels, via a one-tap
   check-in. That flows back so both devices agree on one soul-state.

## What we are building

Two pieces, mirroring the HRV companion split (see `hrv-companion-spec.md`):

1. **Mobile side (iOS).** Receives the Mac's soul-state over the local network.
   Uses it to colour reminders and My Soul. Adds a one-tap check-in and sends
   that back.
2. **Mac side.** Advertises and shares its current `DayLabel` + index. Receives
   the phone's check-in and folds it into the displayed state.

What flows between them is **derived state only**: a label, an index number, a
timestamp, an optional self-reported feeling. Never the raw inputs (no screen-time
minutes, no meeting list, no keystroke counts leave the Mac).

## Privacy stance (non-negotiable)

Same line as the rest of the product. No cloud, no account, nothing leaves the
user's devices.

- **Local network only.** Bonjour discovery + `NWConnection`, Mac to iPhone, same
  wifi. No server, no CloudKit, no third-party networking library. (Matches the
  HRV spec; reuse that transport layer if it ships first.)
- **Derived state, not raw signals.** The wire payload is `{ label, index,
  source, ts }` plus an optional `{ feeling, ts }` back. The Mac's raw collector
  inputs never cross the wire.
- **No analytics, no crash reporter that ships data, no account.**
- **Self-report is opt-in and minimal.** The check-in is one tap, optional, never
  a nag, no streak guilt. The whole consent direction below applies.
- **Degrades silently.** Phone off, different wifi, app not launched: the phone
  falls back to its own default soul-state and the Mac shows its own. Sync is
  enrichment, never a dependency.

## Data flow

```
  [Mac: collectors] --compute--> DayLabel + index
        |
        | (2) share current state when paired + near
        v
  [Mac: Niyora] --(1) advertise via Bonjour-->  [iPhone: Niyora]
  [Mac: Niyora] --(2) send { label, index, ts }->
  [Mac: Niyora] <--(3) one-tap check-in { feeling, ts }--
        |                                           |
        v                                           v
  (4) fold feeling into displayed state    (4) colour reminders + My Soul
```

1. **Discovery + pairing.** Mac advertises `_niyora._tcp`. Phone browses, pairs
   once with a code shown on the Mac and confirmed on the phone. Shared secret in
   the Keychain on both ends. Reject unpaired peers. (Identical to the HRV spec;
   one pairing covers both features.)
2. **State push.** When paired and reachable, the Mac sends its current state on
   change (and on connect). Small, cheap, idempotent.
3. **Check-in.** The phone's one-tap check-in sends `{ feeling, ts }` back.
4. **Display.** Each side shows one reconciled soul-state. The self-reported
   feeling, when present and recent, takes priority over the inferred label,
   because the user telling you beats any guess.

## The mobile enrichment model (decided)

How the phone gets richer over time, without surveillance. This is settled
direction, not an open question:

- **Default with zero extra signals = the one-tap check-in.** Everything else is
  enrichment on top, never required. The app must feel complete for a user who
  only ever taps a feeling.
- **The Mac's synced soul-state is the first enrichment, and it is free** to the
  user: no permission, no setup beyond the one-time pairing. This is the biggest
  lever, because the Mac signal is already rich.
- **Additional phone-side signals are user-chosen and offered later, not on day
  one.** Framed as "what should Niyora pay attention to?", each option stated as a
  benefit, nothing on by default. Introduced only after the user trusts the app.
- **The realistic phone signal is Screen Time, via DeviceActivity.** Preset
  thresholds (total phone use over X today, any use in a late-night window) fire a
  background event the app reads as a bucket: "heavy usage today: yes", "late-night
  use: yes". Apple seals the exact minute counts, which is fine, the raw numbers
  never needed to leave anyway. The app turns the bucket into a soul-state on
  device; only the derived label syncs to the Mac. Cost: a FamilyControls
  permission prompt, a requested Apple entitlement, and a small monitor extension.
- **Calendar (EventKit) is not pursued** for now; unclear it adds enough over the
  Screen Time signal to justify another permission.
- **No surveillance-style inference.** No reading app content, keystrokes, or
  input activity, even where a platform allows it. Note: the Mac's existing
  keystroke-pace collector predates this direction; revisit whether it stays, but
  it is out of scope for this spec.

## Honest gaps (design for them, do not hide them)

- **Devices apart is the normal case.** Local-network sync only happens when both
  are awake on the same wifi. Most of the time the phone is on its own. It must be
  fully usable on its own state; the Mac signal is a bonus when present, never a
  prerequisite. Show the last-known synced state with its age, or just the phone's
  own state. Never an error.
- **Stale state.** A `DayLabel` from three hours ago is not the user's state now.
  Age the synced signal out (e.g. ignore beyond a freshness window) and fall back
  to the phone's own.
- **Conflicts.** Mac says "heavy," user just tapped "feeling light." The
  self-report wins. Always trust the human over the inference.
- **No nag.** If the check-in becomes a chore, the feature has failed. One tap,
  optional, no guilt.

## Build milestones

Each milestone is independently demoable. Sequence assumes the HRV sync layer may
or may not exist yet; milestones 1-2 are shared with it.

| # | Milestone | Outcome |
|---|---|---|
| 1 | Mac: Bonjour advertise + pairing | Mac announces itself, shows a pairing code. |
| 2 | Phone: discover + pair + receive a test payload | Phone finds the Mac, pairs, receives a dummy `{label, index, ts}`. |
| 3 | Mac: push real `DayLabel` + index on change | Phone receives the live soul-state. Verifiable with a sniffer. |
| 4 | Phone: colour reminders + My Soul from synced state | The Mac's day visibly shapes the phone experience when near. |
| 5 | Phone: one-tap check-in + send back | User taps a feeling; Mac receives `{feeling, ts}`. |
| 6 | Reconciliation + freshness | One soul-state per device; self-report wins; stale Mac state ages out. |
| 7 | Offline-first polish | Phone fully usable with no Mac in range; last-known state shown with age. |
| 8 | Later-offered extra signal (opt-in) | The "what should Niyora pay attention to?" menu, with Screen Time via DeviceActivity (usage + late-night thresholds). Needs the FamilyControls entitlement + monitor extension. Ships after trust is established. |

Milestones 1-2 are shared with the HRV companion. If that lands first, this
feature starts at milestone 3.

## Open decisions

1. **Check-in shape.** Pre-session only, post-session only, or both. Both gives a
   stress reading and proof the practice worked, at the cost of two taps. Start
   with one, measure.
2. **Freshness window.** How old a synced Mac state can be before the phone
   ignores it. Guess 1-2 hours; revisit with real use.
3. **State on the phone when never paired.** Pure self-report, or a light
   time-of-day default. Lean self-report to keep one honest source.
4. **Whether pre/post check-ins also enrich the Mac's My Soul trend**, or stay
   phone-local. One source of truth argues for syncing; simplicity argues for not.

## Explicitly out of scope

- CloudKit / any cloud or account (local network only, decided).
- HRV (separate spec, `hrv-companion-spec.md`).
- Calendar / EventKit as a signal (dropped for now).
- Reading exact Screen Time minute counts (Apple seals them; we use buckets via
  DeviceActivity thresholds, which is enough).
- Surveillance-style inference on either platform.
- Changing the Mac's existing collectors.
