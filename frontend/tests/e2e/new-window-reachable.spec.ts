import { expect, test } from '@playwright/test'

/**
 * 端到端判定：新窗口书签点下去，窗口落在书签地址上并真的加载出页面。
 *
 * 这里刻意用本机可达的地址（`/clear.html`，由面板自己提供，返回 200 且有正文）。
 * 之前那条断言只查「页面数变多」，而窗口确实会开、内容却是错误页，等于没验——
 * 换成可达地址并且校验最终 URL 与正文，才真的能区分「开出来了」和「打开失败」。
 *
 * 设备模拟（P7 描述符）就是 DevTools 设备工具栏同一层模拟，不涉及真机。
 */

const mail = process.env.YIN_PANEL_E2E_MAIL
const password = process.env.YIN_PANEL_E2E_PASSWORD
const THEME_FRAME = '[data-testid="theme-home-frame"]'

test('点「新窗口」书签：新窗口落在书签地址上，并且真的加载出来', async ({ page, context }) => {
  test.skip(!mail || !password, '需要真实服务凭据')

  const login = await page.request.post('/api/login', { data: { mail, password } })
  const token = (await login.json())?.data?.token
  test.skip(!token, '登录失败')

  await page.addInitScript((t: string) => {
    try {
      localStorage.setItem('authStorage', JSON.stringify({
        data: { token: t, userInfo: { id: 1, name: 'e2e', mail: 'e2e', avatar: '', role: 1, createTime: 0, updateTime: 0 } },
        expire: null,
      }))
    } catch { /* 主题框内没有 storage */ }
  }, token)

  // 用面板自己提供的确定可达页面当书签目标：它返回 200 且有可校验的正文，
  // 因此「窗口开出来了」和「页面加载成功」可以被严格区分。
  await context.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname
    if (path.endsWith('/spaces/1/items')) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          code: 0,
          data: [
            { id: 1, itemIconGroupId: 1, itemIconGroupType: 1, title: '可达页', url: '/clear.html?from=e2e', openMethod: 2, icon: { itemType: 1, text: 'A', backgroundColor: '#007acc' }, sort: 1 },
          ],
        }),
      })
      return
    }
    await route.fallback()
  })

  await page.goto('/', { waitUntil: 'domcontentloaded' })
  const item = page.frameLocator(THEME_FRAME).locator('.yin-item').first()
  await expect(item, '主题必须渲染出书签').toBeVisible({ timeout: 30_000 })

  const before = context.pages().length
  await item.click()

  // 等到新窗口出现在书签地址上，而不是等「页面数变多」。
  let popup: import('@playwright/test').Page | undefined
  for (let attempt = 0; attempt < 60 && !popup; attempt++) {
    await page.waitForTimeout(250)
    popup = context.pages().find(p => p !== page && p.url().includes('/clear.html'))
  }
  const all = context.pages().map(p => p.url())
  console.log('ALL_PAGES', JSON.stringify(all))
  expect(popup, `新窗口必须落在书签地址上，实际: ${JSON.stringify(all)}`).toBeTruthy()
  expect(context.pages().length).toBeGreaterThan(before)

  // 关键：内容真的加载出来了，不是错误页。
  await popup!.waitForLoadState('domcontentloaded')
  expect(popup!.url(), '不能停在浏览器的加载失败页').not.toContain('chrome-error')
  const text = (await popup!.locator('body').innerText()).trim()
  expect(text.length, '新窗口必须有正文，说明页面真的渲染了').toBeGreaterThan(0)

  console.log('POPUP_OK', JSON.stringify({ url: popup!.url(), chars: text.length }))
  await popup!.close()
})