/**
 * "My Soul" progression tiers. Five levels matched to the orb's visual
 * brightening: as the user practices, their Soul moves Spark → Brilliance.
 *
 * Thresholds count COMPLETED sessions only (abandoned sessions don't progress).
 *
 * Each tier unlocks a set of techniques. Locked techniques remain visible
 * in the pick-from-list with a small lock + "Unlocks at <Tier>" — never
 * hidden, always promised.
 *
 * ---
 *
 * CANONICAL SOURCE OF TRUTH FOR TIER COLORS
 *
 * This file defines the HSL color values for all five soul tiers.
 * These colors MUST be mirrored in:
 *   - niyora-web: src/components/OrbStage.astro
 *   - niyora-companion: Techniques.swift
 *
 * Part of Batman bundle 'tier-colour-sync' (neha-prasad-ux/niyora-companion#19).
 * Machine-readable export available at public/tier-colors.json.
 */

export interface Tier {
  /** Stable id for storage / lookups. */
  id: "spark" | "glow" | "shine" | "radiance" | "brilliance";
  /** Display name. */
  name: string;
  /** Completed-session count to reach this tier. */
  threshold: number;
}

/** HSL color values for each tier. Hue progression: warm → cool as soul brightens. */
export interface TierColor {
  /** Hue (0-360). */
  hue: number;
  /**
   * Saturation (0-100%). Advisory for external consumers (tier-colors.json).
   * Not yet consumed by app CSS; only hue is used via tierHue().
   */
  saturation: number;
  /**
   * Lightness (0-100%). Advisory for external consumers (tier-colors.json).
   * Not yet consumed by app CSS; only hue is used via tierHue().
   */
  lightness: number;
}

export const TIERS: Tier[] = [
  { id: "spark",      name: "Spark",      threshold: 0 },
  { id: "glow",       name: "Glow",       threshold: 5 },
  { id: "shine",      name: "Shine",      threshold: 15 },
  { id: "radiance",   name: "Radiance",   threshold: 40 },
  { id: "brilliance", name: "Brilliance", threshold: 80 },
];

/**
 * Canonical HSL color definitions for each tier.
 * These match the orb's visual progression on the marketing site.
 *
 * After changing any value here, run `pnpm run generate:tier-colors` to
 * regenerate public/tier-colors.json. CI will fail if they diverge.
 */
export const TIER_COLORS: Record<Tier["id"], TierColor> = {
  spark:      { hue: 30,  saturation: 70, lightness: 60 }, // warm orange, first flame
  glow:       { hue: 335, saturation: 70, lightness: 60 }, // rose, settled practice
  shine:      { hue: 280, saturation: 65, lightness: 60 }, // violet, deeper work
  radiance:   { hue: 230, saturation: 65, lightness: 60 }, // deep blue, embodied
  brilliance: { hue: 210, saturation: 60, lightness: 60 }, // cool blue, mastery
};

/** Look up a tier by id. */
export function tierById(id: Tier["id"]): Tier {
  const t = TIERS.find((x) => x.id === id);
  if (!t) throw new Error(`Unknown tier: ${id}`);
  return t;
}

/**
 * Get the full HSL color for a tier.
 * Exported for external tooling and future use. Not yet called within this app;
 * app CSS currently uses only hue via tierHue().
 */
export function tierColor(tier: Tier): TierColor {
  return TIER_COLORS[tier.id];
}

/** Get the hue value for a tier (convenience helper). */
export function tierHue(tier: Tier): number {
  return TIER_COLORS[tier.id].hue;
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
