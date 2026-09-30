import { expect, test } from '@playwright/test'

/**
 * `/clear.html` is the way out of a broken worker or a poisoned cache, so it has
 * to work when the application bundle does not: it is a plain static document
 * with its own inline script and no dependency on the app.
 *
 * Two properties matter and are easy to lose:
 *
 * 1. It must actually clear what it says it clears.
 * 2. It must reach the network. A static page reached by path is precached by
 *    default, and the worker's navigation fallback answers unknown navigations
 *    with `index.html`, so without the deny entry the page would never run — and
 *    it would fail exactly when it is needed. That case is asserted by checking
 *    the built worker, since the failure only appears once a worker controls the
 *    page.
 */

test('the clear page clears local state and reports what it did', async ({ page }) => {
  await page.goto('/clear.html', { waitUntil: 'domcontentloaded' })

  // It is the static document, not the app shell: no bundle, no app root.
  await expect(page.getByTestId('static-clear')).toBeVisible()
  await expect(page.locator('#app')).toHaveCount(0)

  // Seed every store it claims to handle, then clear and reload to confirm the
  // state is gone rather than merely reported as gone.
  await page.evaluate(async () => {
    localStorage.setItem('probe-local', '1')
    sessionStorage.setItem('probe-session', '1')
    if (typeof caches !== 'undefined') {
      const cache = await caches.open('probe-cache')
      await cache.put('/probe', new Response('probe'))
    }
  })

  await page.reload({ waitUntil: 'domcontentloaded' })
  await expect(page.locator('#status')).toContainText('Done')

  // Each step is listed, so a silent partial failure cannot read as success.
  const results = await page.locator('#results li').allTextContents()
  expect(results.length).toBeGreaterThanOrEqual(4)
  expect(results.filter(line => line.startsWith('✓')).length).toBe(results.length)

  const leftovers = await page.evaluate(async () => ({
    local: Object.keys(localStorage).filter(key => key.startsWith('probe-')),
    session: Object.keys(sessionStorage).filter(key => key.startsWith('probe-')),
    caches: typeof caches === 'undefined' ? [] : (await caches.keys()).filter(name => name.startsWith('probe-')),
  }))
  expect(leftovers.local).toEqual([])
  expect(leftovers.session).toEqual([])
  expect(leftovers.caches).toEqual([])
})

test('the clear page follows the browser language', async ({ browser }) => {
  // The page cannot load the app's i18n, so its translations are inlined. That
  // makes drift the real risk: a locale present in one place and missing in the
  // other. This asserts the page matches `resolveBrowserLocale()` for each
  // supported language, which also proves no locale silently falls back to English.
  const cases = [
    { locale: 'zh-TW', heading: '清理本機資料' },
    { locale: 'zh-CN', heading: '清理本地数据' },
    { locale: 'ja-JP', heading: 'ローカルデータを消去' },
    { locale: 'ko-KR', heading: '로컬 데이터 지우기' },
    { locale: 'de-DE', heading: 'Browserdaten löschen' },
    { locale: 'fr-FR', heading: 'Effacer les données locales' },
    { locale: 'es-ES', heading: 'Borrar datos locales' },
    { locale: 'pt-BR', heading: 'Limpar dados locais' },
    { locale: 'ru-RU', heading: 'Очистка локальных данных' },
    { locale: 'en-US', heading: 'Clear local data' },
    // An unsupported language must land on English, not on Simplified Chinese.
    { locale: 'nl-NL', heading: 'Clear local data' },
  ]

  for (const item of cases) {
    const context = await browser.newContext({ locale: item.locale })
    const page = await context.newPage()
    await page.goto('/clear.html', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('h1'), `heading for ${item.locale}`).toHaveText(item.heading)
    // The document language must follow too, or assistive tech reads it wrong.
    await expect(page.locator('html')).toHaveAttribute('lang', item.locale === 'nl-NL' ? 'en-US' : item.locale)
    await context.close()
  }
})

test('the built worker refuses to answer the clear page with the app shell', async ({ request }) => {
  const response = await request.get('/sw.js')
  expect(response.ok()).toBeTruthy()
  const worker = await response.text()

  // The deny entry must be present, or the worker answers the navigation with
  // `index.html` and the clear page never executes.
  expect(worker).toContain('clear')

  // And the page must not be precached: a precached copy is a copy that can be
  // served from a cache the visitor is trying to escape.
  const precache = worker.slice(worker.indexOf('precacheAndRoute('), worker.indexOf('precacheAndRoute(') + 4000)
  expect(precache).not.toContain('clear.html')
})
