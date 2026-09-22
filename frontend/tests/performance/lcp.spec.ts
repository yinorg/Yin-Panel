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

test('home keeps layout stable while delayed space data loads', async ({ page }) => {
  const groups = [1, 2, 3].map((id) => ({ id, title: `Group ${id}`, sort: id, parentId: null }))
  const items = Array.from({ length: 8 }, (_, index) => ({
    id: index + 1,
    title: `Item ${index + 1}`,
    url: `https://example.com/${index + 1}`,
    description: '',
    openMethod: 2,
    itemIconGroupId: Math.floor(index / 3) + 1,
    icon: { itemType: 1, text: 'I', backgroundColor: '#18212b' },
  }))

  await page.addInitScript(() => {
    sessionStorage.setItem('yin-panel-public-access:abcdef', 'test-access')
  })
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    const groupId = Number(new URL(route.request().url()).searchParams.get('groupId'))
    let data: unknown = {}
    let delay = 0

    if (path.endsWith('/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false } }
      delay = 120
    }
    else if (path.endsWith('/getEnableStatus')) {
      data = { enabled: false }
      delay = 80
    }
    else if (path.endsWith('/spaces')) {
      data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
      delay = 160
    }
    else if (path.endsWith('/groups')) {
      data = groups
      delay = 120
    }
    else if (path.endsWith('/items')) {
      data = items.filter(item => item.itemIconGroupId === groupId)
      delay = 180
    }
    else if (path.endsWith('/search-config')) {
      data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }
    }

    if (delay) await new Promise(resolve => setTimeout(resolve, delay))
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })

  await page.addInitScript(() => {
    const target = window as typeof window & { __clsMetric?: number }
    target.__clsMetric = 0
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as (PerformanceEntry & { hadRecentInput?: boolean; value?: number })[]) {
        if (!entry.hadRecentInput) target.__clsMetric = (target.__clsMetric || 0) + (entry.value || 0)
      }
    }).observe({ type: 'layout-shift', buffered: true })
  })

  await page.goto('/abcdef', { waitUntil: 'domcontentloaded' })
  await expect(page.locator('[data-testid="item-group"]')).toHaveCount(groups.length)
  await expect.poll(() => page.evaluate(() => (window as typeof window & { __clsMetric?: number }).__clsMetric || 0), { timeout: 2000 }).toBeLessThan(0.1)
})

test('home keeps ordinary groups stable while system monitor data loads', async ({ page }) => {
  const groups = [{ id: 1, title: 'Group 1', sort: 1, parentId: null }]
  const items = [{
    id: 1,
    title: 'Item 1',
    url: 'https://example.com/1',
    description: '',
    openMethod: 2,
    itemIconGroupId: 1,
    icon: { itemType: 1, text: 'I', backgroundColor: '#18212b' },
  }]

  await page.addInitScript(() => {
    sessionStorage.setItem('yin-panel-public-access:abcdef', 'test-access')
    const target = window as typeof window & { __clsMetric?: number }
    target.__clsMetric = 0
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries() as (PerformanceEntry & { hadRecentInput?: boolean; value?: number })[]) {
        if (!entry.hadRecentInput) target.__clsMetric = (target.__clsMetric || 0) + (entry.value || 0)
      }
    }).observe({ type: 'layout-shift', buffered: true })
  })
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    const delay = path.endsWith('/getEnableStatus') || path.endsWith('/snapshot') ? 220 : 40
    let data: unknown = {}
    if (path.endsWith('/getConfig')) data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: true, systemMonitorShowTitle: true } }
    else if (path.endsWith('/getEnableStatus')) data = { enabled: true, refresh_interval: 10 }
    else if (path.endsWith('/spaces')) data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
    else if (path.endsWith('/groups')) data = groups
    else if (path.endsWith('/items')) data = items
    else if (path.endsWith('/search-config')) data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }
    else if (path.endsWith('/snapshot')) data = { cpu: 10, memory: 20, network: { upload: 1, download: 2 } }
    if (delay) await new Promise(resolve => setTimeout(resolve, delay))
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data }) })
  })

  await page.goto('/abcdef', { waitUntil: 'domcontentloaded' })
  const firstGroup = page.locator('[data-testid="item-group"]').first()
  await expect(firstGroup).toBeVisible()
  await expect(page.locator('.system-monitor-layer')).toBeVisible()
  for (const label of ['CPU', 'RAM', 'NET'])
    await expect(page.getByText(label, { exact: true }).first()).toBeVisible()
  const initialTop = await firstGroup.boundingBox()
  expect(initialTop).not.toBeNull()
  await page.waitForTimeout(700)
  const finalTop = await firstGroup.boundingBox()
  const monitorBox = await page.locator('.system-monitor-layer').boundingBox()
  expect(finalTop?.y).toBe(initialTop?.y)
  expect(monitorBox?.y! + monitorBox?.height!).toBeLessThanOrEqual(finalTop?.y!)
  await expect.poll(() => page.evaluate(() => (window as typeof window & { __clsMetric?: number }).__clsMetric || 0), { timeout: 2000 }).toBeLessThan(0.1)
})

test('home renders cached content without API requests while offline', async ({ page }) => {
  const requests: string[] = []
  await page.addInitScript(() => {
    sessionStorage.setItem('yin-panel-public-access:abcdef', 'test-access')
    if (localStorage.getItem('__offline_test__') === '1')
      Object.defineProperty(navigator, 'onLine', { configurable: true, get: () => false })
  })
  await page.route('**/api/**', async (route) => {
    requests.push(new URL(route.request().url()).pathname)
    const path = new URL(route.request().url()).pathname
    let data: unknown = {}
    if (path.endsWith('/getConfig')) data = { panel: { systemMonitorShow: false } }
    else if (path.endsWith('/getEnableStatus')) data = { enabled: false }
    else if (path.endsWith('/spaces')) data = [{ id: 1, name: 'Cached Space', type: 'personal', ownerUserId: 1 }]
    else if (path.endsWith('/groups')) data = [{ id: 1, title: 'Cached Group', sort: 1, parentId: null }]
    else if (path.endsWith('/items')) data = [{ id: 1, title: 'Cached Item', url: 'https://example.com', description: '', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'I', backgroundColor: '#18212b' } }]
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data }) })
  })

  await page.goto('/abcdef', { waitUntil: 'domcontentloaded' })
  await expect(page.getByText('Cached Item', { exact: true })).toBeVisible()
  const onlineRequestCount = requests.length
  await page.evaluate(() => localStorage.setItem('__offline_test__', '1'))
  await page.reload({ waitUntil: 'domcontentloaded' })
  await expect(page.getByText('Cached Item', { exact: true })).toBeVisible()
  expect(requests.slice(onlineRequestCount)).not.toContain('/api/spaces')
  expect(requests.slice(onlineRequestCount)).not.toContain('/api/panel/userConfig/getConfig')
  expect(requests.slice(onlineRequestCount)).not.toContain('/api/system/monitor/getEnableStatus')
})
