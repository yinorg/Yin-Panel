import { expect, test, type Page } from '@playwright/test'

/**
 * Opening system settings used to log a blocked-aria-hidden warning every time.
 *
 * NModal wraps its body in vueuc's FocusTrap, which renders two 0x0 `aria-hidden`
 * sentinels around the dialog and resolves its initial focus once, on mount, by
 * walking the dialog for a focusable descendant. Nothing inside the settings
 * dialog qualifies at that moment: the pages AppStarter hosts are an async chunk
 * (measured: zero focusable candidates, zero `.n-base-selection`, 273ms in), and
 * Naive's own close button is `tabindex="-1"`. With no candidate, the trap takes
 * its fallback branch and focuses its own `aria-hidden` sentinel, which the
 * browser then reports.
 *
 * The fix is to give the dialog something focusable that is rendered
 * synchronously, so the trap stays on its intended path.
 */

const SENTINEL_SELECTOR = 'div[aria-hidden="true"][tabindex="0"][style*="position: absolute"]'

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

test('opening system settings does not park focus on the focus-trap sentinel', async ({ page }) => {
  await prepareHome(page)
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await page.getByTestId('system-settings-button').click()
  await expect(page.locator('.app-starter-modal-content')).toBeVisible()

  // The behavioural half: focus belongs to a real control inside the dialog. This
  // is asserted first because a "no warning" check alone would also pass if the
  // focus trap were switched off entirely, which would drop keyboard containment.
  const focused = await page.evaluate((selector) => {
    const active = document.activeElement as HTMLElement | null
    return {
      isSentinel: !!active?.matches(selector),
      testId: active?.dataset?.testid ?? null,
      inDialog: !!active?.closest('.n-modal'),
      role: active?.getAttribute('role') ?? null,
      // Guards against a vacuous `isSentinel` check: a selector that matches
      // nothing would make the assertion above pass for any reason at all.
      sentinelCount: document.querySelectorAll(selector).length,
    }
  }, SENTINEL_SELECTOR)

  // The trap is still installed and the selector still finds it, so the
  // `isSentinel` check below is a real test rather than a tautology.
  expect(focused.sentinelCount, 'the focus trap should still render its two sentinels').toBe(2)
  expect(focused.isSentinel, 'focus must not rest on the aria-hidden trap sentinel').toBe(false)
  expect(focused.testId).toBe('app-starter-header-toggle')
  expect(focused.inDialog).toBe(true)
  expect(focused.role).toBe('button')

  // The reported half cannot be asserted here: headless Chromium does not emit
  // this warning at all, so a "no warning" expectation would pass no matter what
  // the code did. Measured, not assumed — provoking the identical condition
  // synthetically on a real page produces zero browser-log entries. What this
  // test *can* prove is the cause, which is the focus placement asserted above.
})

test('the settings header toggle is reachable and operable by keyboard', async ({ page }) => {
  // The toggle only ever had a click handler, so it was unreachable by keyboard.
  // Making it the dialog's first focusable element is also what keeps the focus
  // trap off its fallback path, so the two properties are asserted together.
  await prepareHome(page)
  await page.goto('/', { waitUntil: 'domcontentloaded' })

  await page.getByTestId('system-settings-button').click()
  const toggle = page.getByTestId('app-starter-header-toggle')
  await expect(toggle).toBeVisible()

  // SvgIcon keeps a constant class and swaps the icon through the <use> href, so
  // the href is what actually reflects the bound state.
  const iconHref = () => toggle.locator('use').first().getAttribute('href')
  const before = await iconHref()

  await toggle.focus()
  await expect(toggle).toBeFocused()
  await toggle.press('Enter')
  await expect.poll(iconHref).not.toBe(before)
  await toggle.press('Enter')
  await expect.poll(iconHref).toBe(before)

  await toggle.press('Space')
  await expect.poll(iconHref).not.toBe(before)
})
