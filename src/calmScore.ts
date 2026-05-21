/**
 * Map HRV (SDNN or RMSSD in milliseconds) to a 0-100 calm score.
 *
 * The HRV ↔ stress relationship is inverse (low HRV = stressed), which
 * confuses users when shown as a raw number. We expose a derived score
 * where 0 = highly activated and 100 = deeply calm, matching the
 * direction Whoop, Oura, and other consumer wearables use for their
 * "recovery" / "readiness" scores.
 *
 * Mapping is piecewise linear with population-typical breakpoints:
 *
 *   HRV (ms) | calm score
 *   ---------|-----------
 *      0     |    0
 *     20     |   25
 *     40     |   50
 *     60     |   75
 *     80     |   100
 *    >80     |   100  (cap)
 *
 * These breakpoints come from published consumer HRV norms (HRV4Training,
 * Oura, Whoop research blogs). They are a v1 best-guess; a future
 * milestone could refine per-user (percentile-of-your-own-baseline)
 * once we have enough data.
 */
export function calmScore(hrvMs: number | null | undefined): number | null {
  if (hrvMs === null || hrvMs === undefined) return null;
  if (!Number.isFinite(hrvMs) || hrvMs < 0) return null;

  // Piecewise linear between the breakpoints above.
  if (hrvMs >= 80) return 100;
  if (hrvMs <= 0) return 0;

  // Each 20 ms segment of HRV corresponds to 25 points of score.
  return Math.round((hrvMs / 80) * 100);
}

/** Plain-language state label for a given calm score. */
export function calmLabel(score: number | null): string {
  if (score === null) return "no reading";
  if (score >= 80) return "deeply calm";
  if (score >= 60) return "calm";
  if (score >= 40) return "neutral";
  if (score >= 20) return "activated";
  return "highly activated";
}
