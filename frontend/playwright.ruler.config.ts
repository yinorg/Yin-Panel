import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './tests/visual',
  snapshotDir: './tests/visual/ruler-snapshots',
  timeout: 30000,
  expect: {
    toHaveScreenshot: {
      maxDiffPixels: 100,
      maxDiffPixelRatio: 0.002,
      threshold: 0.2,
    },
  },
  use: {
    baseURL: 'http://127.0.0.1:3003',
    // Block the service worker so the "offline ready" badge cannot appear in
    // one capture and not the other; it depends on SW control timing, which is
    // environmental rather than a visual difference.
    serviceWorkers: 'block',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    { name: 'chromium-desktop', use: { ...devices['Desktop Chrome'], viewport: { width: 1920, height: 1080 } } },
  ],
  reporter: [['list'], ['html', { outputFolder: 'playwright-report-ruler', open: 'never' }]],
  testIgnore: ['**/tests/unit/**', '**/tests/visual/debug-command-center.spec.ts'],
})