import { expect, test } from '@playwright/test'

/**
 * The panel-supplied footer link ("Powered By Yin-Panel").
 *
 * The home renders inside `sandbox="allow-scripts"`, which has no `allow-popups`.
 * A `target="_blank"` link is therefore refused by the browser and logged as a
 * violation, so the theme hands the click to the Core, which opens it through the
 * same path a bookmark uses.
 */
async function prepareHome(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
  })
  // `window.open` is the observable effect; the real popup is never allowed to
  // open, both because the page is served by the test server and to keep the run
  // offline. Asserting on the call is also what makes the test independent of the
  // browser's own popup blocking.
  await page.addInitScript(() => {
    const target = window as unknown as { __openedUrls: string[] }
    target.__openedUrls = []
    const open = window.open.bind(window)
    window.open = ((url?: string | URL, ...rest: unknown[]) => {
      target.__openedUrls.push(String(url))
      return open(url, ...(rest as []))
    }) as typeof window.open
  })

  const current = await (await page.request.get('/api/theme/v2/current')).json()
  const revision = current.data.revision
  const permissions = (current.data.manifest.permissions?.required || []).map((item: { name: string }) => item.name)

  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    let data: unknown = {}

    if (path.startsWith('/api/theme/')) {
      if (path.startsWith('/api/theme/v2/grants/')) {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ code: 0, data: { revision, executionMode: 'sandbox', available: true, granted: true, permissions } }),
        })
      }
      else {
        await route.fallback()
      }
      return
    }

    if (path.endsWith('/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false } }
    }
    else if (path.endsWith('/getEnableStatus')) {
      data = { enabled: false }
    }
    else if (path.endsWith('/spaces')) {
      data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
    }
    else if (path.endsWith('/groups') || path.endsWith('/items')) {
      data = []
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })
}

test('the footer link opens through the Core instead of a blocked popup', async ({ page }) => {
  const sandboxViolations: string[] = []
  page.on('console', message => {
    const text = message.text()
    if (/allow-popups|Blocked opening|sandboxed frame/i.test(text)) sandboxViolations.push(text)
  })

  await prepareHome(page)
  await page.setViewportSize({ width: 1920, height: 1080 })
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  const theme = page.frameLocator('[data-testid="theme-home-frame"]')
  const link = theme.locator('.yin-footer a[href^="https://"]')
  await expect(link).toBeVisible({ timeout: 30_000 })
  await expect(link).toHaveAttribute('href', 'https://github.com/yinorg/Yin-Panel')
  // The `target` is the whole problem: without `allow-popups` the browser refuses
  // the navigation and logs a violation, so the attribute must not be there.
  expect(await link.getAttribute('target')).toBeNull()

  await link.click()
  await expect
    .poll(() => page.evaluate(() => (window as unknown as { __openedUrls: string[] }).__openedUrls), { timeout: 10_000 })
    .toEqual(['https://github.com/yinorg/Yin-Panel'])
  expect(sandboxViolations, 'the sandbox should not report a blocked window').toEqual([])
})
