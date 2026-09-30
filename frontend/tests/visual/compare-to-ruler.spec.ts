import { expect, test } from '@playwright/test'

// Compares what the Core still owns against the confirmed old-version ruler.
// Scenario names and viewports must stay in lockstep with ruler-capture.spec.ts,
// otherwise the snapshot lookup below will not find the ruler image.

const VIEWPORTS = [
  { name: 'desktop', width: 1920, height: 1080 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'mobile', width: 375, height: 667 },
]

// The Core-rendered home is no longer compared here: P4a removed that
// implementation, so this suite's home baselines had no subject left. Pixel
// fidelity of the surviving home is covered by `playwright.theme-compare.config.ts`
// against the same pre-theme baselines, and the Core-rendered fallback view is
// covered behaviourally by `functional-regression.spec.ts`. The login comparison
// below stays: the login page is Core-owned and untouched by that change.

for (const viewport of VIEWPORTS) {
  test.describe(`Compare: Login page - ${viewport.name}`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height } })

    test.beforeEach(async ({ page }) => {
      await page.route('**/api/**', async (route) => {
        const path = new URL(route.request().url()).pathname
        if (path.endsWith('/oauth/config')) {
          await route.fulfill({
            status: 200,
            contentType: 'application/json',
            body: JSON.stringify({ code: 0, data: { enabled: false, providers: [] } }),
          })
        } else {
          await route.fulfill({ status: 404, body: 'Not found' })
        }
      })
    })

    test('login page', async ({ page }) => {
      await page.goto('/login', { waitUntil: 'networkidle' })
      await expect(page.locator('.login-container')).toBeVisible({ timeout: 15000 })
      await page.waitForTimeout(1500)
      await expect(page.locator('.login-container')).toHaveScreenshot(`login-${viewport.name}.png`, {
        animations: 'disabled',
      })
    })
  })
}
