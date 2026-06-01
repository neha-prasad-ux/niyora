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

describe("TIERS constant", () => {
  it("has five tiers in progression order", () => {
    const ids = TIERS.map((t) => t.id);
    expect(ids).toEqual(["spark", "glow", "shine", "radiance", "brilliance"]);
  });

  it("has non-decreasing thresholds starting at 0", () => {
    expect(TIERS[0].threshold).toBe(0);
    for (let i = 1; i < TIERS.length; i++) {
      expect(TIERS[i].threshold).toBeGreaterThan(TIERS[i - 1].threshold);
    }
  });
});

describe("tierById", () => {
  it("returns the correct tier for each valid id", () => {
    for (const tier of TIERS) {
      expect(tierById(tier.id)).toBe(tier);
    }
  });

  it("throws for an unknown id", () => {
    // @ts-expect-error - intentionally testing invalid input
    expect(() => tierById("unknown")).toThrow("Unknown tier: unknown");
  });
});

describe("tierColor", () => {
  it("returns the color record for the given tier", () => {
    const spark = tierById("spark");
    expect(tierColor(spark)).toEqual(TIER_COLORS["spark"]);
    expect(tierColor(spark).hue).toBe(30);
  });

  it("returns distinct colors for each tier", () => {
    const hues = TIERS.map((t) => tierColor(t).hue);
    expect(new Set(hues).size).toBe(TIERS.length);
  });
});

describe("tierHue", () => {
  it("returns the hue from the tier's color definition", () => {
    for (const tier of TIERS) {
      expect(tierHue(tier)).toBe(TIER_COLORS[tier.id].hue);
    }
  });
});

describe("currentTier", () => {
  it("returns spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("stays at spark just below glow threshold", () => {
    expect(currentTier(4).id).toBe("spark");
  });

  it("advances to glow at threshold 5", () => {
    expect(currentTier(5).id).toBe("glow");
  });

  it("advances to shine at threshold 15", () => {
    expect(currentTier(15).id).toBe("shine");
  });

  it("stays at shine just below radiance threshold", () => {
    expect(currentTier(39).id).toBe("shine");
  });

  it("advances to radiance at threshold 40", () => {
    expect(currentTier(40).id).toBe("radiance");
  });

  it("advances to brilliance at threshold 80", () => {
    expect(currentTier(80).id).toBe("brilliance");
  });

  it("stays at brilliance above the final threshold", () => {
    expect(currentTier(200).id).toBe("brilliance");
  });
});

describe("nextTier", () => {
  it("returns glow as next after spark", () => {
    expect(nextTier(tierById("spark"))?.id).toBe("glow");
  });

  it("returns null at the top tier (brilliance)", () => {
    expect(nextTier(tierById("brilliance"))).toBeNull();
  });

  it("returns the immediately following tier for mid tiers", () => {
    expect(nextTier(tierById("glow"))?.id).toBe("shine");
    expect(nextTier(tierById("shine"))?.id).toBe("radiance");
    expect(nextTier(tierById("radiance"))?.id).toBe("brilliance");
  });
});

describe("sessionsToNext", () => {
  it("returns sessions needed from 0 to reach glow", () => {
    expect(sessionsToNext(0)).toBe(5);
  });

  it("counts down correctly as sessions increase", () => {
    expect(sessionsToNext(3)).toBe(2);
  });

  it("returns 0 once at or past next threshold", () => {
    expect(sessionsToNext(5)).toBe(10);
  });

  it("returns 0 at the top tier (brilliance)", () => {
    expect(sessionsToNext(80)).toBe(0);
    expect(sessionsToNext(200)).toBe(0);
  });
});
