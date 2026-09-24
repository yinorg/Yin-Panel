import { expect, test } from '@playwright/test'

const groups = [
  { id: 10, title: 'Alpha', sort: 1, parentId: null },
  { id: 11, title: 'Alpha child', sort: 1, parentId: 10 },
  { id: 20, title: 'Beta', sort: 2, parentId: null },
]

const itemsByGroup: Record<number, { id: number; title: string; url: string; description: string; openMethod: number; itemIconGroupId: number; icon: { itemType: number; text: string; backgroundColor: string } }[]> = {
  10: [],
  11: [{ id: 111, title: 'Nested bookmark with a long title that should wrap on narrow screens', url: 'https://example.com/nested', description: '', openMethod: 2, itemIconGroupId: 11, icon: { itemType: 1, text: 'N', backgroundColor: '#704c2c' } }],
  20: [{ id: 201, title: 'Beta bookmark', url: 'https://example.com/beta', description: '', openMethod: 2, itemIconGroupId: 20, icon: { itemType: 1, text: 'B', backgroundColor: '#384b75' } }],
}

test('directory layout keeps selected group bookmarks visible across desktop, tablet, and mobile', async ({ page }) => {
  let configuredLayout = 'standard'
  let monitorVisible = false
  let monitorInfoStyle = false

  await page.addInitScript(() => {
    localStorage.setItem('authStorage', JSON.stringify({
      data: { token: 'test-token', userInfo: { id: 1 } },
      expire: null,
    }))
  })
  await page.route('http://127.0.0.1:3002/**', (route) => route.continue())

  await page.route('**/api/**', async (route) => {
    const url = new URL(route.request().url())
    const path = url.pathname
    let data: unknown = {}

    if (path.endsWith('/getConfig'))
      data = { panel: { homeLayout: configuredLayout, clockShowSecond: false, searchBoxShow: false, systemMonitorShow: monitorVisible, iconStyle: monitorInfoStyle ? 0 : 1, systemMonitorShowTitle: true } }
    else if (path.endsWith('/getEnableStatus'))
      data = { enabled: monitorVisible, refresh_interval: 10 }
    else if (path.endsWith('/getByName'))
      data = { list: ['cpu', 'memory', 'network'].map(monitorType => ({ monitorType, extendParam: { backgroundColor: '#20242a', color: '#fff', progressColor: '#4c9a78', progressRailColor: '#39414a' } })) }
    else if (path.endsWith('/getSnapshot'))
      data = { cpu: 35, memory: 48, network: { upload: 4, download: 8 } }
    else if (path.endsWith('/spaces'))
      data = [{ id: 1, name: 'Test Space', type: 'personal', ownerUserId: 1 }]
    else if (path.endsWith('/groups'))
      data = groups
    else if (path.endsWith('/items'))
      data = itemsByGroup[Number(url.searchParams.get('groupId'))] || []

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ code: 0, data }),
    })
  })

  for (const viewport of [
    { width: 1440, height: 900 },
    { width: 768, height: 1024 },
    { width: 390, height: 844 },
  ]) {
    if (page.url() !== 'about:blank')
      await page.evaluate(() => localStorage.clear())
    configuredLayout = 'standard'
    await page.setViewportSize(viewport)
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const nestedItem = page.getByTestId('home-item').filter({ has: page.getByText('Nested bookmark with a long title that should wrap on narrow screens', { exact: true }) })
    await expect(page.locator('html')).toHaveAttribute('data-yin-layout', 'standard')
    await expect(nestedItem).toHaveCount(1)
    await expect(nestedItem).not.toBeVisible()

    configuredLayout = 'directory'
    await page.reload({ waitUntil: 'domcontentloaded' })
    await expect(page.locator('html')).toHaveAttribute('data-yin-layout', 'directory')
    const navigation = page.getByRole('navigation', { name: 'Bookmark groups' })
    const navigationState = await navigation.evaluate((element) => {
      const rect = element.getBoundingClientRect()
      const style = getComputedStyle(element)
      return { display: style.display, visibility: style.visibility, opacity: style.opacity, width: rect.width, height: rect.height, top: rect.top }
    })
    expect(navigationState, JSON.stringify(navigationState)).toMatchObject({ display: 'flex', visibility: 'visible', opacity: '1' })
    expect(navigationState.width).toBeGreaterThan(0)
    expect(navigationState.height).toBeGreaterThan(0)
    const folderButton = page.getByRole('button', { name: 'Alpha', exact: true })
    const folderButtonHeight = await folderButton.evaluate((element) => element.getBoundingClientRect().height)
    expect(folderButtonHeight).toBeGreaterThanOrEqual(44)

    const brandControls = page.locator('.directory-brand-controls')
    const brandAlignment = await brandControls.evaluate((element) => {
      const brand = element.querySelector(':scope > span')!.getBoundingClientRect()
      const clock = element.querySelector('.directory-clock')!.getBoundingClientRect()
      const container = element.getBoundingClientRect()
      return {
        brandLines: getComputedStyle(element.querySelector(':scope > span')!).whiteSpace,
        brandRight: brand.right,
        clockRight: clock.right,
        containerRight: container.right,
      }
    })
    expect(brandAlignment.brandLines).toBe('nowrap')
    expect(Math.abs(brandAlignment.clockRight - brandAlignment.containerRight)).toBeLessThanOrEqual(1)

    await expect(nestedItem).toBeVisible()
    const nestedBox = await nestedItem.boundingBox()
    expect(nestedBox?.width).toBeGreaterThan(0)
    expect(nestedBox?.height).toBeGreaterThan(0)
    const nestedIcon = nestedItem.locator('.app-icon-small-icon')
    await expect(nestedIcon).toBeVisible()
    const iconBox = await nestedIcon.boundingBox()
    expect(iconBox?.width).toBeCloseTo(30, 0)
    expect(iconBox?.height).toBeCloseTo(30, 0)
    const directoryRow = nestedItem.locator('.app-icon-small--directory')
    const directoryRowStyle = await directoryRow.evaluate((element) => getComputedStyle(element).display)
    expect(directoryRowStyle).toBe('flex')
    const titleStyle = await nestedItem.locator('.app-icon-small-title').evaluate((element) => {
      const style = getComputedStyle(element)
      return { whiteSpace: style.whiteSpace, overflowWrap: style.overflowWrap, maxWidth: style.maxWidth }
    })
    expect(titleStyle.whiteSpace).toBe('normal')
    expect(titleStyle.overflowWrap).toBe('anywhere')
    expect(titleStyle.maxWidth).not.toBe('none')
    const contentBox = await page.locator('.home-content').boundingBox()
    expect(contentBox?.x).toBeGreaterThanOrEqual(0)
    expect((contentBox?.x || 0) + (contentBox?.width || 0)).toBeLessThanOrEqual(viewport.width + 1)

    await page.getByRole('button', { name: 'Beta', exact: true }).click()
    await expect(page.getByTestId('home-item').filter({ has: page.getByText('Beta bookmark', { exact: true }) })).toBeVisible()
    await expect(nestedItem).toHaveCount(0)

    monitorVisible = true
    monitorInfoStyle = viewport.width === 390
    await page.reload({ waitUntil: 'domcontentloaded' })
    const monitor = page.locator('.system-monitor-layer')
    await expect(monitor).toBeVisible()
    await page.getByRole('button', { name: 'Beta', exact: true }).click()
    await expect(page.getByText('CPU', { exact: true }).first()).toBeVisible()
    const firstItem = page.getByTestId('home-item').filter({ has: page.getByText('Beta bookmark', { exact: true }) })
    await expect(firstItem).toBeVisible()
    const monitorLayout = await monitor.evaluate((element) => {
      const style = getComputedStyle(element)
      const bounds = element.getBoundingClientRect()
      return {
        position: style.position,
        maxHeight: style.maxHeight,
        overflowY: style.overflowY,
        bottom: bounds.bottom,
        scrollHeight: element.scrollHeight,
        clientHeight: element.clientHeight,
      }
    })
    expect(monitorLayout.position).toBe('relative')
    expect(monitorLayout.maxHeight).toBe('none')
    expect(monitorLayout.overflowY).toBe('visible')
    expect(monitorLayout.scrollHeight).toBeLessThanOrEqual(monitorLayout.clientHeight + 1)
    const firstItemBox = await firstItem.boundingBox()
    expect(firstItemBox?.y).toBeGreaterThanOrEqual(monitorLayout.bottom)
  }
})
