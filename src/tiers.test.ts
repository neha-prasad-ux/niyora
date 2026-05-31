import { describe, it, expect } from "vitest";
import {
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
  it("returns the concrete TierColor for spark", () => {
    const c = tierColor(tierById("spark"));
    expect(c.hue).toBe(30);
    expect(c.saturation).toBe(70);
    expect(c.lightness).toBe(60);
  });

});

describe("tierHue", () => {
  it("returns the correct hue for each tier", () => {
    expect(tierHue(tierById("spark"))).toBe(30);
    expect(tierHue(tierById("glow"))).toBe(335);
    expect(tierHue(tierById("shine"))).toBe(280);
    expect(tierHue(tierById("radiance"))).toBe(230);
    expect(tierHue(tierById("brilliance"))).toBe(210);
  });
});

describe("currentTier", () => {
  it("returns spark at 0 sessions", () => {
    expect(currentTier(0).id).toBe("spark");
  });

  it("returns spark below first upgrade threshold", () => {
    expect(currentTier(4).id).toBe("spark");
  });

  it("stays at glow below shine threshold", () => {
    expect(currentTier(14).id).toBe("glow");
  });

  it("stays at shine below radiance threshold", () => {
    expect(currentTier(39).id).toBe("shine");
  });

  it("stays at radiance below brilliance threshold", () => {
    expect(currentTier(79).id).toBe("radiance");
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

  it("returns 1 when one session away from brilliance", () => {
    expect(sessionsToNext(79)).toBe(1);
  });

  it("returns 0 at the top tier", () => {
    expect(sessionsToNext(80)).toBe(0);
    expect(sessionsToNext(200)).toBe(0);
  });

  it("counts remaining sessions correctly mid-tier", () => {
    // At 7 sessions (glow tier), next is shine at 15: 15 - 7 = 8
    expect(sessionsToNext(7)).toBe(8);
    // At 17 sessions (shine tier), next is radiance at 40: 40 - 17 = 23
    expect(sessionsToNext(17)).toBe(23);
  });

  it("returns sessions to next tier when count is exactly on a non-top threshold", () => {
    // At 5 sessions (glow, threshold 5), next is shine at 15: 15 - 5 = 10
    expect(sessionsToNext(5)).toBe(10);
    // At 15 sessions (shine, threshold 15), next is radiance at 40: 40 - 15 = 25
    expect(sessionsToNext(15)).toBe(25);
  });

  it("Math.max guard returns non-negative when count is above the former next-tier threshold", () => {
    // count 41 is above shine's next threshold (radiance at 40); currentTier advances to radiance.
    // nextTier is now brilliance at 80. Math.max(0, 80 - 41) = 39, not negative.
    expect(sessionsToNext(41)).toBe(39);
  });
});
