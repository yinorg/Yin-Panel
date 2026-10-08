import type { Page } from '@playwright/test'

/**
 * Stubs the Core's data APIs so a test can drive the real home without a server-side
 * session.
 *
 * By default the theme package endpoints are answered with an empty package, which
 * leaves the home to the Core's own fallback view. Pass `{ theme: 'real' }` to let
 * the shipped theme load instead, which is the production path: the home renders
 * inside the theme frame and every tap crosses that boundary.
 *
 * Shared by `tests/visual/` and `tests/e2e/` on purpose. The item fixture in
 * particular is load-bearing for the popup tests, which assert against a specific
 * item's URL and open method; two copies of this list would drift and quietly stop
 * matching the tests that depend on them.
 */
export async function mockApi(page: Page, options: { theme?: 'stub' | 'real' } = {}) {
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
      // `itemIds` must list the group's items. The theme's context menu resolves an
      // item's group through this list, and without it the menu silently renders
      // nothing — which reads as "right-click does nothing" rather than "bad fixture".
      data = [
        { id: 1, title: 'Development', sort: 1, parentId: null, itemIds: [1, 2] },
        { id: 2, title: 'Tools', sort: 2, parentId: null, itemIds: [3] },
      ]
    } else if (path.endsWith('/items')) {
      const groupId = Number(new URL(route.request().url()).searchParams.get('groupId'))
      data = panelItems.filter(item => item.itemIconGroupId === groupId)
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
        updateTime: Date.now(),
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
            updateTime: Date.now(),
          },
        },
        expire: null,
      }))
    }
    catch {
      // addInitScript also runs inside the sandbox frame, which has no storage.
    }
  })
}

export interface PanelItemFixture {
  id: number
  title: string
  url: string
  description: string
  /** 1 = this tab, 2 = a new window, 3 = the in-app window. */
  openMethod: number
  itemIconGroupId: number
  icon: { itemType: number, text?: string, backgroundColor?: string, src?: string }
}

/**
 * The item fixture every suite shares.
 *
 * `VS Code` is deliberately `openMethod: 2`, because that is the case the popup
 * tests assert on and it is the one mobile browsers refuse. `Panasonic` is
 * `openMethod: 3` so the in-app window path stays covered by something that is not
 * about popups at all.
 */
export const panelItems: PanelItemFixture[] = [
  { id: 1, title: 'VS Code', url: 'https://code.visualstudio.com/', description: 'Code Editor', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'V', backgroundColor: '#007acc' } },
  { id: 2, title: 'GitHub', url: 'https://github.com/', description: 'Code Hosting', openMethod: 2, itemIconGroupId: 1, icon: { itemType: 1, text: 'G', backgroundColor: '#181717' } },
  { id: 3, title: 'Docker Hub', url: 'https://hub.docker.com/', description: 'Container Registry', openMethod: 2, itemIconGroupId: 2, icon: { itemType: 1, text: 'D', backgroundColor: '#0db7ed' } },
]

/** The fixture item the popup tests target: `openMethod: 2`, first in group 1. */
export const popupFixtureItem = panelItems[0]