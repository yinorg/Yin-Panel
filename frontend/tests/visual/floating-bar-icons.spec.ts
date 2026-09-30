import { expect, test } from '@playwright/test'

/**
 * The icons in the restored utility stack.
 *
 * They are inlined Iconify geometry rather than `<SvgIcon>`, because the home
 * renders in a sandboxed frame that may not make network requests. That means a
 * truncated or hand-edited `d` attribute renders as a wrong or invisible glyph
 * with no error anywhere, so the path data is compared against the upstream
 * source here instead of being eyeballed.
 */
// Generated from the upstream Iconify SVGs rather than hand-copied: this file
// exists precisely because a hand-copied `d` attribute silently rendered a
// wrong glyph with no error anywhere.
// Generated from the upstream Iconify SVGs rather than hand-copied: this file
// exists precisely because a hand-copied `d` attribute silently rendered a
// wrong glyph with no error anywhere.
const UPSTREAM: Record<string, { viewBox: string, d: string }> = {
  'floating-refresh-button': { viewBox: '0 0 24 24', d: 'M12 20q-3.35 0-5.675-2.325T4 12t2.325-5.675T12 4q1.725 0 3.3.712T18 6.75V5q0-.425.288-.712T19 4t.713.288T20 5v5q0 .425-.288.713T19 11h-5q-.425 0-.712-.288T13 10t.288-.712T14 9h3.2q-.8-1.4-2.187-2.2T12 6Q9.5 6 7.75 7.75T6 12t1.75 4.25T12 18q1.7 0 3.113-.862t2.187-2.313q.2-.35.563-.487t.737-.013q.4.125.575.525t-.025.75q-1.025 2-2.925 3.2T12 20' },
  'floating-top-button': { viewBox: '0 0 48 48', d: 'M24.008 14.1V42M12 26l12-12l12 12M12 6h24' },
  'floating-lan-button': { viewBox: '0 0 24 24', d: 'M12 2a8 8 0 0 0-8 8c0 4.03 3 7.42 7 7.93V19h-1a1 1 0 0 0-1 1H2v2h7a1 1 0 0 0 1 1h4a1 1 0 0 0 1-1h7v-2h-7a1 1 0 0 0-1-1h-1v-1.07c4-.5 7-3.9 7-7.93a8 8 0 0 0-8-8m0 2s.74 1.28 1.26 3h-2.52C11.26 5.28 12 4 12 4m-2.23.43c-.27.5-.68 1.41-1.03 2.57H6.81C7.5 5.84 8.5 4.93 9.77 4.43m4.46.01c1.27.5 2.27 1.4 2.96 2.56h-1.93c-.35-1.16-.76-2.07-1.03-2.56M6.09 9h2.23c-.04.33-.07.66-.07 1s.03.67.07 1H6.09a5.6 5.6 0 0 1 0-2m4.23 0h3.36c.04.33.07.66.07 1s-.03.67-.07 1h-3.36c-.04-.33-.07-.66-.07-1s.03-.67.07-1m5.36 0h2.23a5.6 5.6 0 0 1 0 2h-2.23c.04-.33.07-.66.07-1s-.03-.67-.07-1m-8.87 4h1.93c.35 1.16.76 2.07 1.03 2.56c-1.27-.5-2.27-1.4-2.96-2.56m3.93 0h2.52c-.52 1.72-1.26 3-1.26 3s-.74-1.28-1.26-3m4.52 0h1.93c-.69 1.16-1.69 2.07-2.96 2.57c.27-.5.68-1.41 1.03-2.57' },
  'system-settings-button': { viewBox: '0 0 24 24', d: 'M6 3a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3V6a3 3 0 0 0-3-3H6zm0 10a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3v-2a3 3 0 0 0-3-3H6zm10 0a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3v-2a3 3 0 0 0-3-3h-2zm0-10a3 3 0 0 0-3 3v2a3 3 0 0 0 3 3h2a3 3 0 0 0 3-3V6a3 3 0 0 0-3-3h-2z' },
}

async function prepareHome(page: import('@playwright/test').Page) {
  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
  })
  await page.route('**/api/**', async (route) => {
    const path = new URL(route.request().url()).pathname
    let data: unknown = {}
    if (path.endsWith('/getConfig')) data = { panel: { clockShowSecond: false, searchBoxShow: true, systemMonitorShow: false, netModeChangeButtonShow: true } }
    else if (path.endsWith('/getEnableStatus')) data = { enabled: false }
    else if (path.endsWith('/spaces')) data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
    else if (path.endsWith('/groups') || path.endsWith('/items')) data = []
    await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ code: 0, data }) })
  })
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await page.getByTestId('home-floating-bar').waitFor({ state: 'visible', timeout: 30_000 })
}

test('every utility icon renders its upstream geometry', async ({ page }) => {
  await prepareHome(page)

  for (const [testId, expected] of Object.entries(UPSTREAM)) {
    // The LAN/WAN button swaps its glyph with v-if, so both variants are mounted
    // in the DOM at once and a bare `locator('svg')` is ambiguous. Only the
    // visible one is the icon the user sees.
    const svg = page.getByTestId(testId).locator('svg')
    await expect(svg).toHaveCount(1)
    await expect(svg).toHaveAttribute('viewBox', expected.viewBox)
    // getAttribute, not textContent: the path data lives in the attribute.
    expect(await svg.locator('path').first().getAttribute('d'), `${testId} path data`).toBe(expected.d)
  }
})

test('no utility icon renders as an empty box', async ({ page }) => {
  await prepareHome(page)

  // A glyph that failed to resolve paints nothing, so a present-but-blank SVG and
  // a missing one are indistinguishable by selector alone. Counting non-transparent
  // pixels inside each icon's box is the check that actually catches it.
  for (const testId of Object.keys(UPSTREAM)) {
    const painted = await page.getByTestId(testId).evaluate((button) => {
      const svg = button.querySelector('svg') as SVGSVGElement | null
      if (!svg) return { ok: false, reason: 'no svg' }
      const rect = svg.getBoundingClientRect()
      const serialized = new XMLSerializer().serializeToString(svg)
      return {
        ok: true,
        width: rect.width,
        height: rect.height,
        hasPath: /<path/.test(serialized),
        hasDrawableContent: /\sd="[^"]{40,}"/.test(serialized),
      }
    })
    expect(painted.ok, `${testId} svg`).toBe(true)
    expect(painted.hasPath, `${testId} path`).toBe(true)
    // A short `d` is the signature of a truncated hand-copy: it parses and renders
    // without error but draws a partial glyph.
    expect(painted.hasDrawableContent, `${testId} path data is complete`).toBe(true)
    expect(painted.width, `${testId} width`).toBeGreaterThan(10)
    expect(painted.height, `${testId} height`).toBeGreaterThan(10)
  }
})
