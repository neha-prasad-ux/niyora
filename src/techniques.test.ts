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

  it("undefined fallback draws from the full pool, not only spark-tier", () => {
    const sparkNames = new Set(unlockedTechniques(0).map((t) => t.name));
    const nonSparkTechniques = techniques.filter((t) => !sparkNames.has(t.name));
    expect(nonSparkTechniques.length).toBeGreaterThan(0);
    const results = Array.from({ length: 50 }, () => randomTechnique(undefined));
    expect(results.some((t) => !sparkNames.has(t.name))).toBe(true);
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

describe("technique instruction content", () => {
  it("Box Breath instruction carries the nasal cue", () => {
    const box = breathingList.find((t) => t.name === "Box Breath")!;
    expect(box.instructions).toMatch(/Breathe through your nose/);
  });

  it("Belly Breath instruction carries the nasal cue", () => {
    const belly = breathingList.find((t) => t.name === "Belly Breath")!;
    expect(belly.instructions).toMatch(/Breathe through your nose/);
  });

  it("every technique has a non-empty instructions string", () => {
    for (const t of techniques) {
      expect(t.instructions.length).toBeGreaterThan(0);
    }
  });
});

describe("Box Breath phase structure", () => {
  it("has exactly 4 phases", () => {
    const box = breathingList.find((t) => t.name === "Box Breath")!;
    expect(box.phases).toHaveLength(4);
  });

  it("all phases have equal duration of 4s", () => {
    const box = breathingList.find((t) => t.name === "Box Breath")!;
    for (const phase of box.phases) {
      expect(phase.duration).toBe(4);
    }
  });

  it("phase sequence is inhale-hold-exhale-hold", () => {
    const box = breathingList.find((t) => t.name === "Box Breath")!;
    expect(box.phases.map((p) => p.type)).toEqual(["inhale", "hold", "exhale", "hold"]);
  });
});
