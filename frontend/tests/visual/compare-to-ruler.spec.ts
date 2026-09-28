import { expect, test } from '@playwright/test'

// Compares the current build against the confirmed old-version ruler.
// Scenario names and viewports must stay in lockstep with ruler-capture.spec.ts,
// otherwise the snapshot lookup below will not find the ruler image.

const ITEMS = [
  { id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', description: 'Code Editor', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } },
  { id: 2, title: 'GitHub', url: 'https://github.com/', description: 'Code Hosting', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'G', backgroundColor: '#181717' } },
  { id: 3, title: 'Docker Hub', url: 'https://hub.docker.com/', description: 'Container Registry', openMethod: 2, itemIconGroupId: 2, icon: { itemType: 1, text: 'D', backgroundColor: '#0db7ed' } },
  { id: 4, title: 'Postman', url: 'https://www.postman.com/', description: 'API Testing', openMethod: 2, itemIconGroupId: 2, icon: { itemType: 1, text: 'P', backgroundColor: '#ff6c37' } },
  { id: 5, title: 'MDN Web Docs', url: 'https://developer.mozilla.org/', description: 'Web Documentation', openMethod: 2, itemIconGroupId: 3, icon: { itemType: 1, text: 'M', backgroundColor: '#000000' } },
  { id: 6, title: 'Vue.js Guide', url: 'https://vuejs.org/guide/', description: 'Vue Documentation', openMethod: 2, itemIconGroupId: 3, icon: { itemType: 1, text: 'V', backgroundColor: '#42b883' } },
]

async function mockCurrentApi(page: import('@playwright/test').Page) {
  await page.route('**/api/**', async (route) => {
    const u = new URL(route.request().url())
    const path = u.pathname
    let data: unknown = {}

    if (path.endsWith('/system/monitor/getEnableStatus')) {
      data = { enabled: false }
    } else if (path.endsWith('/panel/userConfig/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false } }
    } else if (path === '/api/spaces') {
      data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
    } else if (path.endsWith('/groups')) {
      data = [
        { id: 1, title: 'Development', sort: 1, parentId: null },
        { id: 2, title: 'Tools', sort: 2, parentId: null },
        { id: 3, title: 'Documentation', sort: 3, parentId: null },
      ]
    } else if (path.endsWith('/items')) {
      // The current build fetches every item (no groupId) for its search index
      // and filters per group otherwise. Honour both shapes so the search state
      // is comparable with the ruler.
      const raw = u.searchParams.get('groupId')
      const groupId = raw === null ? null : Number(raw)
      data = groupId === null || Number.isNaN(groupId)
        ? ITEMS
        : ITEMS.filter(item => item.itemIconGroupId === groupId)
    } else if (path.endsWith('/search-config')) {
      data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }
    } else {
      data = {}
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })

  await page.addInitScript((scheme) => {
    localStorage.setItem('authStorage', JSON.stringify({
      token: 'test-token-123',
      userInfo: {
        id: 1,
        name: 'Test User',
        mail: 'test@example.com',
        avatar: '',
        role: 1,
        createTime: Date.now(),
        updateTime: Date.now(),
      },
    }))
    localStorage.setItem('yin-color-scheme', scheme)
    document.documentElement.setAttribute('data-color-scheme', scheme)
  }, 'light')
}

// Freeze the wall clock so the live clock widget renders identically between
// runs. Freezing is preferable to masking: a mask hides the region in every
// image and makes position drift invisible.
async function freezeClock(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    const fixed = new Date('2026-01-01T12:00:00').getTime()
    const RealDate = Date
    // eslint-disable-next-line ts/no-explicit-any
    const MockDate: any = function (this: unknown, ...args: unknown[]) {
      if (args.length === 0) return new RealDate(fixed)
      // @ts-expect-error forwarded constructor
      return new RealDate(...args)
    }
    MockDate.now = () => fixed
    MockDate.parse = RealDate.parse
    MockDate.UTC = RealDate.UTC
    MockDate.prototype = RealDate.prototype
    // eslint-disable-next-line ts/no-explicit-any
    ;(window as any).Date = MockDate
  })
}

const VIEWPORTS = [
  { name: 'desktop', width: 1920, height: 1080 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'mobile', width: 375, height: 667 },
]

for (const viewport of VIEWPORTS) {
  test.describe(`Compare: Home page - ${viewport.name} - light`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height } })

    test.beforeEach(async ({ page }) => {
      await mockCurrentApi(page)
      await freezeClock(page)
    })

    test('main home page', async ({ page }) => {
      await page.goto('/', { waitUntil: 'networkidle' })
      await expect(page.locator('[data-testid="item-group"]').first()).toBeVisible({ timeout: 15000 })
      await page.waitForTimeout(1500)
      await expect(page.locator('.sun-main')).toHaveScreenshot(`home-${viewport.name}-light.png`, {
        animations: 'disabled',
      })
    })

    test('home page with search', async ({ page }) => {
      await page.goto('/', { waitUntil: 'networkidle' })
      await expect(page.locator('[data-testid="home-search-input"]').first()).toBeVisible({ timeout: 15000 })
      await page.locator('[data-testid="home-search-input"]').first().fill('test')
      await page.waitForTimeout(1000)
      await expect(page.locator('.sun-main')).toHaveScreenshot(`home-search-${viewport.name}-light.png`, {
        animations: 'disabled',
      })
    })
  })
}

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
