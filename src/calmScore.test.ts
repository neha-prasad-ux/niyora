import { describe, it, expect } from "vitest";
import { calmScore, calmLabel } from "./calmScore";

describe("calmScore", () => {
  it("returns null for null input", () => {
    expect(calmScore(null)).toBeNull();
  });

  it("returns null for undefined input", () => {
    expect(calmScore(undefined)).toBeNull();
  });

  it("returns null for NaN", () => {
    expect(calmScore(NaN)).toBeNull();
  });

  it("returns null for Infinity", () => {
    expect(calmScore(Infinity)).toBeNull();
  });

  it("returns null for negative HRV", () => {
    expect(calmScore(-1)).toBeNull();
  });

  it("returns 0 at HRV = 0", () => {
    expect(calmScore(0)).toBe(0);
  });

  it("returns 100 at HRV = 80 (upper cap)", () => {
    expect(calmScore(80)).toBe(100);
  });

  it("returns 100 for HRV above cap", () => {
    expect(calmScore(120)).toBe(100);
    expect(calmScore(200)).toBe(100);
  });

  it("maps midpoint breakpoints linearly", () => {
    expect(calmScore(20)).toBe(25);
    expect(calmScore(40)).toBe(50);
    expect(calmScore(60)).toBe(75);
  });

  it("rounds fractional results", () => {
    // 10ms -> (10/80)*100 = 12.5 -> rounds to 13
    expect(calmScore(10)).toBe(13);
  });
});

describe("calmLabel", () => {
  it("returns 'no reading' for null", () => {
    expect(calmLabel(null)).toBe("no reading");
  });

  it("returns 'deeply calm' at 80+", () => {
    expect(calmLabel(80)).toBe("deeply calm");
    expect(calmLabel(100)).toBe("deeply calm");
  });

  it("returns 'calm' for 60-79", () => {
    expect(calmLabel(60)).toBe("calm");
    expect(calmLabel(79)).toBe("calm");
  });

  it("returns 'neutral' for 40-59", () => {
    expect(calmLabel(40)).toBe("neutral");
    expect(calmLabel(59)).toBe("neutral");
  });

  it("returns 'activated' for 20-39", () => {
    expect(calmLabel(20)).toBe("activated");
    expect(calmLabel(39)).toBe("activated");
  });

  it("returns 'highly activated' below 20", () => {
    expect(calmLabel(0)).toBe("highly activated");
    expect(calmLabel(19)).toBe("highly activated");
  });
});
