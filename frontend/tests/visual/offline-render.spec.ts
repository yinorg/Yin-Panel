import { expect, test } from '@playwright/test'

/**
 * The offline claim: with the network cut, the panel opens and the *theme*
 * renders — not the C3 fallback list.
 *
 * Two facts drive the shape of this test, both established by measurement rather
 * than assumed:
 *
 * 1. The worker registers on the `load` event, so the theme requests of the very
 *    first visit necessarily bypass it and are not cached. Offline capability
 *    therefore begins with the *second* visit. Nothing waits for the worker on
 *    the home page: paying that latency on every open would be the wrong trade
 *    for a page that must feel instant.
 * 2. Consequently the warm-up is two visits against the real server. Stubbing the
 *    document or the worker would leave the cache empty and the offline pass
 *    would be asserting against nothing.
 */
// This test drives a real service worker against the real server, so it must not
// share a worker with anything else: a parallel spec that stubs the same
// endpoints would race it for control of the page. Running it in its own file
// with one worker keeps the measurement honest — a shared worker would make the
// offline pass assert against a cache another test filled.
test.describe.configure({ mode: 'serial' })

test('the theme renders with the network offline after a warm-up', async ({ browser }) => {
  const context = await browser.newContext()

  const cacheNames = async (page: import('@playwright/test').Page) => page.evaluate(async () => {
    const names = await caches.keys()
    const counts: Record<string, number> = {}
    for (const name of names) counts[name] = (await (await caches.open(name)).keys()).length
    return counts
  })

  // Visit 1 — installs the worker, precaches the shell. The theme loads straight
  // from the network here, which is fine: this visit is online by definition.
  const first = await context.newPage()
  await first.goto('/', { waitUntil: 'domcontentloaded' })
  await first.waitForFunction(() => navigator.serviceWorker.controller !== null, undefined, { timeout: 30_000 })
  await first.waitForTimeout(2000)
  const afterFirst = await cacheNames(first)
  expect(afterFirst['workbox-precache-v2-http://127.0.0.1:3002/'], 'the shell must be precached for offline').toBeGreaterThan(0)

  // Visit 2 — the worker now controls the page, so the theme manifest, its grant
  // and its resources pass through the caching rules.
  const second = await context.newPage()
  await second.goto('/', { waitUntil: 'domcontentloaded' })
  await second.waitForTimeout(4000)
  const afterSecond = await cacheNames(second)
  expect(
    Object.keys(afterSecond).some(name => name.startsWith('yin-panel-theme-')),
    `the theme must be cached on the second visit, saw: ${Object.keys(afterSecond).join(', ')}`,
  ).toBe(true)

  // Visit 3 — offline.
  const offline = await context.newPage()
  await context.setOffline(true)
  await offline.goto('/', { waitUntil: 'domcontentloaded' })

  // The shell comes back from the precache.
  await expect(offline.locator('.sun-main')).toBeVisible({ timeout: 30_000 })

  // And the theme boots: its own chrome is present, which is only true if the
  // manifest and the resource bytes both came from the cache.
  const theme = offline.frameLocator('[data-testid="theme-home-frame"]')
  await expect(theme.locator('.yin-masthead')).toBeVisible({ timeout: 30_000 })
  await expect(theme.locator('[data-testid="theme-home-clock"]')).toBeVisible()

  // Crucially, the C3 fallback must NOT be what is on screen.
  await expect(offline.locator('[data-testid="home-fallback"]')).toHaveCount(0)

  await context.setOffline(false)
  await context.close()
})
