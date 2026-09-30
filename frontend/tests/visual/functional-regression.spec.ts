import { expect, test } from '@playwright/test'

/**
 * Stubs the Core's data APIs. By default the theme package endpoints are answered
 * with an empty package as well, which leaves the home to the Core's own fallback
 * view — that is the state these tests exercise, because the Core no longer ships
 * a second home implementation. Pass `{ theme: 'real' }` to let the shipped theme
 * load instead and drive the production path.
 */
async function mockApi(page: import('@playwright/test').Page, options: { theme?: 'stub' | 'real' } = {}) {
  const theme = options.theme ?? 'stub'
  // Driving the production path needs the theme runtime to hold read scopes, and a
  // grant is only fetched for a signed-in session, so that case replays the sandbox
  // grant exactly as the pixel-comparison suite does.
  let revision = ''
  let permissions: string[] = []
  if (theme === 'real') {
    const current = await (await page.request.get('/api/theme/v2/current')).json()
    revision = current.data.revision
    permissions = (current.data.manifest.permissions?.required || []).map((item: { name: string }) => item.name)
  }

  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    const method = route.request().method()
    let data: unknown = {}

    if (path.startsWith('/api/theme/')) {
      if (theme !== 'real') {
        // Answered with an empty package below, leaving the home to the fallback.
      }
      else if (path.startsWith('/api/theme/v2/grants/')) {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ code: 0, data: { revision, executionMode: 'sandbox', available: true, granted: true, permissions } }),
        })
        return
      }
      else {
        await route.fallback()
        return
      }
    }

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
    // The project's storage wrapper persists `{ data, expire }`; a bare object left
    // the store's token undefined, which silently skipped the grant fetch and left
    // the theme with no read scopes.
    try {
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
            updateTime: Date.now()
          }
        },
        expire: null
      }))
    }
    catch {
      // addInitScript also runs inside the sandbox frame, which has no storage.
    }
  })
}

test.describe('Functional Regression: Fallback home view', () => {
  // With no theme home available the Core's own fallback view owns the page. This
  // is the state the previous "Core Home Features" suite described, retargeted now
  // that the Core's richer home implementation has been removed: the fallback is
  // the only Core-rendered home left.
  test.beforeEach(async ({ page }) => {
    await mockApi(page)
  })

  test('Fallback renders every group and item from the snapshot', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    await expect(page.locator('[data-testid="home-fallback"]')).toBeVisible()
    await expect(page.locator('[data-testid="home-fallback-group"]')).toHaveCount(2)

    const groups = page.locator('[data-testid="home-fallback-group"]')
    await expect(groups.nth(0).locator('[data-testid="home-fallback-item"]')).toHaveCount(2)
    await expect(groups.nth(1).locator('[data-testid="home-fallback-item"]')).toHaveCount(1)
  })

  test('Fallback item click opens the resolved bookmark URL', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('[data-testid="home-fallback-item"]').first()).toBeVisible()

    // Every mocked item uses openMethod 2 (new tab), so the Core must hand the
    // resolved URL to the browser rather than doing anything theme-specific.
    const popupPromise = page.waitForEvent('popup')
    await page.locator('[data-testid="home-fallback-item"]').first().click()
    const popup = await popupPromise
    expect(popup.url()).toContain('code.visualstudio.com')
  })
})

