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
    expect(tierById("spark").id).toBe("spark");
    expect(tierById("glow").id).toBe("glow");
    expect(tierById("shine").id).toBe("shine");
    expect(tierById("radiance").id).toBe("radiance");
    expect(tierById("brilliance").id).toBe("brilliance");
  });

  it("throws for an unknown id", () => {
    expect(() => tierById("unknown" as never)).toThrow("Unknown tier");
  });
});

describe("tierColor", () => {
  it("returns the TierColor matching TIER_COLORS for each tier", () => {
    for (const tier of TIERS) {
      expect(tierColor(tier)).toEqual(TIER_COLORS[tier.id]);
    }
  });

  it("returns the expected hue for spark", () => {
    expect(tierColor(tierById("spark")).hue).toBe(30);
  });

  it("returns the expected hue for brilliance", () => {
    expect(tierColor(tierById("brilliance")).hue).toBe(210);
  });
});

describe("tierHue", () => {
  it("returns TIER_COLORS hue for each tier", () => {
    for (const tier of TIERS) {
      expect(tierHue(tier)).toBe(TIER_COLORS[tier.id].hue);
    }
  });
});

describe("currentTier", () => {
  it("returns spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("returns spark below glow threshold", () => {
    expect(currentTier(4).id).toBe("spark");
  });

  it("returns glow at exactly the glow threshold (5)", () => {
    expect(currentTier(5).id).toBe("glow");
  });

  it("returns glow between glow and shine thresholds", () => {
    expect(currentTier(10).id).toBe("glow");
  });

  it("returns shine at exactly the shine threshold (15)", () => {
    expect(currentTier(15).id).toBe("shine");
  });

  it("returns radiance at exactly the radiance threshold (40)", () => {
    expect(currentTier(40).id).toBe("radiance");
  });

  it("returns brilliance at exactly the brilliance threshold (80)", () => {
    expect(currentTier(80).id).toBe("brilliance");
  });

  it("returns brilliance well above the top threshold", () => {
    expect(currentTier(500).id).toBe("brilliance");
  });
});

describe("nextTier", () => {
  it("returns glow as the next tier above spark", () => {
    expect(nextTier(tierById("spark"))?.id).toBe("glow");
  });

  it("returns shine as the next tier above glow", () => {
    expect(nextTier(tierById("glow"))?.id).toBe("shine");
  });

  it("returns radiance as the next tier above shine", () => {
    expect(nextTier(tierById("shine"))?.id).toBe("radiance");
  });

  it("returns brilliance as the next tier above radiance", () => {
    expect(nextTier(tierById("radiance"))?.id).toBe("brilliance");
  });

  it("returns null at the top tier (brilliance)", () => {
    expect(nextTier(tierById("brilliance"))).toBeNull();
  });
});

describe("sessionsToNext", () => {
  it("returns 5 at 0 sessions (spark, next is glow at 5)", () => {
    expect(sessionsToNext(0)).toBe(5);
  });

  it("returns 1 at 4 sessions (one away from glow)", () => {
    expect(sessionsToNext(4)).toBe(1);
  });

  it("returns 10 at exactly glow threshold (next is shine at 15)", () => {
    expect(sessionsToNext(5)).toBe(10);
  });

  it("returns 0 at or above brilliance threshold", () => {
    expect(sessionsToNext(80)).toBe(0);
    expect(sessionsToNext(200)).toBe(0);
  });
});
