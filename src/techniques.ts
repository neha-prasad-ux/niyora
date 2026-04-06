/**
 * Niyora techniques — pranayama breathing + mindfulness moments.
 *
 * Two types share the same particle renderer:
 * - "breathing": timer-driven phases, particle motion follows breath
 * - "mindfulness": text-driven prompts, particles set the ambient mood
 */

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
  phases: BreathPhase[];
  rounds: number;
  visual: VisualConfig;
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
  prompts: MindfulPrompt[];
  visual: VisualConfig;
}

// ── Union type ──

export type Technique = BreathingTechnique | MindfulnessTechnique;

// =====================================================================
// BREATHING TECHNIQUES
// =====================================================================

const breathingTechniques: BreathingTechnique[] = [
  {
    kind: "breathing",
    name: "Box Breathing",
    subtitle: "Sama Vritti",
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
    name: "Ujjayi",
    subtitle: "Ocean Breath",
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
    subtitle: "Sheetali",
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
    subtitle: "Nadi Shodhana",
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
    subtitle: "Chandra Bhedana",
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
    name: "Diaphragmatic",
    subtitle: "Belly Breathing",
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
    name: "4-7-8 Breath",
    subtitle: "Relaxing Breath",
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
    name: "Self Compassion",
    subtitle: "Kristin Neff's MSC",
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
    name: "Leaves on a Stream",
    subtitle: "Cognitive Defusion",
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
    name: "Gratitude",
    subtitle: "A Moment of Warmth",
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
    name: "Self Hug",
    subtitle: "Somatic Comfort",
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
    name: "Positive Affirmation",
    subtitle: "Words of Strength",
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
    name: "5-4-3-2-1",
    subtitle: "Grounding",
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
    name: "Trataka",
    subtitle: "Steady Gaze",
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

export function randomTechnique(): Technique {
  return techniques[Math.floor(Math.random() * techniques.length)];
}
