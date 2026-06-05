import { describe, it, expect, vi, afterEach } from "vitest";

describe("isWindows", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("returns false for a macOS userAgent", async () => {
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
    });
    const { isWindows } = await import("./platform");
    expect(isWindows).toBe(false);
  });

  it("returns true for a Windows userAgent", async () => {
    vi.stubGlobal("navigator", {
      userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
    });
    const { isWindows } = await import("./platform");
    expect(isWindows).toBe(true);
  });

  it("returns false when userAgent is empty", async () => {
    vi.stubGlobal("navigator", { userAgent: "" });
    const { isWindows } = await import("./platform");
    expect(isWindows).toBe(false);
  });
});
