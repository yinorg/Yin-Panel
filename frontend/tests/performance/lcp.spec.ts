import { expect, test, type Page } from '@playwright/test'

async function mockApi(page: Page) {
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    let data: unknown = {}

    if (path.endsWith('/oauth/config')) {
      data = { enabled: false, providers: [] }
    }
    else if (path.endsWith('/spaces')) {
      data = []
    }
    else if (path.endsWith('/getEnableStatus')) {
      data = { enabled: false }
    }
    else if (path.endsWith('/getConfig')) {
      data = { panel: {} }
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })
}

async function readLcp(page: Page) {
  return await page.evaluate(() => (window as typeof window & {
    __lcpMetric?: { time: number; marker?: string; text?: string }
  }).__lcpMetric ?? null)
}

for (const path of ['/', '/login']) {
  test(`${path} renders the brand within the LCP budget`, async ({ page }) => {
    await mockApi(page)
    await page.addInitScript(() => {
      const target = window as typeof window & {
        __lcpMetric?: { time: number; marker?: string; text?: string }
      }
      new PerformanceObserver((list) => {
        const entry = list.getEntries().at(-1) as PerformanceEntry & { element?: Element | null }
        if (entry) {
          target.__lcpMetric = {
            time: entry.startTime,
            marker: entry.element?.getAttribute('data-lcp') ?? undefined,
            text: entry.element?.textContent?.trim() ?? undefined,
          }
        }
      }).observe({ type: 'largest-contentful-paint', buffered: true })
    })
    await page.goto(path, { waitUntil: 'domcontentloaded' })
    await expect(page.locator('[data-lcp="brand"]').last()).toBeVisible()
    await expect.poll(() => readLcp(page), { timeout: 5000 }).toBeTruthy()
    const lcp = await readLcp(page)
    expect(lcp?.time).toBeLessThan(2500)
    expect(lcp?.marker).toBe('brand')
    expect(lcp?.text).toContain('Yin-Panel')
  })
}

test('service worker does not fall back API navigations to index.html', async ({ request }) => {
  const response = await request.get('/sw.js')
  expect(response.ok()).toBeTruthy()

  const serviceWorker = await response.text()
  expect(serviceWorker).toContain('denylist:[/^\\/api(?:\\/|$)/]')
})
