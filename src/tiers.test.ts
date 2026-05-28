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
  it("returns the correct tier for a valid id", () => {
    const t = tierById("spark");
    expect(t.id).toBe("spark");
    expect(t.name).toBe("Spark");
    expect(t.threshold).toBe(0);
  });

  it("returns brilliance tier", () => {
    const t = tierById("brilliance");
    expect(t.id).toBe("brilliance");
    expect(t.threshold).toBe(80);
  });

  it("throws for an unknown id", () => {
    expect(() => tierById("unknown" as never)).toThrow("Unknown tier: unknown");
  });
});

describe("tierColor", () => {
  it("returns the TierColor for a tier", () => {
    const t = tierById("spark");
    const c = tierColor(t);
    expect(c).toEqual(TIER_COLORS["spark"]);
    expect(c.hue).toBe(TIER_COLORS["spark"].hue);
  });

  it("returns distinct colors per tier", () => {
    const hues = TIERS.map((t) => TIER_COLORS[t.id].hue);
    const unique = new Set(hues);
    expect(unique.size).toBe(TIERS.length);
  });
});

describe("tierHue", () => {
  it("returns hue matching TIER_COLORS", () => {
    for (const t of TIERS) {
      expect(tierHue(t)).toBe(TIER_COLORS[t.id].hue);
    }
  });
});

describe("currentTier", () => {
  it("returns spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("returns spark below first upgrade threshold", () => {
    expect(currentTier(4).id).toBe("spark");
  });

  it("advances to glow at exactly 5 sessions", () => {
    expect(currentTier(5).id).toBe("glow");
  });

  it("advances to shine at exactly 15 sessions", () => {
    expect(currentTier(15).id).toBe("shine");
  });

  it("advances to radiance at exactly 40 sessions", () => {
    expect(currentTier(40).id).toBe("radiance");
  });

  it("reaches brilliance at exactly 80 sessions", () => {
    expect(currentTier(80).id).toBe("brilliance");
  });

  it("stays at brilliance well above 80", () => {
    expect(currentTier(500).id).toBe("brilliance");
  });
});

describe("nextTier", () => {
  it("returns glow as the next tier after spark", () => {
    const spark = tierById("spark");
    const next = nextTier(spark);
    expect(next?.id).toBe("glow");
  });

  it("returns null at the top tier (brilliance)", () => {
    const brilliance = tierById("brilliance");
    expect(nextTier(brilliance)).toBeNull();
  });

  it("returns shine after glow", () => {
    const glow = tierById("glow");
    expect(nextTier(glow)?.id).toBe("shine");
  });
});

describe("sessionsToNext", () => {
  it("returns sessions needed to reach glow from 0", () => {
    expect(sessionsToNext(0)).toBe(5);
  });

  it("returns 1 when one session away from glow", () => {
    expect(sessionsToNext(4)).toBe(1);
  });

  it("returns 0 at the top tier", () => {
    expect(sessionsToNext(80)).toBe(0);
    expect(sessionsToNext(200)).toBe(0);
  });

  it("counts remaining sessions correctly mid-tier", () => {
    // At 7 sessions (glow tier), next is shine at 15: 15 - 7 = 8
    expect(sessionsToNext(7)).toBe(8);
  });

  it("Math.max guard does not clamp a count already above the current tier threshold", () => {
    // At 17 sessions (shine tier, threshold 15) next is radiance at 40: Math.max(0, 40 - 17) = 23
    expect(sessionsToNext(17)).toBe(23);
  });

  it("Math.max guard returns non-negative when count is above the former next-tier threshold", () => {
    // count 41 is above shine's next threshold (radiance at 40); currentTier advances to radiance.
    // nextTier is now brilliance at 80. Math.max(0, 80 - 41) = 39, not negative.
    expect(sessionsToNext(41)).toBe(39);
  });
});
