import { test } from "@playwright/test";

/**
 * Visual smoke test for the locked-row tooltip in the technique picker.
 * Captures a screenshot with hover so we can see where the tooltip lands.
 */

test("locked-row tooltip on hover screenshot", async ({ page }) => {
  await page.goto("/?force-sessions=0&force-stress=70");
  await page.waitForLoadState("domcontentloaded");

  // Open the picker.
  await page.getByRole("button", { name: "Try a different one" }).click();
  await page.waitForTimeout(200);

  // Find a locked row — Ocean Breath is locked at session count 0 (Radiance tier).
  const oceanRow = page.locator(".picker-row", { hasText: "Ocean Breath" });
  await oceanRow.hover({ force: true });
  await page.waitForTimeout(400); // allow tooltip transition

  await page.screenshot({ path: "test-results/picker-tooltip-hover.png", fullPage: false });
});