test.describe('Functional Regression: Theme home (production path)', () => {
  // The shipped theme is what renders the home in production, so its behaviour is
  // covered here rather than only by the pixel comparison.
  test.beforeEach(async ({ page }) => {
    await mockApi(page, { theme: 'real' })
  })

  test('Theme renders the snapshot groups and items', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator('[data-testid="theme-home-frame"]')
    await expect(theme.locator('section.yin-group')).toHaveCount(2, { timeout: 30_000 })
    // The group count resolving does not mean the item nodes have painted; the
    // frame reports them in a later pass. Without the explicit timeout these fall
    // back to the 5s default and flake under parallel load.
    await expect(theme.locator('section.yin-group').nth(0).locator('.yin-item')).toHaveCount(2, { timeout: 30_000 })
    await expect(theme.locator('section.yin-group').nth(1).locator('.yin-item')).toHaveCount(1, { timeout: 30_000 })
  })

  test('Theme search filters the rendered collection', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator('[data-testid="theme-home-frame"]')
    await expect(theme.locator('.yin-item')).toHaveCount(3, { timeout: 30_000 })

    await theme.locator('.yin-search-input').fill('vs')
    await expect(theme.locator('.yin-item')).toHaveCount(1)
    await expect(theme.locator('.yin-item-title')).toHaveText('VS Code')
  })

  test('Wallpaper layer still renders behind the theme', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    await expect(page.locator('[data-testid="wallpaper-layer"]').first()).toBeVisible()
  })

  test('Core utility entries are reachable without hovering the action bar', async ({ page }) => {
    // P4a deleted the Core copy of the refresh / back-to-top / LAN-WAN /
    // settings chrome. Those entries are Core chrome again (see
    // tests/visual/floating-actions.spec.ts for why they cannot be theme-drawn),
    // and the theme's own remaining bar must not hide the last of them.
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator('[data-testid="theme-home-frame"]')
    const bar = theme.locator('.yin-actions')
    await expect(bar).toBeAttached({ timeout: 30_000 })
    // The two entries the theme keeps are Core surfaces, so they need the command
    // permission rather than a Space capability.
    await expect(theme.locator('[data-testid="theme-open-command-center"]')).toBeAttached()
    await expect(theme.locator('[data-testid="theme-open-style"]')).toBeAttached()
  })

  test('typing in the theme still opens the command centre', async ({ page }) => {
    // The home is rendered inside a cross-origin frame, so a keystroke made there
    // never reaches the Core's own window listener. Without the theme forwarding
    // the key, the global shortcut is dead for every theme-rendered home.
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator('[data-testid="theme-home-frame"]')
    await expect(theme.locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })

    await theme.locator('.yin-page').click({ position: { x: 5, y: 5 } })
    await page.keyboard.press('a')

    await expect(page.locator('[data-testid="command-center-panel"]')).toBeVisible({ timeout: 10_000 })
    // The keystroke seeds the query, which is the behaviour the shortcut had
    // before the home moved into a frame.
    await expect(page.locator('[data-testid="command-center-input"]')).toHaveValue('a')
  })
})

