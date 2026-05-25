import { test, expect, type Page } from "@playwright/test";

/**
 * Visual regression for the stress ball across all 5 Niyora Index tiers.
 *
 * Each test forces a specific score via the `?force-stress=N` URL param
 * (handled by `useSnapshot` in dev mode), waits for the ball to render,
 * disables animations to keep the screenshot stable, then snapshots the
 * `.stress-ball` element only.
 */

const TIERS: { score: number; name: string }[] = [
  { score: 95, name: "calm" },
  { score: 70, name: "normal" },
  { score: 50, name: "dense" },
  { score: 30, name: "heavy" },
  { score: 10, name: "overload" },
];

async function freezeAnimations(page: Page) {
  // `animation: none` alone freezes the animation at whatever frame it happens
  // to be on, which gives non-deterministic screenshots. Instead we explicitly
  // pin the animated properties to their 0% keyframe values:
  //   - .stress-ball breathing pulse: transform scale 1 → 1.04 → 1 → freeze at scale(1)
  //   - .stress-ball::before cloud spin: background-position 0 → -240px → freeze at 0
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        animation: none !important;
        transition: none !important;
        animation-duration: 0s !important;
        animation-delay: 0s !important;
      }
      .stress-ball {
        transform: scale(1) !important;
      }
      .stress-ball::before {
        background-position: 0 50% !important;
      }
    `,
  });
}

for (const tier of TIERS) {
  test(`stress ball renders correctly at score ${tier.score} (${tier.name})`, async ({ page }) => {
    await page.goto(`/?force-stress=${tier.score}`);
    await page.waitForLoadState("domcontentloaded");

    const ball = page.locator(".stress-ball");
    await expect(ball).toBeVisible();

    // Let the score-driven gradient settle.
    await page.waitForTimeout(200);

    // Disable animations for a stable pixel-comparison.
    await freezeAnimations(page);

    // Tiny extra tick to let the no-animation styles take effect.
    await page.waitForTimeout(50);

    await expect(ball).toHaveScreenshot(`ball-${tier.score}.png`);
  });
}
