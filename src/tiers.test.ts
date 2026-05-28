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
  it("returns color matching TIER_COLORS for each tier", () => {
    for (const tier of TIERS) {
      expect(tierColor(tier)).toEqual(TIER_COLORS[tier.id]);
    }
  });
});

describe("tierHue", () => {
  it("returns the hue field from TIER_COLORS", () => {
    for (const tier of TIERS) {
      expect(tierHue(tier)).toBe(TIER_COLORS[tier.id].hue);
    }
  });

  it("hue values match documented progression (warm to cool)", () => {
    expect(tierHue(tierById("spark"))).toBe(30);
    expect(tierHue(tierById("brilliance"))).toBe(210);
  });
});

describe("currentTier", () => {
  it("returns Spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("returns Spark below Glow threshold", () => {
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

  it("returns Brilliance well above the top threshold", () => {
    expect(currentTier(500).id).toBe("brilliance");
  });
});

describe("nextTier", () => {
  it("returns Glow after Spark", () => {
    expect(nextTier(tierById("spark"))?.id).toBe("glow");
  });

  it("returns Shine after Glow", () => {
    expect(nextTier(tierById("glow"))?.id).toBe("shine");
  });

  it("returns Radiance after Shine", () => {
    expect(nextTier(tierById("shine"))?.id).toBe("radiance");
  });

  it("returns Brilliance after Radiance", () => {
    expect(nextTier(tierById("radiance"))?.id).toBe("brilliance");
  });

  it("returns null at the top tier (Brilliance)", () => {
    expect(nextTier(tierById("brilliance"))).toBeNull();
  });
});

describe("sessionsToNext", () => {
  it("returns 5 at 0 sessions (need Glow)", () => {
    expect(sessionsToNext(0)).toBe(5);
  });

  it("returns 1 at 4 sessions (one away from Glow)", () => {
    expect(sessionsToNext(4)).toBe(1);
  });

  it("returns 10 at 5 sessions (need Shine at 15)", () => {
    expect(sessionsToNext(5)).toBe(10);
  });

  it("returns 0 at or above Brilliance threshold", () => {
    expect(sessionsToNext(80)).toBe(0);
    expect(sessionsToNext(200)).toBe(0);
  });
});
