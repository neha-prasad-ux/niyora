import { test, expect } from "@playwright/test";

/**
 * Basic interaction flows. These exercise the React UI in plain Vite (no
 * Tauri runtime) — Tauri `invoke` calls fail, but the components are written
 * to swallow those errors and still drive the UI forward.
 */

test("Begin button click triggers the info-screen exit transition", async ({ page }) => {
  await page.goto("/?force-stress=70");
  await page.waitForLoadState("domcontentloaded");

  await expect(page.locator(".stress-ball")).toBeVisible();

  await page.getByRole("button", { name: "Begin" }).click();

  // The transitioning state adds the `info-exit-content` class.
  await expect(page.locator(".info-exit-content")).toBeVisible({ timeout: 3_000 });
});

test("My Soul close button returns to main view", async ({ page }) => {
  await page.goto("/?force-stress=70");
  await page.waitForLoadState("domcontentloaded");

  await page.locator(".niyora-gear-btn").click();
  await expect(page.locator(".soul-header")).toBeVisible();

  await page.locator(".soul-close").click();

  // Main view: stress ball + gear button reappear.
  await expect(page.locator(".stress-ball")).toBeVisible();
  await expect(page.locator(".niyora-gear-btn")).toBeVisible();
});

test("mood capture handles tauri-invoke failure gracefully", async ({ page }) => {
  // No real way to land on the mood view from URL alone, so we just verify
  // that loading the app, kicking off a session, and letting the page idle
  // doesn't throw any uncaught errors. This is a smoke test that the
  // try/catch around `invoke('log_event', ...)` etc. holds in browser-only.
  const errors: string[] = [];
  page.on("pageerror", (err) => errors.push(err.message));

  await page.goto("/?force-stress=70");
  await page.waitForLoadState("domcontentloaded");

  // Click Begin — kicks off a session that will eventually call invoke.
  await page.getByRole("button", { name: "Begin" }).click();
  await page.waitForTimeout(500);

  expect(errors, `Page errors: ${errors.join(", ")}`).toHaveLength(0);
});

test("PSS-4 weekly check-in does not auto-popup on load", async ({ page }) => {
  // Per v1 spec, PSS-4 was moved out of the auto-popup path and will live
  // inside the My Soul profile area. Loading the app should always land on
  // the breathing main view, regardless of weekday.
  await page.goto("/?force-stress=70");
  await page.waitForLoadState("domcontentloaded");

  await expect(page.locator(".stress-ball")).toBeVisible();
  await expect(page.locator(".niyora-gear-btn")).toBeVisible();
  // The PSS-4 view container should never appear on load.
  await expect(page.locator(".niyora-pss4")).toHaveCount(0);
});
