/**
 * Niyora techniques — pranayama breathing + mindfulness moments.
 *
 * Two types share the same particle renderer:
 * - "breathing": timer-driven phases, particle motion follows breath
 * - "mindfulness": text-driven prompts, particles set the ambient mood
 *
 * Each technique declares an `unlockTier` — the My Soul tier that unlocks it.
 * Spark-tier techniques are available from day one; harder techniques unlock
 * progressively as the user practices.
 */

import type { Tier } from "./tiers";

// ── Shared visual config ──

export interface PhaseColors {
  inhale: [number, number, number];
  hold: [number, number, number];
  exhale: [number, number, number];
}

export interface VisualConfig {
  colors: PhaseColors;
  motion: "converge" | "wave" | "snowfall" | "alternate" | "lunar" | "belly"
    | "sedation" | "river" | "warm-pulse" | "orbit" | "sensory" | "ambient";
  brightnessBoost?: number;
}

// ── Breathing techniques ──

export interface BreathPhase {
  type: "inhale" | "hold" | "exhale";
  label: string;
  duration: number;
}

export interface BreathingTechnique {
  kind: "breathing";
  name: string;
  subtitle: string;
  instructions: string;
  benefits: string;
  phases: BreathPhase[];
  rounds: number;
  visual: VisualConfig;
  unlockTier: Tier["id"];
}

// ── Mindfulness moments ──

export interface MindfulPrompt {
  text: string;
  /** How long the text is visible (seconds) */
  duration: number;
}

export interface MindfulnessTechnique {
  kind: "mindfulness";
  name: string;
  subtitle: string;
  instructions: string;
  benefits: string;
  prompts: MindfulPrompt[];
  visual: VisualConfig;
  unlockTier: Tier["id"];
}

// ── Union type ──

export type Technique = BreathingTechnique | MindfulnessTechnique;

// =====================================================================
// BREATHING TECHNIQUES
// =====================================================================

const breathingTechniques: BreathingTechnique[] = [
  {
    kind: "breathing",
    name: "Box Breath",
    subtitle: "calms under pressure · 65s",
    unlockTier: "spark",
    instructions: "Breathe through your nose. In 4, hold 4, out 4, hold 4. Steady rhythm.",
    benefits: "Let's lower your stress hormones and switch your body into rest mode.",
    phases: [
      { type: "inhale", label: "inhale", duration: 4 },
      { type: "hold",   label: "hold",   duration: 4 },
      { type: "exhale", label: "exhale", duration: 4 },
      { type: "hold",   label: "hold",   duration: 4 },
    ],
    rounds: 4,
    visual: {
      colors: {
        inhale: [230, 28, 12],
        hold:   [250, 18, 11],
        exhale: [210, 22, 11],
      },
      motion: "converge",
    },
  },
  {
    kind: "breathing",
    name: "Ocean Breath",
    subtitle: "slows heart rate · 60s",
    unlockTier: "radiance",
    instructions: "Soft 'haaa' sound at the back of your throat. Breathe through nose.",
    benefits: "Time to slow your heart rate and quiet those overactive brain signals.",
    phases: [
      { type: "inhale", label: "inhale through nose", duration: 4 },
      { type: "exhale", label: "exhale slowly",       duration: 6 },
    ],
    rounds: 6,
    visual: {
      colors: {
        inhale: [185, 30, 12],
        hold:   [190, 20, 11],
        exhale: [175, 25, 11],
      },
      motion: "wave",
    },
  },
  {
    kind: "breathing",
    name: "Cooling Breath",
    subtitle: "lowers body heat · 60s",
    unlockTier: "glow",
    instructions: "Curl your tongue. Inhale through it, exhale through nose.",
    benefits: "Let's cool your body down and lower your blood pressure in just a few breaths.",
    phases: [
      { type: "inhale", label: "inhale through mouth", duration: 4 },
      { type: "hold",   label: "hold",                 duration: 2 },
      { type: "exhale", label: "exhale through nose",  duration: 6 },
    ],
    rounds: 5,
    visual: {
      colors: {
        inhale: [200, 35, 13],
        hold:   [210, 20, 11],
        exhale: [195, 30, 10],
      },
      motion: "snowfall",
      brightnessBoost: 0.15,
    },
  },
  {
    kind: "breathing",
    name: "Alternate Nostril",
    subtitle: "reset between tasks · 75s",
    unlockTier: "radiance",
    instructions: "Inhale left, exhale right. Then inhale right, exhale left.",
    benefits: "Get ready to bring both sides of your brain into sync.",
    phases: [
      { type: "inhale", label: "inhale left",  duration: 4 },
      { type: "hold",   label: "hold",         duration: 4 },
      { type: "exhale", label: "exhale right", duration: 4 },
      { type: "inhale", label: "inhale right", duration: 4 },
      { type: "hold",   label: "hold",         duration: 4 },
      { type: "exhale", label: "exhale left",  duration: 4 },
    ],
    rounds: 3,
    visual: {
      colors: {
        inhale: [270, 25, 12],
        hold:   [45, 20, 11],
        exhale: [280, 20, 11],
      },
      motion: "alternate",
    },
  },
  {
    kind: "breathing",
    name: "Left Nostril",
    subtitle: "switches into rest · 60s",
    unlockTier: "shine",
    instructions: "Press right nostril closed. Breathe in and out through the left.",
    benefits: "Let's activate your body's built-in calming system and ease you toward rest.",
    phases: [
      { type: "inhale", label: "inhale left",  duration: 4 },
      { type: "hold",   label: "hold",         duration: 2 },
      { type: "exhale", label: "exhale right", duration: 6 },
    ],
    rounds: 5,
    visual: {
      colors: {
        inhale: [220, 18, 13],
        hold:   [230, 12, 11],
        exhale: [215, 15, 10],
      },
      motion: "lunar",
      brightnessBoost: 0.1,
    },
  },
  {
    kind: "breathing",
    name: "Belly Breath",
    subtitle: "eases the body · 60s",
    unlockTier: "spark",
    instructions: "Breathe through your nose. Let your belly rise on the in-breath, soften on the out.",
    benefits: "Let's strengthen your body's stress resilience and bring down inflammation.",
    phases: [
      { type: "inhale", label: "breathe into belly", duration: 4 },
      { type: "exhale", label: "release slowly",     duration: 6 },
    ],
    rounds: 6,
    visual: {
      colors: {
        inhale: [90, 22, 12],
        hold:   [60, 18, 11],
        exhale: [35, 25, 11],
      },
      motion: "belly",
    },
  },
  {
    kind: "breathing",
    name: "Wind Down",
    subtitle: "deep relaxation · 75s",
    unlockTier: "spark",
    instructions: "Inhale 4, hold 7, exhale 8. The long exhale is the key.",
    benefits: "Get ready to guide your nervous system into deep relaxation.",
    phases: [
      { type: "inhale", label: "inhale",  duration: 4 },
      { type: "hold",   label: "hold",    duration: 7 },
      { type: "exhale", label: "exhale",  duration: 8 },
    ],
    rounds: 4,
    visual: {
      colors: {
        inhale: [260, 22, 12],
        hold:   [250, 18, 10],
        exhale: [245, 15, 9],
      },
      motion: "sedation",
    },
  },
];

