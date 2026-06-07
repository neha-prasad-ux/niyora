import { describe, it, expect, vi } from "vitest";

vi.mock("./platform", () => ({ isWindows: true }));

describe("SLIDES (Windows)", () => {
  it("has 3 slides on Windows (no QR slide)", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES).toHaveLength(3);
  });

  it("slide 0 bullets say 'Data stays on your device' on Windows", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES[0].bullets?.[0]).toBe("Data stays on your device");
  });

  it("slide 1 note mentions Windows on Windows", async () => {
    const { SLIDES } = await import("./onboarding-slides");
    expect(SLIDES[1].note).toContain("Windows");
  });
});
