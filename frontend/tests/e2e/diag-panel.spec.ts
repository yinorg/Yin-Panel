import { expect, test } from '@playwright/test'

/**
 * 诊断面板自身的验收。
 *
 * 它必须是可靠的，否则拿到的信息比没有更糟。这一条先证明：
 * 开关经快照送达、面板出现在主题文档里、点击后写出了可读的事实。
 *
 * 面板渲染在主题自己的文档内，父文档读不到它的内容（opaque 沙箱），所以唯一可靠的
 * 观察方式是通过 frame 句柄读文本。
 */

const mail = process.env.YIN_PANEL_E2E_MAIL
const password = process.env.YIN_PANEL_E2E_PASSWORD
const THEME_FRAME = '[data-testid="theme-home-frame"]'

async function openPanelWithTrace(page: import('@playwright/test').Page) {
  const login = await page.request.post('/api/login', { data: { mail, password } })
  const token = (await login.json())?.data?.token
  await page.addInitScript((t: string) => {
    try {
      localStorage.setItem('authStorage', JSON.stringify({
        data: { token: t, userInfo: { id: 1, name: 'diag', mail: 'diag', avatar: '', role: 1, createTime: 0, updateTime: 0 } },
        expire: null,
      }))
    } catch { /* 主题框内没有 storage */ }
  }, token)
  // `?yinDiag` 由 Core 读进快照再送给主题——主题自己读不到面板 URL。
  await page.goto('/?yinDiag=1', { waitUntil: 'domcontentloaded' })
  await expect(page.frameLocator(THEME_FRAME).locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })
}

test('诊断开关经快照送达，面板能记录一次真实点击', async ({ page }) => {
  test.skip(!mail || !password, '需要真实服务凭据')
  await openPanelWithTrace(page)

  // 面板是**第一次点击时**才创建的（之前那条先等它可见是错的：等不到，
  // 因为没东西触发它）。所以顺序必须是先点、再等。
  await page.frameLocator(THEME_FRAME).locator('.yin-item').first().click()

  const panel = page.frameLocator(THEME_FRAME).locator('.yin-diag')
  await expect(panel, '点击后应当出现诊断面板').toBeVisible({ timeout: 10_000 })
  const text = (await panel.textContent()) || ''
  console.log('DIAG_BEGIN')
  console.log(text)
  console.log('DIAG_END')

  // 这些是「点击确实到达主题」的证据，也是判断「事件没到 vs 开窗失败」的分界线。
  expect(text, '面板必须记录到这次点击').toContain('点击: item')
  expect(text, '必须记录事件是否到达文档').toMatch(/文档收到的最近事件/)
  expect(text, '必须记录锚点的导航目标').toMatch(/href:/)
  expect(text, '必须记录原生 target').toMatch(/target:/)
})

test('没有 ?yinDiag 时不渲染任何诊断 DOM', async ({ page }) => {
  test.skip(!mail || !password, '需要真实服务凭据')
  const login = await page.request.post('/api/login', { data: { mail, password } })
  const token = (await login.json())?.data?.token
  await page.addInitScript((t: string) => {
    try {
      localStorage.setItem('authStorage', JSON.stringify({
        data: { token: t, userInfo: { id: 1, name: 'diag', mail: 'diag', avatar: '', role: 1, createTime: 0, updateTime: 0 } },
        expire: null,
      }))
    } catch { /* 主题框内没有 storage */ }
  }, token)
  await page.goto('/', { waitUntil: 'domcontentloaded' })
  await expect(page.frameLocator(THEME_FRAME).locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })

  // 正常使用时主题不该为调试留任何后门。
  await expect(page.frameLocator(THEME_FRAME).locator('.yin-diag')).toHaveCount(0)
})