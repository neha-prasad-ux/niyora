import { describe, it, expect } from "vitest";
import {
  TIERS,
  TIER_COLORS,
  tierById,
  tierColor,
  tierHue,
  currentTier,
  nextTier,
  sessionsToNext,
} from "./tiers";

describe("tierById", () => {
  it("returns the tier for each valid id", () => {
    for (const tier of TIERS) {
      expect(tierById(tier.id)).toBe(tier);
    }
  });

  it("throws for an unknown id", () => {
    // @ts-expect-error intentional invalid id
    expect(() => tierById("unknown")).toThrow("Unknown tier: unknown");
  });
});

describe("tierColor", () => {
  it("returns the color entry matching the tier id", () => {
    for (const tier of TIERS) {
      expect(tierColor(tier)).toEqual(TIER_COLORS[tier.id]);
    }
  });
});

describe("tierHue", () => {
  it("returns the hue from TIER_COLORS for each tier", () => {
    for (const tier of TIERS) {
      expect(tierHue(tier)).toBe(TIER_COLORS[tier.id].hue);
    }
  });
});

describe("currentTier", () => {
  it("returns spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("returns glow at threshold boundary (5)", () => {
    expect(currentTier(5).id).toBe("glow");
  });

  it("returns shine at threshold boundary (15)", () => {
    expect(currentTier(15).id).toBe("shine");
  });

  it("returns radiance at threshold boundary (40)", () => {
    expect(currentTier(40).id).toBe("radiance");
  });

  it("returns brilliance at threshold boundary (80)", () => {
    expect(currentTier(80).id).toBe("brilliance");
  });

  it("returns brilliance well above the top threshold", () => {
    expect(currentTier(500).id).toBe("brilliance");
  });

  it("stays on spark at 4 sessions (just below glow threshold)", () => {
    expect(currentTier(4).id).toBe("spark");
  });
});

describe("nextTier", () => {
  it("returns glow when current is spark", () => {
    expect(nextTier(TIERS[0])?.id).toBe("glow");
  });

  it("returns null when current is brilliance (top tier)", () => {
    expect(nextTier(TIERS[TIERS.length - 1])).toBeNull();
  });

  it("returns the correct successor for each non-top tier", () => {
    for (let i = 0; i < TIERS.length - 1; i++) {
      expect(nextTier(TIERS[i])?.id).toBe(TIERS[i + 1].id);
    }
  });
});

describe("sessionsToNext", () => {
  it("returns 5 at 0 sessions (spark, next is glow at 5)", () => {
    expect(sessionsToNext(0)).toBe(5);
  });

  it("returns 10 at the glow threshold (next is shine at 15)", () => {
    expect(sessionsToNext(5)).toBe(10);
  });

  it("returns 0 at brilliance (top tier, nothing next)", () => {
    expect(sessionsToNext(80)).toBe(0);
    expect(sessionsToNext(200)).toBe(0);
  });

  it("returns sessions remaining within a tier", () => {
    // At 10 sessions, current is glow (threshold 5), next is shine (threshold 15)
    expect(sessionsToNext(10)).toBe(5);
  });
});
