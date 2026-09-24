import { expect, test, type Page } from '@playwright/test'

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
      data = { panel: { clockShowSecond: false, searchBoxShow: true, netModeChangeButtonShow: true } }
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
