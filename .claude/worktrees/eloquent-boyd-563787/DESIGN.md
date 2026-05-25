# DESIGN.md

The visual, interaction, and tonal language for the Niyora macOS app. This document is for designers, contributors, and AI agents making changes to the app's surface.

## Brand promise

> Calm in 60 seconds. Privacy-first. Nothing leaves your Mac.

Three claims, in this order:
1. The product is fast, small, and finishes in a minute.
2. The product respects you.
3. The product earns trust through architecture, not promises.

Every interaction decision should reinforce at least one of these. If a change doesn't, it probably shouldn't ship.

## Voice and copy

- **Quiet, not chirpy.** No exclamation points. No "Yay!" or "Great job!"
- **Direct, not preachy.** State what's happening. Trust the user.
- **No motivational mantras.** "You've got this!" is out.
- **No em dashes.** Use periods, commas, or middle dots (`·`).
- **No emojis** in body copy (notification text, panel UI). The visuals are the cue.
- **First person works** sparingly (the founder note on the site). The app itself stays in second person or impersonal voice.

## The core experience

The user lives a long screen day. Niyora notices and offers a 60-second reset. Everything in the design exists to keep that loop short, low-friction, and skippable.

### The one-minute session

- Click tray icon → panel appears
- Optional pre-session info screen (technique name + brief instruction)
- Breathing visual + audio for ~60 seconds
- Optional post-session mood capture
- Panel dismisses; back to your work

If any step takes more than a few seconds of friction, it's broken. The user shouldn't have to think.

## The five Soul tiers

Practice history unlocks more advanced techniques and shifts the visual identity of the app's "My Soul" panel.

| Tier | Hue (HSL) | Sat | Feeling |
|---|---:|---:|---|
| Spark | ~30 (warm orange) | 70% | First flame, beginner energy |
| Glow | ~335 (rose) | 70% | Settled, regular practice |
| Shine | ~280 (violet) | 65% | Confidence, deeper work |
| Radiance | ~230 (deep blue) | 65% | Steady, embodied |
| Brilliance | ~210 (cool blue) | 60% | Quiet mastery |

These tier colors are the same ones the marketing site's orb cycles through. If you change one, change both ([niyora-web `OrbStage.astro`](https://github.com/neha-prasad-ux/niyora-web/blob/main/src/components/OrbStage.astro)).

Tier progression is based on practice history (frequency + recency), not technique difficulty. See `src/tiers.ts` and the project memory note `project_wave4_soul_levels.md`.

## Breathing techniques

Seven pranayama practices, each with its own visual personality:

- **Box Breath** — square cadence, even
- **Ocean Breath (Ujjayi)** — wave-like fall and rise
- **Cooling Breath (Sheetali)** — cool palette, soft particles
- **Alternate Nostril (Naadishodhana)** — left/right alternation
- **Left Nostril** — single-side breath, deeper hue
- **Belly Breath (Diaphragmatic)** — slow, grounded
- **Wind Down (4-7-8)** — extended exhale, dimming visual

Each technique has a unique visual personality (particles, river, leaf, glowing focal point, etc.). See `src/BreathingSession.tsx` and `src/techniques.ts`.

## Mindfulness practices

Seven non-breathing moments grounded in CBT, self-compassion, and grounding research:

- Be Kind to Yourself (self-compassion · Neff 2003)
- Let It Drift (CBT thought defusion)
- Bring Someone to Mind (gratitude)
- Hold Yourself (somatic)
- Kind Words (affirmation)
- Five Senses (5-4-3-2-1 grounding)
- Soft Gaze (Trataka · Talwadkar 2014)

Every practice has a citation behind it. No mystic claims.

## Visual language

### Background and depth

- App background: near-black with a faint indigo cast.
- 3D spheres and gradient backgrounds make the surface feel deep without being busy.
- Source Serif font for headings (calm, restrained); system sans for UI controls.
- The breathing canvas dominates during a session; chrome dims toward the edges.

### Color use

- Tier colors come from the user's progression, not arbitrary accent choices.
- Avoid hard accent colors outside tier hues. Errors and warnings, when needed, use soft variants of red/amber.
- Contrast on body text ≥ 7:1.

### Typography

- **Source Serif 4** for headings and long-form copy (e.g., post-session screens).
- **System sans** (San Francisco) for controls and panel chrome.
- Generous line-height (1.6 for body, 1.2 for headings).
- No display fonts, no script fonts.

### Motion

- Breath cycles dictate animation cadence. Easing should feel like an inhale, not a bounce.
- `prefers-reduced-motion` must be respected throughout. Particles and 3D depth scale down or disable.
- No animation should outlast a single breath cycle (~5–7 seconds).
- The picker overlay slides in calmly; never a hard pop-in.

## Interaction grammar

- **One primary action per screen.** Either "Begin," or "Done," or the close button. Never two equal-weight buttons.
- **Click outside to dismiss** the panel. Escape key also closes.
- **Tray icon click** brings the panel forward; second click dismisses.
- **Notifications** have two action buttons: "Breathe now" (opens panel) and "Snooze 30 min" (delays the next reminder). No third action.
- **No carousel, no modal stack, no multi-step wizard** outside onboarding.

## Reminders

- Smart reminders run during work hours (9am–6pm, configurable).
- Frequency is computed from screen-time signals, mic activity, app switching, and the time of day. The "Niyora Index" combines these.
- The user can always snooze, mute for the day, or open the panel manually from the tray.
- The app must NEVER nag. If a notification is dismissed, the next one is computed gently, not aggressively.

## Onboarding

- Single-screen first-launch flow. No multi-step quiz.
- Asks the minimum: which window of hours to remind during, and a one-tap "I'm in." Everything else has sensible defaults.
- No account creation. No email collection. No "share with a friend" CTA.

## Privacy in the visual

The privacy promise has to be visible in the product, not just the README:

- The app shows no remote content.
- The "About" screen names the local data dir and links to the source.
- No "share to" buttons that imply network reach.
- Settings panel can include a single "Reset all my data" action (deletes the JSONL files).

## Accessibility

- Focus rings visible by default; do not suppress globally.
- Color is never the only way to convey state.
- The breathing canvas always has a non-visual cue (audio, haptics if added, screen-reader-friendly status text).
- All interactive elements have an accessible name.
- High-contrast mode tested before each release.

## What's intentionally absent

- A streak counter or guilt-tripping "you missed yesterday" screens.
- Achievements, badges, leaderboards.
- Social features.
- Coach voices, virtual humans, AI avatars.
- An "Apple Watch companion."
- Cloud sync.

If a future PR proposes one of these, it should justify it against this list, not just say "users would like it."
