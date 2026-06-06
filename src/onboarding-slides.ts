import { isWindows } from "./platform";

export interface Slide {
  eyebrow?: string;
  title: string;
  body?: string;
  bullets?: string[];
  /** Auto-advance duration override in ms. Only used for middle slides. */
  durationMs?: number;
  /** Bold heading line at the top of the note panel. */
  noteHead?: string;
  /** Secondary detail text, rendered in a panel below the body. */
  note?: string;
  /** When true, Onboarding renders an App Store QR instead of body copy. */
  qrSlide?: boolean;
}

export const SLIDES: Slide[] = [
  {
    eyebrow: "Niyora",
    title: "Calm in 60 seconds.",
    body: "For founders, sales, devs, PMs, designers.",
    bullets: [
      isWindows ? "Data stays on your device" : "Data stays on your Mac",
      "We spot stress in your work patterns",
      "We notify you when you need calm",
      "Breathing and mindful practices, combined",
      "Every practice under 60 seconds",
    ],
  },
  {
    title: "Private by design",
    noteHead: "Your data stays local.",
    note: isWindows
      ? "Stress scores, breath patterns, anything that identifies you. None of it leaves your device.\n\nNiyora starts with Windows so it's there when you need it."
      : "Stress scores, breath patterns, anything that identifies you. None of it leaves your Mac.\n\nNiyora launches with your Mac so it's there when you need it.",
  },
  {
    title: "I'll breathe with Niyora for 7 days.",
    body: "A soft intention, not a contract. One minute a day is enough.",
    durationMs: 5000,
  },
  ...(!isWindows ? [{ title: "Niyora, in your pocket too.", qrSlide: true }] : []),
];
