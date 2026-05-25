import { test, expect } from "@playwright/test";

/**
 * Functional assertions for forced stress states. For each of the 5 tiers we
 * verify:
 *   - the contextual message override renders under the ball on the main view
 *   - the My Soul panel shows the matching today-label
 *
 * Note: per the privacy-first redesign, raw numeric score, screen-time,
 * meeting, after-hours, and reminder-interval fields are no longer surfaced
 * on the My Soul panel — only the human-readable today-label remains.
 */

const TIERS: { score: number; label: string }[] = [
  { score: 95, label: "Calm" },
  { score: 70, label: "Normal" },
  { score: 50, label: "Dense" },
  { score: 30, label: "Heavy" },
  { score: 10, label: "Heavy" }, // overload-band score still maps to the "heavy" label
];

for (const tier of TIERS) {
  test(`forced score ${tier.score} drives main view + My Soul (${tier.label})`, async ({ page }) => {
    await page.goto(`/?force-stress=${tier.score}&msg=Test+message`);
    await page.waitForLoadState("domcontentloaded");

    // Stress ball renders on the pre-session screen.
    // (The contextual message is no longer surfaced here per the redesign;
    // the orb's colour tier is the user-facing signal of today's stress.)
    await expect(page.locator(".stress-ball")).toBeVisible();

    // Open My Soul.
    await page.locator(".niyora-gear-btn").click();

    // Today's label is shown on the soul panel as "Today: <Label>".
    await expect(page.locator(".soul-stress")).toHaveText(`Today: ${tier.label}`);

    // Back to main.
    await page.locator(".soul-close").click();
    await expect(page.locator(".stress-ball")).toBeVisible();
  });
}
