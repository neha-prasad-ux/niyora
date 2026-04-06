/**
 * Pranayama breathing techniques with visual personalities.
 */

export interface BreathPhase {
  type: "inhale" | "hold" | "exhale";
  label: string;
  duration: number;
}

/** Per-phase color overrides (HSL). If not provided, falls back to visual defaults. */
export interface PhaseColors {
  inhale: [number, number, number];
  hold: [number, number, number];
  exhale: [number, number, number];
}

/** Visual personality for a technique */
export interface VisualConfig {
  /** Base background HSL per phase */
  colors: PhaseColors;
  /**
   * Particle motion style:
   * - "converge"  — default, particles pull to center on inhale, push out on exhale
   * - "wave"      — ocean-like rolling horizontal wave motion
   * - "snowfall"  — particles drift downward with sparkle
   * - "alternate" — particles split left/right, alternating sides
   * - "lunar"     — soft leftward drift, slow and dreamy
   * - "belly"     — particles rise and fall vertically (diaphragm)
   * - "sedation"  — particles gradually slow and dim over rounds
   */
  motion: "converge" | "wave" | "snowfall" | "alternate" | "lunar" | "belly" | "sedation";
  /** Particle brightness boost (0 = normal, 0.2 = brighter) */
  brightnessBoost?: number;
}

export interface Technique {
  name: string;
  subtitle: string;
  phases: BreathPhase[];
  rounds: number;
  visual: VisualConfig;
}

export const techniques: Technique[] = [
  {
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
        inhale: [230, 28, 12],  // blue-indigo
        hold:   [250, 18, 11],  // soft purple
        exhale: [210, 22, 11],  // muted blue
      },
      motion: "converge",
    },
  },
  {
    name: "Ujjayi",
    subtitle: "Ocean Breath",
    phases: [
      { type: "inhale", label: "inhale through nose", duration: 4 },
      { type: "exhale", label: "exhale slowly",       duration: 6 },
    ],
    rounds: 6,
    visual: {
      colors: {
        inhale: [185, 30, 12],  // warm teal
        hold:   [190, 20, 11],
        exhale: [175, 25, 11],  // deep ocean
      },
      motion: "wave",
    },
  },
  {
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
        inhale: [200, 35, 13],  // ice blue
        hold:   [210, 20, 11],
        exhale: [195, 30, 10],  // cool blue-white
      },
      motion: "snowfall",
      brightnessBoost: 0.15,
    },
  },
  {
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
        inhale: [270, 25, 12],  // purple
        hold:   [45, 20, 11],   // warm gold
        exhale: [280, 20, 11],  // soft violet
      },
      motion: "alternate",
    },
  },
  {
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
        inhale: [220, 18, 13],  // silver-blue
        hold:   [230, 12, 11],  // moonlight
        exhale: [215, 15, 10],  // deep silver
      },
      motion: "lunar",
      brightnessBoost: 0.1,
    },
  },
  {
    name: "Diaphragmatic",
    subtitle: "Belly Breathing",
    phases: [
      { type: "inhale", label: "breathe into belly", duration: 4 },
      { type: "exhale", label: "release slowly",     duration: 6 },
    ],
    rounds: 6,
    visual: {
      colors: {
        inhale: [90, 22, 12],   // earthy green
        hold:   [60, 18, 11],   // warm moss
        exhale: [35, 25, 11],   // amber-earth
      },
      motion: "belly",
    },
  },
  {
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
        inhale: [260, 22, 12],  // deep violet
        hold:   [250, 18, 10],  // navy
        exhale: [245, 15, 9],   // darker navy
      },
      motion: "sedation",
    },
  },
];

export function randomTechnique(): Technique {
  return techniques[Math.floor(Math.random() * techniques.length)];
}