// =====================================================================
// MINDFULNESS MOMENTS
// =====================================================================

const mindfulnessTechniques: MindfulnessTechnique[] = [
  {
    kind: "mindfulness",
    name: "Be Kind to Yourself",
    subtitle: "softens self-criticism · 25s",
    unlockTier: "spark",
    instructions: "Read each phrase slowly. Let it land.",
    benefits: "Let's lower your stress hormones and give your mind the warmth it needs.",
    prompts: [
      { text: "this is a moment of difficulty",     duration: 8 },
      { text: "difficulty is part of being human",  duration: 8 },
      { text: "may I be kind to myself right now",  duration: 8 },
    ],
    visual: {
      colors: {
        inhale: [25, 28, 12],
        hold:   [30, 22, 11],
        exhale: [20, 25, 11],
      },
      motion: "warm-pulse",
    },
  },
  {
    kind: "mindfulness",
    name: "Let It Drift",
    subtitle: "loosens stuck thoughts · 35s",
    unlockTier: "shine",
    instructions: "Notice a thought. Place it on a leaf. Watch it float away.",
    benefits: "Time to loosen the grip of those thoughts so they stop feeling like facts.",
    prompts: [
      { text: "notice what's on your mind",        duration: 6 },
      { text: "place the thought on a leaf",       duration: 7 },
      { text: "watch it float gently downstream",  duration: 8 },
      { text: "let the stream carry it away",      duration: 7 },
      { text: "the stream flows on",               duration: 5 },
    ],
    visual: {
      colors: {
        inhale: [140, 22, 12],
        hold:   [150, 18, 11],
        exhale: [130, 20, 10],
      },
      motion: "river",
    },
  },
  {
    kind: "mindfulness",
    name: "Bring Someone to Mind",
    subtitle: "warms the mood · 25s",
    unlockTier: "spark",
    instructions: "Picture one person who matters. Feel the warmth.",
    benefits: "Let's boost your feel-good brain chemicals and ease your body.",
    prompts: [
      { text: "think of one person",          duration: 8 },
      { text: "feel that warmth",             duration: 8 },
      { text: "let it settle",                duration: 8 },
    ],
    visual: {
      colors: {
        inhale: [35, 30, 13],
        hold:   [40, 25, 12],
        exhale: [30, 28, 11],
      },
      motion: "warm-pulse",
      brightnessBoost: 0.1,
    },
  },
  {
    kind: "mindfulness",
    name: "Hold Yourself",
    subtitle: "signals safety · 30s",
    unlockTier: "spark",
    instructions: "Wrap your arms around yourself. Hold gently.",
    benefits: "Let's tell your brain you're safe and release those calming hormones.",
    prompts: [
      { text: "wrap your arms around yourself",  duration: 6 },
      { text: "hold gently",                     duration: 8 },
      { text: "feel the warmth of your own care", duration: 8 },
      { text: "you are held",                     duration: 6 },
    ],
    visual: {
      colors: {
        inhale: [20, 30, 13],
        hold:   [25, 25, 12],
        exhale: [15, 28, 11],
      },
      motion: "converge",
    },
  },
  {
    kind: "mindfulness",
    name: "Kind Words",
    subtitle: "quiets the inner critic · 30s",
    unlockTier: "spark",
    instructions: "Read each line silently. Repeat it to yourself.",
    benefits: "Time to rewire how your brain talks to you about yourself.",
    prompts: [
      { text: "I am enough",                   duration: 7 },
      { text: "I am doing my best",            duration: 7 },
      { text: "I deserve kindness",            duration: 7 },
      { text: "I am exactly where I need to be", duration: 7 },
    ],
    visual: {
      colors: {
        inhale: [45, 25, 12],
        hold:   [50, 20, 11],
        exhale: [40, 22, 11],
      },
      motion: "ambient",
      brightnessBoost: 0.05,
    },
  },
  {
    kind: "mindfulness",
    name: "Five Senses",
    subtitle: "grounds you in the body · 35s",
    unlockTier: "spark",
    instructions: "Notice 5 you see, 4 you touch, 3 you hear, 2 you smell, 1 you taste.",
    benefits: "Let's pull your attention out of those spiraling thoughts and back into your body.",
    prompts: [
      { text: "5 things you can see",    duration: 7 },
      { text: "4 things you can touch",  duration: 7 },
      { text: "3 things you can hear",   duration: 7 },
      { text: "2 things you can smell",  duration: 6 },
      { text: "1 thing you can taste",   duration: 6 },
    ],
    visual: {
      colors: {
        inhale: [160, 20, 12],
        hold:   [120, 18, 11],
        exhale: [80, 22, 11],
      },
      motion: "sensory",
    },
  },
  {
    kind: "mindfulness",
    name: "Soft Gaze",
    subtitle: "relaxes tired eyes · 35s",
    unlockTier: "brilliance",
    instructions: "Soften your eyes on the centre point. Let everything else blur.",
    benefits: "Let's increase your calm brainwave activity and relieve that eye-strain tension.",
    prompts: [
      { text: "soften your gaze",              duration: 5 },
      { text: "rest your eyes on the centre",  duration: 10 },
      { text: "let everything else blur",      duration: 10 },
      { text: "just seeing",                   duration: 8 },
    ],
    visual: {
      colors: {
        inhale: [270, 18, 12],
        hold:   [265, 15, 11],
        exhale: [275, 12, 10],
      },
      motion: "orbit",
    },
  },
];

