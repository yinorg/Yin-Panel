import { defineConfig, devices } from '@playwright/test'

// Compares the THEME-RENDERED home against the confirmed old-version ruler.
//
// This is the A-route companion to playwright.compare.config.ts: that config
// exercises the Core Vue home, this one forces the Yin theme package to render
// through the real sandbox pipeline so the theme's own markup/CSS is measured.
//
// Safety: updateSnapshots is pinned to 'none' so this config can never
// overwrite the ruler baselines. Regenerating the ruler is only possible
// through playwright.ruler.config.ts against the official image on :3003.
export default defineConfig({
  testDir: './tests/visual',
  testMatch: 'compare-theme-to-ruler.spec.ts',
  timeout: 45000,
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
