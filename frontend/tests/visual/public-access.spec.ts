import { expect, test, type Page } from '@playwright/test'

/**
 * Core chrome on a public link.
 *
 * A public link is opened by a visitor who may also be signed in, so the auth
 * token can still be in localStorage. Nothing about the request layer clears it —
 * a `publiccode` path only makes the interceptor skip the Authorization header —
 * so every chrome decision has to test the public code explicitly instead of
 * treating a present token as "this is a normal visit".
 *
 * The three behaviours below are the ones that were wrong:
 *   - the space selector rendered, and a public link has exactly one space;
 *   - the utility stack was hidden, but only the operator's settings button
 *     should be;
 *   - the keyboard shortcut was refused, so the palette could not be opened.
 */

const PUBLIC_CODE = 'abcdef'

async function preparePublicLink(page: Page) {
  await page.addInitScript((code) => {
    // A visitor who is signed in while opening the public link.
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
    sessionStorage.setItem(`yin-panel-public-access:${code}`, 'test-access')
  }, PUBLIC_CODE)

  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    let data: unknown = {}

    if (path.endsWith('/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false, netModeChangeButtonShow: true } }
    }
    else if (path.endsWith('/getEnableStatus')) {
      data = { enabled: false }
    }
    else if (path.endsWith('/spaces')) {
      data = [{ id: 1, name: 'Public Space', type: 'personal', ownerUserId: 1, publicMode: 'direct' }]
    }
    else if (path.endsWith('/groups') || path.endsWith('/items')) {
      data = []
    }

    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data }) })
  })
}

test('a public link has no space selector and no system settings button', async ({ page }) => {
  await preparePublicLink(page)
  await page.goto(`/${PUBLIC_CODE}`, { waitUntil: 'domcontentloaded' })

  // The selector is what proves the mismatch: a public link names one space, so
  // offering a switcher would promise navigation the visitor cannot have.
  await expect(page.getByTestId('home-floating-bar')).toBeVisible({ timeout: 30_000 })
  await expect(page.locator('.space-status-bar')).toHaveCount(0)

  const buttons = page.locator('.home-floating-button')
  await expect(buttons).toHaveCount(3)
  await expect(page.getByTestId('floating-refresh-button')).toBeVisible()
  await expect(page.getByTestId('floating-top-button')).toBeVisible()
  // The network switch is whichever of the two the current mode offers.
  await expect(page.locator('[data-testid="floating-wan-button"], [data-testid="floating-lan-button"]')).toHaveCount(1)
  // System settings is the operator surface, and it must not be reachable.
  await expect(page.getByTestId('system-settings-button')).toHaveCount(0)
})

test('a public link does not probe the monitor it can never render', async ({ page }) => {
  // The monitor band is JWT-only: both its enabling flag and its data endpoint
  // refuse a public code with 1005, so the band cannot render on a public link.
  // Probing anyway earned that 1005 on every open, and the request layer answered
  // it with a warning the visitor could do nothing about. This asserts the probe
  // is gone, which is the cause; the toast it produced is not observable here —
  // the request layer's message api does not mount a container in this build, so
  // counting `.n-message` would pass whether or not the guard held.
  const monitorProbes: string[] = []
  page.on('request', (request) => {
    if (request.url().includes('/system/monitor/')) monitorProbes.push(request.url())
  })

  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
    sessionStorage.setItem('yin-panel-public-access:abcdef', 'test-access')
  })
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    let data: unknown = {}
    let code = 0

    if (path.endsWith('/getConfig')) {
      data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: true, netModeChangeButtonShow: true } }
    }
    else if (path.endsWith('/getEnableStatus')) {
      // The real server refuses this for a public code; reproduce that refusal so
      // a regression is visible as a probe that should not have happened.
      code = 1005
    }
    else if (path.endsWith('/spaces')) {
      data = [{ id: 1, name: 'Public Space', type: 'personal', ownerUserId: 1, publicMode: 'direct' }]
    }
    else if (path.endsWith('/groups') || path.endsWith('/items')) {
      data = []
    }

    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code, msg: '', data }) })
  })

  await page.goto(`/${PUBLIC_CODE}`, { waitUntil: 'domcontentloaded' })
  await expect(page.getByTestId('home-floating-bar')).toBeVisible({ timeout: 30_000 })

  expect(monitorProbes, 'a public link must not probe the monitor endpoints').toEqual([])
})

test('a public link keeps the other three buttons working', async ({ page }) => {
  await preparePublicLink(page)
  await page.goto(`/${PUBLIC_CODE}`, { waitUntil: 'domcontentloaded' })
  await expect(page.getByTestId('home-floating-bar')).toBeVisible({ timeout: 30_000 })

  for (const testId of ['floating-refresh-button', 'floating-top-button']) {
    const button = page.getByTestId(testId)
    await expect(button).toBeEnabled()
    // The bar sits over the theme frame, so a transparent overlay above it would
    // pass a visibility check and still be unclickable.
    const hitTest = await button.evaluate((element) => {
      const rect = element.getBoundingClientRect()
      const target = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)
      return !!target && (target === element || element.contains(target))
    })
    expect(hitTest, `${testId} should be the pointer hit target`).toBe(true)
  }

  // The mode actually flips, which is what the button is for: it decides whether
  // a bookmark opens its LAN or its WAN address.
  const before = await page.locator('.home-floating-button').nth(2).getAttribute('data-testid')
  await page.locator('.home-floating-button').nth(2).click()
  await expect(page.locator('.home-floating-button').nth(2)).not.toHaveAttribute('data-testid', before!)
})

test('the keyboard opens the search palette on a public link, without commands', async ({ page }) => {
  await preparePublicLink(page)
  await page.goto(`/${PUBLIC_CODE}`, { waitUntil: 'domcontentloaded' })
  await expect(page.getByTestId('home-floating-bar')).toBeVisible({ timeout: 30_000 })

  await page.locator('body').click({ position: { x: 4, y: 4 } })
  await page.keyboard.press('a')

  const panel = page.getByTestId('command-center-panel')
  await expect(panel).toBeVisible()
  await expect(page.getByTestId('command-center-input')).toHaveValue('a')

  // Typing a plain character searches. `/` is the command prefix, and a public
  // visitor has no commands, so that list must stay empty rather than offer
  // operator actions that would refuse to run.
  await page.getByTestId('command-center-input').fill('/')
  await expect(page.locator('.command-center-result')).toHaveCount(0)
  await expect(page.locator('.command-center-empty')).toBeVisible()
})
