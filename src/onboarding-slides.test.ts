import { describe, it, expect, vi } from "vitest";

vi.mock("./platform", () => ({ isWindows: false }));

describe("SLIDES", () => {
  it("has 4 slides on non-Windows (pledge + QR)", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES).toHaveLength(4);
  });

  it("slide 0 is the value-prop slide", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES[0].eyebrow).toBe("Niyora");
    expect(SLIDES[0].bullets).toBeDefined();
  });

  it("slide 1 is the privacy slide", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES[1].title).toBe("Private by design");
    expect(SLIDES[1].noteHead).toBeDefined();
  });

  it("slide 2 is the pledge slide and has no note panel or checklist", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    const pledge = SLIDES[2];
    expect(pledge.title).toContain("7 days");
    expect(pledge.note).toBeUndefined();
    expect(pledge.noteHead).toBeUndefined();
    expect(pledge.bullets).toBeUndefined();
  });

  it("pledge slide durationMs is at least 5000", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES[2].durationMs).toBeGreaterThanOrEqual(5000);
  });

  it("slide 3 is the App Store QR slide on non-Windows", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    const qr = SLIDES[3];
    expect(qr.qrSlide).toBe(true);
    expect(qr.title).toBe("Niyora, in your pocket too.");
  });
});
