import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests/visual',
  // The suite drives one single-process server, and nearly every spec boots the
  // theme in a frame that then loads further async chunks. Run in parallel, specs
  // that wait for the theme to become interactive intermittently fail with the
  // theme rendered but holding no permissions: the grant response loses a race the
  // specs do not control. Serial was clean across eight repeated runs; parallel
  // flakes about one run in three.
  workers: 1,
  testIgnore: [
    '**/tests/unit/**',
    // The ruler toolchain runs against its own base URL and snapshot folder, and
    // each of these three has a dedicated config that owns both:
    // `ruler-capture.spec.ts` captures the *old* build on port 3003
    // (`playwright.ruler.config.ts`), and the two compare specs read those
    // captured images under `playwright.compare.config.ts` and
    // `playwright.theme-compare.config.ts`. Running them here points them at the
    // current build on port 3002, where the old DOM they look for no longer
    // exists, and at `ruler-snapshots/`, which is generated and not committed.
    '**/tests/visual/ruler-capture.spec.ts',
    '**/tests/visual/compare-to-ruler.spec.ts',
    '**/tests/visual/compare-theme-to-ruler.spec.ts',
  ],
  timeout: 60_000,
  fullyParallel: true,
  reporter: [['html', { outputFolder: 'playwright-report-visual' }], ['line']],
  use: {
    baseURL: 'http://127.0.0.1:3002',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium-desktop',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1920, height: 1080 } },
    },
    {
      name: 'chromium-tablet',
      use: { ...devices['Desktop Chrome'], viewport: { width: 768, height: 1024 } },
    },
    {
      name: 'chromium-mobile',
      use: { ...devices['Desktop Chrome'], viewport: { width: 375, height: 667 } },
    },
    {
      name: 'firefox-desktop',
      use: { ...devices['Desktop Firefox'], viewport: { width: 1920, height: 1080 } },
    },
    {
      name: 'webkit-desktop',
      use: { ...devices['Desktop Safari'], viewport: { width: 1920, height: 1080 } },
    },
  ],
  webServer: {
    command: 'echo "Using existing server"',
    port: 3002,
    reuseExistingServer: true,
    timeout: 10_000,
  },
  snapshotDir: './tests/visual/snapshots',
  expect: {
    toHaveScreenshot: {
      maxDiffPixels: 100,
      threshold: 0.2,
    },
  },
})