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
  it("returns the correct tier for each valid id", () => {
    expect(tierById("spark").name).toBe("Spark");
    expect(tierById("glow").name).toBe("Glow");
    expect(tierById("shine").name).toBe("Shine");
    expect(tierById("radiance").name).toBe("Radiance");
    expect(tierById("brilliance").name).toBe("Brilliance");
  });

  it("throws for an unknown id", () => {
    expect(() => tierById("unknown" as never)).toThrow("Unknown tier: unknown");
  });
});

describe("tierColor", () => {
  it("returns the canonical color for each tier", () => {
    for (const tier of TIERS) {
      expect(tierColor(tier)).toEqual(TIER_COLORS[tier.id]);
    }
  });
});

describe("tierHue", () => {
  it("returns the hue component of the tier color", () => {
    for (const tier of TIERS) {
      expect(tierHue(tier)).toBe(TIER_COLORS[tier.id].hue);
    }
  });

  it("returns a number in the 0-360 range for every tier", () => {
    for (const tier of TIERS) {
      const hue = tierHue(tier);
      expect(hue).toBeGreaterThanOrEqual(0);
      expect(hue).toBeLessThanOrEqual(360);
    }
  });
});

describe("currentTier", () => {
  it("returns Spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("returns Spark below the Glow threshold", () => {
    expect(currentTier(4).id).toBe("spark");
  });

  it("returns Glow at threshold (5)", () => {
    expect(currentTier(5).id).toBe("glow");
  });

  it("returns Shine at threshold (15)", () => {
    expect(currentTier(15).id).toBe("shine");
  });

  it("returns Radiance at threshold (40)", () => {
    expect(currentTier(40).id).toBe("radiance");
  });

  it("returns Brilliance at threshold (80)", () => {
    expect(currentTier(80).id).toBe("brilliance");
  });

  it("returns Brilliance above the top threshold", () => {
    expect(currentTier(200).id).toBe("brilliance");
  });
});

describe("nextTier", () => {
  it("returns Glow as the next tier after Spark", () => {
    expect(nextTier(tierById("spark"))?.id).toBe("glow");
  });

  it("returns Shine as the next tier after Glow", () => {
    expect(nextTier(tierById("glow"))?.id).toBe("shine");
  });

  it("returns null when already at Brilliance (top tier)", () => {
    expect(nextTier(tierById("brilliance"))).toBeNull();
  });
});

describe("sessionsToNext", () => {
  it("returns 5 at 0 sessions (Spark needs 5 to reach Glow)", () => {
    expect(sessionsToNext(0)).toBe(5);
  });

  it("returns 1 at 4 sessions (one away from Glow)", () => {
    expect(sessionsToNext(4)).toBe(1);
  });

  it("returns 0 when exactly at Brilliance threshold", () => {
    expect(sessionsToNext(80)).toBe(0);
  });

  it("returns 0 well above the top threshold", () => {
    expect(sessionsToNext(500)).toBe(0);
  });

  it("returns the correct delta mid-tier", () => {
    expect(sessionsToNext(10)).toBe(5);
  });
});