test.describe('Functional Regression: Layout Stability', () => {
  // Each test installs the mock it needs: a describe-level stub would answer the
  // theme endpoints before the theme-path test's own routes see them.
  test('No layout shift during load (CLS < 0.1)', async ({ page }) => {
    await mockApi(page)

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

  test('System monitor does not occlude the first theme group', async ({ page }) => {
    // The monitor layer is Core-owned and overlays the theme frame; the theme
    // reserves room for it from the height the Core reports. The invariant that
    // matters is that the band never covers content, which is what the old
    // Core-home version of this test checked before that home was removed.
    await mockApi(page, { theme: 'real' })

    // Registered after mockApi, so these handlers run first and fall through for
    // everything they do not own — including the theme endpoints and the grant.
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname
      const slow = path.endsWith('/getEnableStatus') || path.endsWith('/snapshot')
      if (slow) await new Promise(resolve => setTimeout(resolve, 220))
      if (path.endsWith('/getConfig')) {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data: { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: true, systemMonitorShowTitle: true } } }) })
        return
      }
      if (path.endsWith('/getEnableStatus')) {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data: { enabled: true, refresh_interval: 10 } }) })
        return
      }
      if (path.endsWith('/snapshot')) {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data: { cpu: 10, memory: 20, network: { upload: 1, download: 2 } } }) })
        return
      }
      await route.fallback()
    })

    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator('[data-testid="theme-home-frame"]')
    const firstGroup = theme.locator('section.yin-group').first()
    await expect(firstGroup).toBeVisible({ timeout: 30_000 })
    await page.waitForTimeout(800)

    const monitorLayer = page.locator('[data-testid="theme-runtime-monitor"]').first()
    if (await monitorLayer.isVisible().catch(() => false)) {
      const groupBox = await firstGroup.boundingBox()
      const monitorBox = await monitorLayer.boundingBox()
      if (groupBox && monitorBox)
        expect(monitorBox.y + monitorBox.height).toBeLessThanOrEqual(groupBox.y)
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
    await expect.poll(() => page.evaluate(() => (window as typeof window & { __lcpMetric?: { time: number, marker?: string } }).__lcpMetric), { timeout: 5000 }).toBeTruthy()

    const lcp = await page.evaluate(() => (window as typeof window & { __lcpMetric?: { time: number, marker?: string } }).__lcpMetric)
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

test.describe('Functional Regression: Request layer logging', () => {
  // The message api is mounted as a separate app, so a refused request can fail to
  // produce any visible toast for reasons the request layer cannot see. The console
  // is the channel that cannot go missing, and it is what a developer has when a
  // request is refused and nothing appears on screen.
  test('a refused request is logged with its code', async ({ page }) => {
    await mockApi(page)
    const logs: string[] = []
    page.on('console', msg => logs.push(`${msg.type()} ${msg.text()}`))

    await page.route('**/api/spaces**', route => route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 1400, msg: 'bad shape', data: [] }),
    }))

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await expect.poll(() => logs.some(l => l.includes('[api]') && l.includes('/spaces') && l.includes('1400')))
      .toBe(true)
    // A real fault is an error, not a warning, so it stands out from the expected
    // refusals (1000/1001/1005) the layer handles on its own.
    expect(logs.filter(l => l.startsWith('error') && l.includes('[api]')).length).toBeGreaterThan(0)

    // And the reference: a refusal the layer treats as expected stays a warning.
    logs.length = 0
    await page.unroute('**/api/spaces**')
    await page.route('**/api/spaces**', route => route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 1005, msg: '', data: [] }),
    }))
    await page.reload({ waitUntil: 'domcontentloaded' })
    await expect.poll(() => logs.some(l => l.includes('[api]') && l.includes('1005'))).toBe(true)
    expect(logs.filter(l => l.startsWith('warning') && l.includes('[api]')).length).toBeGreaterThan(0)
    expect(logs.filter(l => l.startsWith('error') && l.includes('1005'))).toHaveLength(0)
  })
})

test.describe('Functional Regression: Public link theme rendering', () => {
  // A public link has no user, so it can never hold a theme grant record. The
  // Core must still hand the theme the read scopes the link already exposes —
  // otherwise `scopedSnapshot` blanks every collection and visitors get an empty
  // home. This case exists because an earlier revision of that change was
  // verified only by "an iframe appeared", which hid exactly that failure.
  test.beforeEach(async ({ page }) => {
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname
      // Let the real theme package and asset endpoints through so the shipped
      // theme is exercised rather than a stub.
      if (path.startsWith('/api/theme/')) {
        await route.fallback()
        return
      }
      let data: unknown = {}
      if (path.endsWith('/getEnableStatus'))
        data = { enabled: false }
      else if (path.endsWith('/getConfig'))
        data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false } }
      else if (path.endsWith('/spaces'))
        data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1, side: 'yin', canEdit: false, publicEnabled: true, publicId: 'test12', publicMode: 'direct' }]
      else if (path.endsWith('/groups'))
        data = [{ id: 1, title: 'Development', sort: 1, parentId: null }]
      else if (path.endsWith('/items'))
        data = [{ id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', description: 'Code Editor', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } }]
      else if (path.endsWith('/search-config'))
        data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }

      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ code: 0, data }),
      })
    })
  })

  test('public link renders real items inside the theme sandbox', async ({ page }) => {
    await page.goto('/test12', { waitUntil: 'networkidle' })

    const theme = page.frameLocator('[data-testid="theme-home-frame"]')
    await expect(theme.locator('.yin-item').first()).toBeVisible({ timeout: 30000 })
    expect(await theme.locator('.yin-item').count()).toBeGreaterThan(0)
    await expect(theme.locator('.yin-item-title').first()).toHaveText('VS Code')

    // Read-only: the link exposes reads, never writes. The theme hides these by
    // setting `hidden`, so they stay in the DOM and must be asserted as not
    // visible rather than absent.
    await expect(theme.locator('[data-testid="theme-add-group"]')).toBeHidden()
    await expect(theme.locator('[data-testid="theme-add-item"]').first()).toBeHidden()
    // The command center needs only items.read, which this link does hold, but the
    // Core refuses to open it without a session — so it must be hidden rather than
    // offered as a button that silently does nothing.
    await expect(theme.locator('[data-testid="theme-open-command-center"]')).toBeHidden()
    await expect(theme.locator('[data-testid="theme-open-settings"]')).toBeHidden()
    // The Core's own collection must not render alongside the theme.
    await expect(page.locator('[data-testid="item-group"]')).toHaveCount(0)
  })
})

