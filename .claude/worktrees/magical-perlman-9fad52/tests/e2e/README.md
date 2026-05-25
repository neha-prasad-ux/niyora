# Playwright tests

End-to-end visual + functional tests for the Niyora React UI. They run against
the Vite dev server (`http://localhost:1420`) — **not** the full Tauri app —
and exercise all five Niyora Index tiers via the dev-only `?force-stress=N`
URL parameter handled by `useSnapshot`.

## Running

```bash
pnpm test:visual
```

Playwright will start the Vite dev server automatically (or reuse a running
one). The first run captures baseline screenshots; subsequent runs diff
against them.

## Viewing the report

```bash
pnpm test:visual:report
```

Opens the most recent HTML report in your browser, including diffs for any
failed visual tests.

## Updating baselines

When you intentionally change the design (new ball gradient, new layout, etc.)
the visual tests will fail because the rendered pixels no longer match the
checked-in baselines. Regenerate them:

```bash
pnpm test:visual:update
```

Then commit the updated PNGs under `tests/e2e/*-snapshots/`.

## Caveats

- Screenshots can drift slightly across machines and OS versions (font
  hinting, anti-aliasing, GPU compositing). The config allows up to 2 %
  pixel-ratio difference, but baselines may still need regenerating on a
  different host. Treat them as a design artefact, not a bit-exact contract.
- Tauri `invoke` calls fail in plain Vite (no Tauri runtime). The components
  are written to swallow those errors; the tests assert that the UI doesn't
  crash and that the dev-only URL-param overrides drive the visible state.
- The URL-param override is gated by `import.meta.env.DEV` and is dropped from
  production builds — these tests rely on running against `pnpm dev`.

## Available URL params (dev-only)

| Param           | Effect                                              |
| --------------- | --------------------------------------------------- |
| `force-stress`  | Score 0–100 (forces a synthesised snapshot).        |
| `label`         | `calm` \| `normal` \| `dense` \| `heavy` (override).|
| `msg`           | Contextual message under the ball.                  |
| `interval`      | Reminder interval in minutes.                       |
| `meeting=true`  | Sets `in_meeting: true`.                            |
| `afterhours=true` | Sets `after_hours: true`.                         |
