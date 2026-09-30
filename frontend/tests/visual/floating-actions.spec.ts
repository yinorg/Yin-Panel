import { expect, test, type Page } from '@playwright/test'

/**
 * The fixed utility stack on the right edge of the home page: refresh, back to
 * top, the LAN/WAN switch and system settings.
 *
 * P4a deleted the Core's copy of this chrome and it was reinstated as Core-owned
 * chrome again, because it is viewport chrome: the theme runs in a frame sized to
 * its full content height so the Core can scroll it, and inside that frame
 * `position: fixed` resolves against the content box — a theme-drawn stack would
 * sit at the bottom of the page and only appear once you scrolled there.
 */
async function prepareHome(page: Page) {
  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
  })
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

async function expectFloatingButtonsHitTestable(page: Page) {
  for (const testId of [
    'floating-refresh-button',
    'floating-top-button',
    'floating-lan-button',
    'system-settings-button',
  ]) {
    const button = page.getByTestId(testId)
    await expect(button).toBeVisible()
    // Asserting the hit target matters because the bar sits over the theme frame;
    // a transparent overlay above it would pass a visibility check and still be
    // unclickable.
    const hitTest = await button.evaluate((element) => {
      const rect = element.getBoundingClientRect()
      const target = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2)
      return !!target && (target === element || element.contains(target))
    })
    expect(hitTest, `${testId} should be the pointer hit target`).toBe(true)
  }
}

test('floating home actions remain reachable on desktop and mobile', async ({ page }) => {
  await prepareHome(page)
  await page.setViewportSize({ width: 390, height: 844 })
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await expectFloatingButtonsHitTestable(page)
  await page.setViewportSize({ width: 1280, height: 720 })
  await expectFloatingButtonsHitTestable(page)
  await page.getByTestId('system-settings-button').click()
  await expect(page.locator('.app-starter-modal-content')).toBeVisible()
})

test('the floating stack keeps the geometry the pre-theme build had', async ({ page }) => {
  // Measured off the old build's own ruler screenshot (1920x1080) rather than
  // eyeballed: the stack occupied x 1864..1910 and y 894..1030, i.e. 46px wide,
  // four 34px cells with no gaps, 10px from the right edge and 50px from the
  // bottom. Reproducing those numbers is what keeps the restoration honest.
  await prepareHome(page)
  await page.setViewportSize({ width: 1920, height: 1080 })
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  const bar = page.getByTestId('home-floating-bar')
  await expect(bar).toBeVisible({ timeout: 30_000 })
  const box = await bar.boundingBox()
  expect(box).not.toBeNull()
  expect(box!.x).toBeCloseTo(1864, 0)
  expect(box!.width).toBeCloseTo(46, 0)
  expect(box!.height).toBeCloseTo(136, 0)
  // 10px from the right edge, 50px up from the bottom.
  expect(1920 - (box!.x + box!.width)).toBeCloseTo(10, 0)
  expect(1080 - (box!.y + box!.height)).toBeCloseTo(50, 0)

  // Four 34px cells, stacked flush, matching a Naive vertical button group.
  const cells = page.locator('.home-floating-button')
  await expect(cells).toHaveCount(4)
  for (let index = 0; index < 4; index += 1) {
    const cell = await cells.nth(index).boundingBox()
    expect(cell!.height).toBeCloseTo(34, 0)
    expect(cell!.width).toBeCloseTo(46, 0)
    expect(cell!.y).toBeCloseTo(box!.y + index * 34, 0)
  }
})

test('the stack stays put when the page is scrolled', async ({ page }) => {
  // The regression that sent this back to the Core: drawn by the theme, the bar
  // was positioned against the frame's content box and therefore only showed up
  // once the page was scrolled to the bottom.
  await prepareHome(page)
  await page.setViewportSize({ width: 1280, height: 720 })
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  const before = await page.getByTestId('home-floating-bar').boundingBox()
  await page.mouse.wheel(0, 4000)
  await page.waitForTimeout(400)
  const after = await page.getByTestId('home-floating-bar').boundingBox()
  expect(after!.y).toBeCloseTo(before!.y, 0)
  await expect(page.getByTestId('floating-refresh-button')).toBeInViewport()
})
