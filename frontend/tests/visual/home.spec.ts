import { expect, test } from '@playwright/test'

// ============ Helper Functions (defined first) ============

async function mockApi(page: import('@playwright/test').Page) {
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
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
        { id: 3, title: 'Documentation', sort: 3, parentId: null },
      ]
    } else if (path.endsWith('/items')) {
      const groupId = Number(new URL(route.request().url()).searchParams.get('groupId'))
      const allItems = [
        { id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', description: 'Code Editor', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } },
        { id: 2, title: 'GitHub', url: 'https://github.com/', description: 'Code Hosting', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'G', backgroundColor: '#181717' } },
        { id: 3, title: 'Docker Hub', url: 'https://hub.docker.com/', description: 'Container Registry', openMethod: 2, itemIconGroupId: 2, icon: { itemType: 1, text: 'D', backgroundColor: '#0db7ed' } },
        { id: 4, title: 'Postman', url: 'https://www.postman.com/', description: 'API Testing', openMethod: 2, itemIconGroupId: 2, icon: { itemType: 1, text: 'P', backgroundColor: '#ff6c37' } },
        { id: 5, title: 'MDN Web Docs', url: 'https://developer.mozilla.org/', description: 'Web Documentation', openMethod: 2, itemIconGroupId: 3, icon: { itemType: 1, text: 'M', backgroundColor: '#000000' } },
        { id: 6, title: 'Vue.js Guide', url: 'https://vuejs.org/guide/', description: 'Vue Documentation', openMethod: 2, itemIconGroupId: 3, icon: { itemType: 1, text: 'V', backgroundColor: '#42b883' } },
      ]
      data = allItems.filter(item => item.itemIconGroupId === groupId)
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

async function setColorScheme(page: import('@playwright/test').Page, scheme: 'light' | 'dark') {
  await page.addInitScript((scheme) => {
    document.documentElement.setAttribute('data-color-scheme', scheme)
    localStorage.setItem('yin-color-scheme', scheme)
  }, scheme)
}

const VIEWPORTS = [
  { name: 'desktop', width: 1920, height: 1080 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'mobile', width: 375, height: 667 },
]

const COLOR_SCHEMES = ['light', 'dark']

for (const viewport of VIEWPORTS) {
  for (const scheme of COLOR_SCHEMES) {
    test.describe(`Home page - ${viewport.name} - ${scheme}`, () => {
      test.use({ viewport: { width: viewport.width, height: viewport.height } })

      test.beforeEach(async ({ page }) => {
        await mockApi(page)
        await setColorScheme(page, scheme)
      })

      test('main home page', async ({ page }) => {
        await page.goto('/', { waitUntil: 'domcontentloaded' })
        await expect(page.locator('.sun-main')).toBeVisible()
        await page.waitForTimeout(500)
        await expect(page.locator('.sun-main')).toHaveScreenshot(`home-${viewport.name}-${scheme}.png`, {
          animations: 'disabled',
        })
      })

      test('home page with search', async ({ page }) => {
        await page.goto('/', { waitUntil: 'domcontentloaded' })
        await expect(page.locator('[data-testid="home-search-input"]').first()).toBeVisible()
        await page.locator('[data-testid="home-search-input"]').first().fill('test')
        await page.waitForTimeout(300)
        await expect(page.locator('.sun-main')).toHaveScreenshot(`home-search-${viewport.name}-${scheme}.png`, {
          animations: 'disabled',
        })
      })
    })
  }
}

for (const viewport of VIEWPORTS) {
  test.describe(`Login page - ${viewport.name}`, () => {
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
      await page.goto('/login', { waitUntil: 'domcontentloaded' })
      await expect(page.locator('.login-container')).toBeVisible()
      await page.waitForTimeout(300)
      await expect(page.locator('.login-container')).toHaveScreenshot(`login-${viewport.name}.png`, {
        animations: 'disabled',
      })
    })
  })
}