test.describe('Functional Regression: Fallback home view (safe mode)', () => {
  // Boundary C3. Once P4a removes the Core's own presentation, "the theme cannot
  // render the home" must still produce a usable page rather than a blank one.
  // Safe mode is the supported way to force that state.
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      sessionStorage.setItem('yin-theme-safe-mode', '1')
    })
    await page.route('**/api/**', async (route) => {
      const path = new URL(route.request().url()).pathname
      if (path.startsWith('/api/theme/')) {
        await route.fallback()
        return
      }
      let data: unknown = {}
      if (path.endsWith('/getEnableStatus'))
        data = { enabled: false }
      else if (path.endsWith('/getConfig'))
        data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false, logoText: 'My Panel' } }
      else if (path.endsWith('/spaces'))
        data = [{ id: 1, name: 'Space One', type: 'personal', ownerUserId: 1, side: 'yin', canEdit: true }]
      else if (path.endsWith('/groups'))
        data = [{ id: 1, title: 'Development', sort: 1, parentId: null }]
      else if (path.endsWith('/items'))
        data = [{ id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', description: 'Code Editor', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } }]
      else if (path.endsWith('/search-config'))
        data = { currentSearchEngine: { iconSrc: '/assets/search_engine_svg/bing.svg', title: 'Bing', url: 'https://www.bing.com/search?q=%s' } }

      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ code: 0, data }),
      })
    })
  })

  test('safe mode renders the fallback list instead of a blank page', async ({ page }) => {
    await page.goto('/', { waitUntil: 'networkidle' })

    const fallback = page.locator('[data-testid="home-fallback"]')
    await expect(fallback).toBeVisible({ timeout: 30000 })
    // The point of the fallback: the user can still see and reach their bookmarks.
    await expect(page.locator('[data-testid="home-fallback-item"]').first()).toHaveText(/VS Code/)
    await expect(page.locator('[data-testid="home-fallback-group"]').first()).toContainText('Development')
    // It must say why it is showing instead of the theme.
    await expect(page.locator('[data-testid="home-fallback-notice"]')).toBeVisible()
    // And offer the way out — the in-app route, not a static page: the fallback is
    // a Vue component, so the router is already running and has the full recovery
    // surface. The static page it used to point at was a subset of this route and
    // was answered by the worker's navigation fallback anyway.
    await expect(page.locator('[data-testid="home-fallback-recovery"]')).toHaveAttribute('href', '/__yin/theme-recovery')
    // Exactly one full view is mounted.
    await expect(page.locator('[data-testid="theme-home-frame"]')).toHaveCount(0)
    await expect(page.locator('.home-scroll-container')).toHaveCount(0)
  })
})