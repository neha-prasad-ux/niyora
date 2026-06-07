import { describe, it, expect } from "vitest";
import {
  techniques,
  breathingList,
  mindfulnessList,
  unlockedTechniques,
  randomTechnique,
} from "./techniques";

describe("breathingList / mindfulnessList", () => {
  it("every entry in breathingList has kind 'breathing'", () => {
    expect(breathingList.length).toBeGreaterThan(0);
    for (const t of breathingList) {
      expect(t.kind).toBe("breathing");
    }
  });

  it("every entry in mindfulnessList has kind 'mindfulness'", () => {
    expect(mindfulnessList.length).toBeGreaterThan(0);
    for (const t of mindfulnessList) {
      expect(t.kind).toBe("mindfulness");
    }
  });

  it("techniques is the union of both lists", () => {
    expect(techniques.length).toBe(breathingList.length + mindfulnessList.length);
  });
});

describe("unlockedTechniques", () => {
  it("at 0 sessions (spark) returns only spark-tier techniques", () => {
    const unlocked = unlockedTechniques(0);
    expect(unlocked.length).toBeGreaterThan(0);
    for (const t of unlocked) {
      expect(t.unlockTier).toBe("spark");
    }
  });

  it("at spark tier includes Box Breath and Belly Breath", () => {
    const unlocked = unlockedTechniques(0);
    const names = unlocked.map((t) => t.name);
    expect(names).toContain("Box Breath");
    expect(names).toContain("Belly Breath");
  });

  it("at glow tier (5 sessions) includes glow-unlocked Cooling Breath", () => {
    const unlocked = unlockedTechniques(5);
    const names = unlocked.map((t) => t.name);
    expect(names).toContain("Cooling Breath");
    expect(names).toContain("Box Breath");
  });

  it("at brilliance tier (80 sessions) unlocks all techniques", () => {
    const unlocked = unlockedTechniques(80);
    expect(unlocked.length).toBe(techniques.length);
  });

  it("each step up in tier adds more techniques (monotonically increasing)", () => {
    const counts = [0, 5, 15, 40, 80].map(
      (s) => unlockedTechniques(s).length
    );
    for (let i = 1; i < counts.length; i++) {
      expect(counts[i]).toBeGreaterThan(counts[i - 1]);
    }
  });
});

describe("randomTechnique", () => {
  it("returns Box Breath when completedSessions is 0 (first-session rule)", () => {
    const t = randomTechnique(0);
    expect(t.name).toBe("Box Breath");
  });

  it("returns a Technique when completedSessions is undefined", () => {
    const t = randomTechnique(undefined);
    expect(t).toBeDefined();
    expect(typeof t.name).toBe("string");
    expect(techniques.some((u) => u.name === t.name)).toBe(true);
  });

  it("returns a technique within the unlocked set for a known session count", () => {
    const sessions = 5;
    const unlocked = unlockedTechniques(sessions);
    const unlockedNames = new Set(unlocked.map((t) => t.name));
    for (let i = 0; i < 20; i++) {
      const t = randomTechnique(sessions);
      expect(unlockedNames.has(t.name)).toBe(true);
    }
  });

  it("never returns a technique locked above the current tier", () => {
    const sessions = 2;
    const sparkOnlyNames = new Set(unlockedTechniques(sessions).map((t) => t.name));
    for (let i = 0; i < 30; i++) {
      const t = randomTechnique(sessions);
      expect(sparkOnlyNames.has(t.name)).toBe(true);
    }
  });
});
