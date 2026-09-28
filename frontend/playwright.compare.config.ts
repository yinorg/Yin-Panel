import { defineConfig, devices } from '@playwright/test'

// Compares the CURRENT build against the confirmed old-version ruler.
//
// Safety: updateSnapshots is pinned to 'none' so this config can never
// overwrite the ruler baselines. Regenerating the ruler is only possible
// through playwright.ruler.config.ts against the official image on :3003.
export default defineConfig({
  testDir: './tests/visual',
  testMatch: 'compare-to-ruler.spec.ts',
  timeout: 30000,
  updateSnapshots: 'none',
  expect: {
    toHaveScreenshot: {
      maxDiffPixels: 100,
      maxDiffPixelRatio: 0.002,
      threshold: 0.2,
    },
  },
  use: {
    baseURL: 'http://127.0.0.1:3002',
    // Match the ruler capture: blocking the service worker keeps the "offline
    // ready" badge out of both images so it is not reported as a difference.
    serviceWorkers: 'block',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    { name: 'chromium-desktop', use: { ...devices['Desktop Chrome'], viewport: { width: 1920, height: 1080 } } },
  ],
  reporter: [['list']],
  // Resolve expected screenshots to the read-only ruler directory.
  snapshotPathTemplate: '{testDir}/ruler-snapshots/ruler-capture.spec.ts-snapshots/{arg}{-projectName}{-snapshotSuffix}{ext}',
})
