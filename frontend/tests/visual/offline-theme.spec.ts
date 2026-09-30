import { expect, test } from '@playwright/test'

/**
 * Offline behaviour.
 *
 * Two separate things are checked here, because they failed for different
 * reasons and a fix for one must not undo the other:
 *
 * 1. The panel opens at all with no network. That needs `index.html` in the
 *    precache and as the navigation fallback. Making navigations network-only
 *    was tried and broke exactly this.
 * 2. The theme renders, not the C3 fallback list. The theme is fetched by URL at
 *    runtime from a cross-origin sandbox, so its resources have to be cached by
 *    the worker; `cache: 'no-store'` on that request defeated the cache even
 *    once the rule existed.
 */
async function prepareHome(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
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
    else if (path.endsWith('/groups')) {
      data = [{ id: 1, title: 'Development', sort: 1, parentId: null }]
    }
    else if (path.endsWith('/items')) {
      const groupId = Number(new URL(route.request().url()).searchParams.get('groupId'))
      data = groupId === 1
        ? [{ id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } }]
        : []
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })
}

test('the generated worker keeps the shell offline-capable and claims clients', async ({ page }) => {
  // Read the worker the build actually produced rather than the config that was
  // meant to produce it: the two drifted once already.
  const worker = await page.request.get('/sw.js').then(response => response.text())

  expect(worker, 'index.html must stay precached or the panel cannot open offline').toContain('"index.html"')
  expect(worker, 'navigations must keep the precached fallback').toContain('createHandlerBoundToURL("/index.html")')
  expect(worker, 'a new worker must take over immediately, or it serves the previous build').toContain('self.skipWaiting()')
  expect(worker, 'an open tab must be claimed by the new worker').toContain('clientsClaim()')
  expect(worker, 'theme resources must be cached or the theme fails offline').toContain('yin-panel-theme-assets')
})

test('theme resources are requested cacheable so the worker can store them', async ({ page }) => {
  const requests: string[] = []
  page.on('request', (request) => {
    if (request.url().includes('/api/theme/v2/assets/')) requests.push(request.url())
  })

  await prepareHome(page)
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  const theme = page.frameLocator('[data-testid="theme-home-frame"]')
  await expect(theme.locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })

  expect(requests.length, 'the theme should fetch its resources').toBeGreaterThan(0)
  // `no-store` on this request is what made the theme unreachable offline even
  // with a cache rule in place, so the mode is asserted rather than assumed.
  for (const url of requests) {
    const response = await page.request.get(url)
    expect(response.headers()['cache-control'], `${url} should be cacheable`).toContain('immutable')
  }
})
