import { expect, test } from '@playwright/test'

// Compares the THEME-RENDERED home against the confirmed old-version ruler.
//
// The theme runtime normally needs a per-user grant, so this spec plays the
// sandbox grant back as granted and lets every other theme endpoint hit the
// real server: /api/theme/v2/current and /api/theme/v2/assets/* are public, so
// the package that renders here is exactly the one the backend serves.
//
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

const GROUPS = [
  { id: 1, title: 'Development', sort: 1, parentId: null },
  { id: 2, title: 'Tools', sort: 2, parentId: null },
  { id: 3, title: 'Documentation', sort: 3, parentId: null },
]

async function mockThemeHomeApi(page: import('@playwright/test').Page, revision: string, permissions: string[]) {
  await page.route('**/api/**', async (route) => {
    const u = new URL(route.request().url())
    const path = u.pathname

    // Let the real server serve the theme package and its assets (both public).
    if (path === '/api/theme/v2/current'
      || path.startsWith('/api/theme/v2/assets/')
      || path.startsWith('/api/theme/v2/packages')) {
      return route.fallback()
    }

    // Play the sandbox grant back as granted so the theme runtime activates.
    if (path.startsWith('/api/theme/v2/grants/')) {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ code: 0, data: { revision, executionMode: 'sandbox', available: true, granted: true, permissions } }),
      })
    }

    let data: unknown = {}
    if (path.endsWith('/system/monitor/getEnableStatus')) {
      data = { enabled: false }
    } else if (path.endsWith('/panel/userConfig/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false } }
    } else if (path === '/api/spaces') {
      data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1, side: 'yin', canEdit: true }]
    } else if (path.endsWith('/groups')) {
      data = GROUPS
    } else if (path.endsWith('/items')) {
      const raw = u.searchParams.get('groupId')
      const groupId = raw === null ? null : Number(raw)
      data = groupId === null || Number.isNaN(groupId)
        ? ITEMS
        : ITEMS.filter(item => item.itemIconGroupId === groupId)
    } else if (path.endsWith('/search-config')) {
      data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })

  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: {
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
      },
      expire: null,
    }))
    localStorage.setItem('yin-color-scheme', 'light')
    document.documentElement.setAttribute('data-color-scheme', 'light')
  })
}

// Freeze the wall clock so the live clock renders identically between runs.
// addInitScript also runs inside the theme sandbox frame.
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

interface ActiveTheme {
  revision: string
  permissions: string[]
}

// The theme runtime only activates when the grant covers every permission the
// package declares, so read the real package first and grant exactly those.
async function activeTheme(page: import('@playwright/test').Page): Promise<ActiveTheme> {
  const response = await page.request.get('/api/theme/v2/current')
  const body = await response.json()
  const revision = body?.data?.revision
  if (typeof revision !== 'string' || !revision)
    throw new Error('The active theme package has no revision; is a theme installed?')
  const required = body?.data?.manifest?.permissions?.required ?? []
  return {
    revision,
    permissions: required.map((permission: { name?: string }) => permission?.name).filter((name: unknown): name is string => typeof name === 'string'),
  }
}

const VIEWPORTS = [
  { name: 'desktop', width: 1920, height: 1080 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'mobile', width: 375, height: 667 },
]

for (const viewport of VIEWPORTS) {
  test.describe(`Theme compare: Home page - ${viewport.name} - light`, () => {
    test.use({ viewport: { width: viewport.width, height: viewport.height } })

    test.beforeEach(async ({ page }) => {
      const theme = await activeTheme(page)
      await mockThemeHomeApi(page, theme.revision, theme.permissions)
      await freezeClock(page)
    })

    test('main home page', async ({ page }) => {
      await page.goto('/', { waitUntil: 'networkidle' })
      // The theme sandbox renders the collection inside its own frame.
      await expect(page.frameLocator('iframe[title], iframe').locator('.yin-item').first()).toBeVisible({ timeout: 20000 })
      await page.waitForTimeout(1500)
      await expect(page.locator('.sun-main')).toHaveScreenshot(`home-${viewport.name}-light.png`, {
        animations: 'disabled',
      })
    })

    test('home page with search', async ({ page }) => {
      await page.goto('/', { waitUntil: 'networkidle' })
      const input = page.frameLocator('iframe[title], iframe').locator('.yin-search-input')
      await expect(input.first()).toBeVisible({ timeout: 20000 })
      await input.first().fill('test')
      await page.waitForTimeout(1000)
      await expect(page.locator('.sun-main')).toHaveScreenshot(`home-search-${viewport.name}-light.png`, {
        animations: 'disabled',
      })
    })
  })
}
