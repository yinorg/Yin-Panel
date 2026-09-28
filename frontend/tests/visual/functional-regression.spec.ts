import { expect, test } from '@playwright/test'

async function mockApi(page: import('@playwright/test').Page) {
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    const method = route.request().method()
    let data: unknown = {}

    if (path.endsWith('/oauth/config')) {
      data = { enabled: false, providers: [] }
    } else if (path.endsWith('/spaces')) {
      data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
    } else if (path.endsWith('/getEnableStatus')) {
      data = { enabled: false }
    } else if (path.endsWith('/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false } }
    } else if (path.endsWith('/groups')) {
      data = [
        { id: 1, title: 'Development', sort: 1, parentId: null },
        { id: 2, title: 'Tools', sort: 2, parentId: null },
      ]
    } else if (path.endsWith('/items')) {
      const groupId = Number(new URL(route.request().url()).searchParams.get('groupId'))
      const allItems = [
        { id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', description: 'Code Editor', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } },
        { id: 2, title: 'GitHub', url: 'https://github.com/', description: 'Code Hosting', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'G', backgroundColor: '#181717' } },
        { id: 3, title: 'Docker Hub', url: 'https://hub.docker.com/', description: 'Container Registry', openMethod: 2, itemIconGroupId: 2, icon: { itemType: 1, text: 'D', backgroundColor: '#0db7ed' } },
      ]
      data = allItems.filter(item => item.itemIconGroupId === groupId)
    } else if (path.endsWith('/search-config')) {
      data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }
    } else if (path.endsWith('/login') && method === 'POST') {
      data = {
        token: 'test-token-123',
        name: 'Test User',
        mail: 'test@example.com',
        id: 1,
        avatar: '',
        role: 1,
        createTime: Date.now(),
        updateTime: Date.now()
      }
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })

  await page.addInitScript(() => {
    const authStorage = {
      token: 'test-token-123',
      userInfo: {
        id: 1,
        name: 'Test User',
        mail: 'test@example.com',
        avatar: '',
        role: 1,
        createTime: Date.now(),
        updateTime: Date.now()
      }
    }
    localStorage.setItem('authStorage', JSON.stringify(authStorage))
  })
}

test.describe('Functional Regression: Core Home Features', () => {
  test.beforeEach(async ({ page }) => {
    await mockApi(page)
  })

  test('Home page loads and displays groups/items', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    await expect(page.locator('[data-testid="item-group"]')).toHaveCount(2)

    const group1 = page.locator('[data-testid="item-group"]').first()
    await expect(group1.locator('[data-testid="home-item"]')).toHaveCount(2)

    const group2 = page.locator('[data-testid="item-group"]').nth(1)
    await expect(group2.locator('[data-testid="home-item"]')).toHaveCount(1)
  })

  test('Search input is visible and interactive', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const searchInput = page.locator('[data-testid="home-search-input"]').first()
    await expect(searchInput).toBeVisible()

    await searchInput.fill('test')
    await page.waitForTimeout(300)

    await expect(searchInput).toHaveValue('test')
  })

  test('Wallpaper layer renders', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    await expect(page.locator('[data-testid="wallpaper-layer"]').first()).toBeVisible()
  })

  test('Item click triggers handler', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const firstItem = page.locator('[data-testid="home-item"]').first()
    await firstItem.click()
    await page.waitForTimeout(200)
  })
})

test.describe('Functional Regression: Layout Stability', () => {
  test.beforeEach(async ({ page }) => {
    await mockApi(page)
  })

  test('No layout shift during load (CLS < 0.1)', async ({ page }) => {
    await page.addInitScript(() => {
      const target = window as typeof window & { __clsMetric?: number }
      target.__clsMetric = 0
      new PerformanceObserver((list) => {
        for (const entry of list.getEntries() as (PerformanceEntry & { hadRecentInput?: boolean; value?: number })[]) {
          if (!entry.hadRecentInput) target.__clsMetric = (target.__clsMetric || 0) + (entry.value || 0)
        }
      }).observe({ type: 'layout-shift', buffered: true })
    })

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(2000)

    const cls = await page.evaluate(() => (window as typeof window & { __clsMetric?: number }).__clsMetric || 0)
    expect(cls).toBeLessThan(0.1)
  })

  test('Groups remain stable while system monitor loads', async ({ page }) => {
    const groups = [{ id: 1, title: 'Group 1', sort: 1, parentId: null }]
    const items = [{ id: 1, title: 'Item 1', url: 'https://example.com/1', description: '', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'I', backgroundColor: '#18212b' } }]

    await page.addInitScript(() => {
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

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    const firstGroup = page.locator('[data-testid="item-group"]').first()
    await expect(firstGroup).toBeVisible()

    const initialBox = await firstGroup.boundingBox()
    expect(initialBox).not.toBeNull()

    await page.waitForTimeout(700)

    const finalBox = await firstGroup.boundingBox()
    expect(finalBox).not.toBeNull()

    expect(finalBox?.y).toBe(initialBox?.y)

    const cls = await page.evaluate(() => (window as typeof window & { __clsMetric?: number }).__clsMetric || 0)
    expect(cls).toBeLessThan(0.1)
  })

  test('No content occlusion by monitor', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const firstGroup = page.locator('[data-testid="item-group"]').first()
    const monitorLayer = page.locator('.system-monitor-layer, .theme-runtime-monitor-layer').first()

    if (await monitorLayer.isVisible().catch(() => false)) {
      const groupBox = await firstGroup.boundingBox()
      const monitorBox = await monitorLayer.boundingBox()

      if (groupBox && monitorBox) {
        expect(monitorBox.y + monitorBox.height).toBeLessThanOrEqual(groupBox.y)
      }
    }
  })
})

test.describe('Functional Regression: Performance', () => {
  test.beforeEach(async ({ page }) => {
    await mockApi(page)
  })

  test('LCP within budget (< 2500ms)', async ({ page }) => {
    await page.addInitScript(() => {
      const target = window as typeof window & { __lcpMetric?: { time: number; marker?: string; text?: string } }
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

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('[data-lcp="brand"]').last()).toBeVisible()
    await expect.poll(() => page.evaluate(() => (window as typeof window & { __lcpMetric?: { time: number } }).__lcpMetric), { timeout: 5000 }).toBeTruthy()

    const lcp = await page.evaluate(() => (window as typeof window & { __lcpMetric?: { time: number } }).__lcpMetric)
    expect(lcp?.time).toBeLessThan(2500)
    expect(lcp?.marker).toBe('brand')
  })

  test('Home page loads without console errors', async ({ page }) => {
    const errors: string[] = []
    page.on('console', msg => {
      if (msg.type() === 'error') {
        errors.push(msg.text())
      }
    })
    page.on('pageerror', error => {
      errors.push(error.message)
    })

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await page.waitForTimeout(2000)

    const criticalErrors = errors.filter(e =>
      !e.includes('favicon') &&
      !e.includes('Failed to load resource') &&
      !e.includes('Extension') &&
      !e.includes('content-script')
    )

    expect(criticalErrors).toHaveLength(0)
  })
})