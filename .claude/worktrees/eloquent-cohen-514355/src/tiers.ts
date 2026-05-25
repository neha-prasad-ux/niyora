/**
 * "My Soul" progression tiers. Five levels matched to the orb's visual
 * brightening: as the user practices, their Soul moves Spark → Brilliance.
 *
 * Thresholds count COMPLETED sessions only (abandoned sessions don't progress).
 *
 * Each tier unlocks a set of techniques. Locked techniques remain visible
 * in the pick-from-list with a small lock + "Unlocks at <Tier>" — never
 * hidden, always promised.
 */

export interface Tier {
  /** Stable id for storage / lookups. */
  id: "spark" | "glow" | "shine" | "radiance" | "brilliance";
  /** Display name. */
  name: string;
  /** Completed-session count to reach this tier. */
  threshold: number;
}

export const TIERS: Tier[] = [
  { id: "spark",      name: "Spark",      threshold: 0 },
  { id: "glow",       name: "Glow",       threshold: 5 },
  { id: "shine",      name: "Shine",      threshold: 15 },
  { id: "radiance",   name: "Radiance",   threshold: 40 },
  { id: "brilliance", name: "Brilliance", threshold: 80 },
];

/** Look up a tier by id. */
export function tierById(id: Tier["id"]): Tier {
  const t = TIERS.find((x) => x.id === id);
  if (!t) throw new Error(`Unknown tier: ${id}`);
  return t;
}

/** Resolve the current tier from a completed-session count. */
export function currentTier(completedSessions: number): Tier {
  let current = TIERS[0];
  for (const t of TIERS) {
    if (completedSessions >= t.threshold) current = t;
  }
  return current;
}

/** The next tier above `current`, or null if already at the top. */
export function nextTier(current: Tier): Tier | null {
  const idx = TIERS.findIndex((t) => t.id === current.id);
  return idx >= 0 && idx < TIERS.length - 1 ? TIERS[idx + 1] : null;
}

/** Sessions remaining to reach the next tier (0 if at the top). */
export function sessionsToNext(completedSessions: number): number {
  const next = nextTier(currentTier(completedSessions));
  return next ? Math.max(0, next.threshold - completedSessions) : 0;
}