// =====================================================================
// ALL TECHNIQUES + RANDOM SELECTION
// =====================================================================

export const techniques: Technique[] = [
  ...breathingTechniques,
  ...mindfulnessTechniques,
];

/** Same as `techniques` but split by kind, for the grouped picker UI. */
export const breathingList: BreathingTechnique[] = breathingTechniques;
export const mindfulnessList: MindfulnessTechnique[] = mindfulnessTechniques;

import { TIERS, currentTier } from "./tiers";

/** Techniques the user has unlocked at their current session count. */
export function unlockedTechniques(completedSessions: number): Technique[] {
  const tier = currentTier(completedSessions);
  const tierIdx = TIERS.findIndex((t) => t.id === tier.id);
  const reachedIds = new Set(TIERS.slice(0, tierIdx + 1).map((t) => t.id));
  return techniques.filter((t) => reachedIds.has(t.unlockTier));
}

/** Pick a random technique from the user's unlocked set.
 * Falls back to all techniques if completedSessions is unknown (e.g. backend
 * fetch failed) so the practice is never blocked.
 *
 * First-session rule: every new user gets Box Breath as their very first
 * practice. It's the most universally teachable technique and gives the
 * onboarding a calm, predictable shape before randomization kicks in. */
export function randomTechnique(completedSessions?: number): Technique {
  if (completedSessions === 0) {
    const box = breathingTechniques.find((t) => t.name === "Box Breath");
    if (box) return box;
  }
  const pool =
    completedSessions === undefined
      ? techniques
      : unlockedTechniques(completedSessions);
  return pool[Math.floor(Math.random() * pool.length)];
}
