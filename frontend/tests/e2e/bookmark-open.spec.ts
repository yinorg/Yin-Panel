import { expect, test, type Page } from '@playwright/test'
import { mockApi } from '../support/panel-mock'

/**
 * 「新窗口」书签在手机上打不开，以及清理页被沙箱化之后四项全失败。
 *
 * 这两件事本机复现不了，原因必须写清楚，否则下一个人又会以为这里是假绿：
 *
 * 1. Playwright 默认给 Chromium 传 `--disable-popup-blocking`（见
 *    `playwright-core/lib/coreBundle.js`），弹窗拦截器根本没开。本文件用
 *    `ignoreDefaultArgs` 去掉它，这一步是有效的、已在启动参数里核对过。
 * 2. 去掉之后本机依然测不出手机的行为，因为 Chromium 的 transient activation 有
 *    约 5 秒窗口，Core 在点击后几毫秒调 `window.open` 仍在窗口内；而 Chromium 又会
 *    把子框的手势传播给父窗口。手机浏览器两样都不做：手势不跨框，且时窗更紧。
 *
 * 所以这里不假装能测手机，而是把手机的条件**如实模拟**出来：把 Core 的 `window.open`
 * 替换成「只有调用者自己仍在手势内才放行，否则返回 null」。这个模拟对 iOS Safari 与
 * Android Chrome 的策略都成立（两者都拒绝跨框手势的新窗口），因此它在两种引擎上都能
 * 判别修复是否有效，而不是只对某一个引擎有效。
 *
 * 上层断言的是浏览器行为（有没有新窗口、它是什么 URL），不是我们自己写的模拟器。
 */

const THEME_FRAME = '[data-testid="theme-home-frame"]'

/**
 * 模拟移动端的弹窗门槛。
 *
 * 必须在点击之前装好，且要装到**所有**文档上：Core 的顶层文档是判据所在，主题框里
 * 保持原生行为，这样"主题在自己的手势里开窗"才是被允许的那条路径。
 *
 * 注意这里不能用 `navigator.userActivation` 当判据：Playwright 的 `page.evaluate`
 * 自带 `userGesture: true`，任何经由它发起的调用都会被算成在手势内，测出来永远是放行。
 * 所以用手势的真实载体——事件栈：只有从真实输入事件（`isTrusted`）同步派生的调用才算。
 */
/**
 * 装上移动端的弹窗门槛，**顶层文档和主题框都要装**。
 *
 * 漏掉主题框会让这套判据彻底失效：它有自己的 realm，`window.open` 是原生函数。
 * 实测过这一点——沙箱令牌退回 `allow-scripts`（真机上主题开窗必被拒）时，只在顶层
 * 装门槛的话测试照样绿，因为主题框里的原生调用根本没被拦到。
 *
 * 模拟两件事，缺一不可：
 * 1. **沙箱令牌**：主题框若没有 `allow-popups`，它开的任何窗口都必须失败。这是
 *    手机上"点了没反应"的直接原因。
 * 2. **手势**：Core 不在点击的同步栈里，它自己开窗必须失败。手机上跨框的手势不
 *    传给父窗口，桌面会传。
 */
async function installMobilePopupGate(page: Page) {
  await page.addInitScript(() => {
    const inRealGesture = () => {
      // 由真实输入事件同步派生的调用栈上会带有该事件。
      // 不能用 `navigator.userActivation`：`page.evaluate` 自带 userGesture，
      // 任何经由它发起的调用都会被算成在手势内。
      const event = window.event as Event | undefined
      return !!event && event.isTrusted === true
    }

    const nativeOpen = window.open.bind(window)
    const state = { refusals: [] as string[], inFrame: window !== window.top }
    ;(window as unknown as { __popupGate: typeof state }).__popupGate = state

    // 主题框是否拿到了开窗权限。有 `allow-scripts` 之外的框（顶层）当然有。
    const frameMayOpenPopups = () => {
      if (!state.inFrame) return true
      const frame = window.frameElement as HTMLIFrameElement | null
      if (!frame) return true
      const tokens = (frame.getAttribute('sandbox') || '').split(/\s+/)
      return tokens.includes('allow-popups')
    }

    window.open = ((url?: string | URL, target?: string, features?: string) => {
      const name = target === undefined ? '' : String(target)
      const href = url === undefined || url === null ? '' : String(url)

      // 主题在自己的手势里直接开目标窗口——这才是修复依赖的那一步，所以它在主题框里
      // 只要手势还在就必须被放行。少了这个分支，测试会误判成"被拦"。
      //
      // 沙箱令牌仍然要查：主题框没有 `allow-popups` 时开窗必被拒，这是手机上"点了没
      // 反应"的直接原因之一。
      if (state.inFrame && !frameMayOpenPopups()) {
        state.refusals.push(`refused-no-popup-token:${href}`)
        return null
      }
      if (!inRealGesture()) {
        state.refusals.push(`refused-no-gesture:${href}`)
        return null
      }
      state.refusals.push(`${state.inFrame ? 'theme-opened' : 'core-opened'}:${name}:${href}`)
      return nativeOpen(url, target, features)
    }) as typeof window.open
  })
}

/** 主题框里预打开的窗口会让 context 多出页面；只保留原始页。 */
async function newWindowURLs(page: Page): Promise<string[]> {
  const urls: string[] = []
  for (const ctxPage of page.context().pages()) {
    if (ctxPage === page) continue
    urls.push(ctxPage.url())
  }
  return urls
}

test.describe('E2E: 新窗口书签（移动端门槛）', () => {
  test.beforeEach(async ({ page }) => {
    await mockApi(page, { theme: 'real' })
    await installMobilePopupGate(page)
  })

  test('点「新窗口」书签会开出新窗口，并导航到解析后的地址', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator(THEME_FRAME)
    const item = theme.locator('.yin-item').first()
    await expect(item).toBeVisible({ timeout: 30_000 })

    // 快照里必须有 openMethod 和解析后的 url。少了 url 主题就开不出窗（而新窗口可能
    // 仍会由 Core 兜底开出来，看起来一切正常）——这一条正是为那个陷阱准备的。
    const snapshot = await theme.locator('body').evaluate((body: Element) => {
      const button = body.ownerDocument.querySelector('.yin-item') as (Element & { _yin?: { item?: { openMethod?: number, url?: string } } }) | null
      const entry = button?._yin?.item
      return { openMethod: entry?.openMethod, url: (entry as Record<string, unknown> | undefined)?.url ?? null }
    })
    expect(snapshot.openMethod).toBe(2)
    expect(snapshot.url, '主题要自己开窗就必须拿到解析后的地址').toBe('https://code.visualstudio.com/')

    await item.click()

    // 关键断言：新窗口停在书签地址上。
    await expect.poll(async () => (await newWindowURLs(page)).some(u => u.includes('code.visualstudio.com')), { timeout: 15_000 })
      .toBe(true)

    // 当前页不能被导航走：这是「新窗口」，不是「当前页跳转」。
    expect(page.url()).toContain('127.0.0.1:3002')
    expect(page.url()).not.toContain('code.visualstudio.com')

    // 任何一次被拒都说明修复没生效，直接指出是哪一种。
    const log = await page.evaluate(() => (window as unknown as { __popupGate: { refusals: string[] } }).__popupGate.refusals)
    expect(log.filter(e => e.startsWith('refused-'))).toEqual([])
  })

  test('新窗口书签是原生锚点，不依赖 JS 开窗', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator(THEME_FRAME)
    const item = theme.locator('.yin-item').first()
    await expect(item).toBeVisible({ timeout: 30_000 })

    // 核心断言：书签必须是真正的锚点，而不是靠脚本开窗的 button。
    //
    // 这是「不依赖 JS 事件」的保证：即使主题的点击处理器因为任何原因没有执行，
    // `target="_blank"` 依然会让浏览器原生开出新窗口。此前用 `window.open` 的写法
    // 一旦处理器没跑到，就是「点了完全没反应」，且从外部无法分辨。
    const shape = await item.evaluate((el: Element) => ({
      tag: el.tagName,
      href: el.getAttribute('href'),
      target: el.getAttribute('target'),
      rel: el.getAttribute('rel'),
    }))
    expect(shape.tag, '新窗口书签必须是锚点').toBe('A')
    expect(shape.target, '必须声明原生新窗口').toBe('_blank')
    expect(shape.rel, '必须切断 opener').toContain('noopener')
    expect(shape.href, '锚点必须带解析后的书签地址').toBe('https://code.visualstudio.com/')

    await item.click()
    await expect.poll(async () => (await newWindowURLs(page)).some(u => u.includes('code.visualstudio.com')), { timeout: 15_000 })
      .toBe(true)

    // 主题不该再用脚本开窗：那条路径在部分环境下不成立，而且和原生导航叠加会开两次。
    const themeCalls = await theme.locator('body').evaluate((body: Element) => {
      const gate = (body.ownerDocument.defaultView as unknown as { __popupGate?: { refusals: string[] } }).__popupGate
      return gate ? gate.refusals : null
    })
    expect(themeCalls).not.toBeNull()
    expect(themeCalls!.filter(e => e.startsWith('theme-opened:')), '主题不该再用脚本开窗').toEqual([])
    expect(themeCalls!.filter(e => e.startsWith('refused-'))).toEqual([])

    // Core 也不该替它开：Core 没有手势，手机上会被拒。
    const coreCalls = await page.evaluate(() => (window as unknown as { __popupGate: { refusals: string[] } }).__popupGate.refusals)
    expect(coreCalls.filter(e => e.startsWith('refused-'))).toEqual([])
    expect(coreCalls.filter(e => e.startsWith('core-opened:') && e.includes('code.visualstudio.com')))
      .toEqual([])
  })

  test('同页打开的书签不被改成新窗口', async ({ page }) => {
    await mockApi(page, { theme: 'real' })
    // 把第一个 item 换成「当前页打开」，确认新窗口机制不会误伤它。
    await page.route('**/api/**', async (route) => {
      const url = new URL(route.request().url())
      if (url.pathname.endsWith('/items')) {
        await route.fulfill({
          status: 200,
          contentType: 'application/json',
          body: JSON.stringify({ code: 0, data: [{ id: 1, title: '同页', url: 'https://example.com/same-tab', description: '', openMethod: 1, itemIconGroupId: 1, icon: { itemType: 1, text: 'S', backgroundColor: '#333' } }] }),
        })
        return
      }
      await route.fallback()
    })

    await page.goto('/', { waitUntil: 'domcontentloaded' })
    const theme = page.frameLocator(THEME_FRAME)
    const item = theme.locator('.yin-item').first()
    await expect(item).toBeVisible({ timeout: 30_000 })
    await item.click()

    // 当前页跳转，不开新窗口。
    await page.waitForURL('**example.com/same-tab**', { timeout: 15_000 })
    expect(await newWindowURLs(page)).toEqual([])
  })
})

test.describe('E2E: 清理页', () => {
  test('顶层打开时四项全部成功', async ({ page }) => {
    await page.goto('/clear.html', { waitUntil: 'domcontentloaded' })
    await expect(page.locator('body')).toContainText('Done', { timeout: 30_000 })
    // 沙箱里 storage/caches/serviceWorker 全部不可访问，四项会一起失败。
    // 顶层打开时不允许出现 sandboxed 字样。
    await expect(page.locator('body')).not.toContainText('sandboxed')
  })

  test('被沙箱框起来时给出可执行提示，而不是抛四行看不懂的错', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await page.evaluate(() => {
      const frame = document.createElement('iframe')
      frame.setAttribute('sandbox', 'allow-scripts')
      frame.src = '/clear.html'
      frame.style.cssText = 'width:600px;height:700px;border:1px solid #ccc'
      frame.id = 'sandboxed-clear'
      document.body.append(frame)
    })

    const inner = page.frameLocator('#sandboxed-clear')
    const body = inner.locator('body')

    // 四行 cryptic 的 storage 报错换成一句能照做的话。
    await expect(body).not.toContainText("lacks the 'allow-same-origin' flag", { timeout: 20_000 })
    await expect(body).toContainText('clear.html', { timeout: 20_000 })
    // 必须把完整地址给出来，用户才能复制到标签页打开。
    await expect(body).toContainText('http')
  })

  test('应用内没有任何沙箱 iframe 指向清理页', async ({ page }) => {
    await mockApi(page, { theme: 'real' })
    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await expect(page.frameLocator(THEME_FRAME).locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })

    // 谁都不该把清理页框进沙箱——这正是它会四项全失败的原因。
    const sandboxed = await page.evaluate(() => {
      return Array.from(document.querySelectorAll('iframe[sandbox]'))
        .map(f => ({ sandbox: f.getAttribute('sandbox') || '', src: (f as HTMLIFrameElement).src }))
        .filter(f => f.src.includes('clear.html'))
    })
    expect(sandboxed).toEqual([])
  })
})

test.describe('E2E: 主题沙箱边界', () => {
  test('沙箱令牌必须让弹窗拿到正常 origin，并包含 allow-same-origin', async ({ page }) => {
    await mockApi(page, { theme: 'real' })
    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await expect(page.frameLocator(THEME_FRAME).locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })

    // 必须走 `locator(...).getAttribute(...)`：`page.getAttribute(selector, ...)` 会把
    // 字符串当成 selector 引擎名来解析，`[data-testid=...]` 于是匹配不到真实节点，
    // 取回错误的值——这正是本条最初假红的原因。
    const tokens = await page.locator(THEME_FRAME).getAttribute('sandbox')
    expect(tokens).toBeTruthy()
    // 没有这个 token，主题开的窗口导航之后 origin 仍是 `null`，书签打开后没有登录态。
    expect(tokens).toContain('allow-popups-to-escape-sandbox')
    expect(tokens).toContain('allow-popups')

    // `allow-same-origin` 是**必需的**，原因见 `sandbox.ts` 的说明：没有它，主题框是
    // opaque origin，收不到 DevTools 设备模拟用 `Input.emulateTouchFromMouseEvent`
    // 合成出来的「鼠标转触摸」输入，症状是「设备模拟下点了完全没反应、真机却正常」。
    //
    // 它的代价是真实的：主题从此与面板同源，能读面板的 localStorage（含 JWT）与
    // cookie，也能自行移除本 sandbox。主题由「被隔离」变为「完全受信」。这条断言因此
    // 同时钉住了「必须加」和「这条边界是弱的」两个事实，避免有人以为隔离仍在。
    expect(tokens).toContain('allow-same-origin')
  })

  test('主题开的窗口能正常使用 cookie 和 storage', async ({ page, context }) => {
    await mockApi(page, { theme: 'real' })

    // 先在顶层放一个值，代表面板自己的 storage。
    await page.goto('/', { waitUntil: 'domcontentloaded' })
    await page.evaluate(() => {
      localStorage.setItem('yin-e2e-probe', 'value-from-core')
      document.cookie = 'yin_probe=cookie-from-core; path=/'
    })

    // 令牌必须从**真实主题框**上读，不能在这里写死一份。写死的话这条测试就跟真实
    // 主题无关：把 sandbox.ts 里的 token 删掉它照样绿，是一条假绿。
    //
    // 必须等主题框真正挂上再读：早一步拿到 null，探针就会建成 `sandbox=""`（全禁），
    // 于是永远开不出窗。这正是单跑绿、全套跑红的原因。
    await expect(page.locator(THEME_FRAME)).toBeAttached({ timeout: 30_000 })
    const liveSandbox = await page.locator(THEME_FRAME).getAttribute('sandbox', { timeout: 30_000 })
    expect(liveSandbox, '主题框应已挂载并带上 sandbox 令牌').toBeTruthy()
    await page.evaluate((sandboxTokens) => {
      const probe = document.createElement('iframe')
      probe.setAttribute('sandbox', sandboxTokens || '')
      probe.srcdoc = '<button id="go" style="width:120px;height:60px">go</button><script>document.getElementById("go").addEventListener("click",function(){window.open("http://127.0.0.1:3002/login","yin-e2e-window")})<\/script>'
      probe.id = 'window-probe'
      // 主题框是 `position:absolute; inset:0`，会盖住任何后插入的探针并吞掉点击。
      // 所以固定到左上角并抬到顶层，否则 Playwright 只会报 "intercepts pointer events"。
      probe.style.cssText = 'position:fixed;left:0;top:0;width:120px;height:60px;z-index:2147483647;border:0'
      document.body.append(probe)
    }, liveSandbox)

    // popup 事件要挂在 `page` 上：触发它的是这个 iframe 所属的 Page，
    // `context.waitForEvent('popup')` 在这里会超时——实测 context.pages() 明明从 1 变 2。
    const popupPromise = page.waitForEvent('popup', { timeout: 15_000 })
    await page.frameLocator('#window-probe').locator('#go').click()
    const popup = await popupPromise
    await popup.waitForLoadState('domcontentloaded').catch(() => {})
    await popup.waitForTimeout(1500)

    // 关键断言：窗口有真实 origin，storage 可用。这一条在缺 token 时会红。
    const win = await popup.evaluate(() => ({
      origin: window.origin,
      storage: (() => { try { return String(localStorage.getItem('yin-e2e-probe')) } catch (e) { return `BLOCKED:${(e as Error).name}` } })(),
    }))
    expect(win.origin).not.toBe('null')
    expect(win.storage).not.toContain('BLOCKED')

    // 主题框反过来读这个窗口。`allow-same-origin` 之后主题与面板同源，而 popup 也在
    // 同一个源上，所以这里**读得到**——这条断言把「主题已完全受信」这个事实钉住，
    // 免得有人误以为隔离还在。这是为修好 DevTools 设备模拟输入而明确接受的代价，
    // 依据见 `sandbox.ts`。若将来引入第三方主题，这条必须重新变成 SecurityError。
    const leak = await page.frameLocator('#window-probe').locator('body').evaluate((body: Element) => {
      const w = body.ownerDocument.defaultView as unknown as { origin: string, open: (u: string, n: string) => Window | null }
      const handle = w.open('', 'yin-e2e-window')
      if (!handle) return { gotHandle: false, origin: '', storage: '' }
      return {
        gotHandle: 'yes',
        origin: String(w.origin),
        storage: String(handle.localStorage.getItem('yin-e2e-probe')),
      }
    })
    expect(leak.gotHandle).toBe('yes')
    // 探针与面板同源（这就是 allow-same-origin 的含义）。
    expect(leak.origin).not.toBe('null')
    expect(leak.storage, '同源后主题可读面板 storage —— 这是已接受的信任代价').toBe('value-from-core')

    await popup.close()
  })

  // 快照形状的断言并入上面「新窗口书签」那一条的第一段：那里直接读主题手上真实的
  // item，检查 `openMethod === 2` 且没有 `url`。这里原本还有一条独立测试，但它读的
  // `__lastSnapshot` 根本不存在，于是永远通过——那是一条假绿，不留下来。
})

test.describe('E2E: DevTools 设备模拟的输入路径', () => {
  // 这条专为一个真实缺陷而设：DevTools 设备模式把鼠标转成触摸
  // （`Input.emulateTouchFromMouseEvent`），而这个合成输入**送不进 opaque origin 的
  // iframe**。当时的表现是「设备模拟下点了完全没反应、真机却完全正常」，而所有用直接
  // 触摸派发的测试都是绿的 —— 因为它们走的是真机路径，永远碰不到这条合成路径。
  //
  // 所以它**必须**用 `emulateTouchFromMouseEvent`，不能用 `touchscreen.tap()`：后者是
  // 直接派发触摸事件，正是真机走的那条路，测不出这个问题。
  test('鼠标转触摸的合成输入必须能点开新窗口', async ({ page, context }) => {
    await mockApi(page, { theme: 'real' })
    await page.goto('/', { waitUntil: 'domcontentloaded' })

    const theme = page.frameLocator(THEME_FRAME)
    const item = theme.locator('.yin-item').first()
    await expect(item).toBeVisible({ timeout: 30_000 })

    await theme.locator('body').evaluate((body: Element) => {
      const win = body.ownerDocument.defaultView as unknown as Record<string, unknown>
      const events: string[] = []
      win.__ev = events
      for (const type of ['pointerdown', 'pointerup', 'click']) {
        body.ownerDocument.addEventListener(type, () => events.push(type), { capture: true })
      }
    })

    const box = await item.boundingBox()
    expect(box).toBeTruthy()
    const x = Math.round(box!.x + box!.width / 2)
    const y = Math.round(box!.y + box!.height / 2)

    const cdp = await context.newCDPSession(page)
    await cdp.send('Input.emulateTouchFromMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 })
    await cdp.send('Input.emulateTouchFromMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 })

    // 先钉住「事件到了主题框」——窗口没开时，这一条能区分「输入没进去」和「开窗被拦」。
    await expect.poll(async () => {
      const events = await theme.locator('body').evaluate((b: Element) => (b.ownerDocument.defaultView as unknown as Record<string, unknown>).__ev)
      return Array.isArray(events) ? events.includes('click') : false
    }, { timeout: 10_000 }).toBe(true)

    await expect.poll(async () => (await newWindowURLs(page)).some(u => u.includes('code.visualstudio.com')), { timeout: 15_000 })
      .toBe(true)
  })
})

test.describe('E2E: 编辑书签弹窗可关闭（真实服务）', () => {
  // 这一组刻意不走 mockApi：主题的右键菜单要成立，item 必须带 `item.update` 能力，
  // 而 `toThemeItem` 的 capabilities 是固定的 ['item.open']，mock 环境下菜单根本不会
  // 渲染。那是 fixture 的局限，不是产品的行为，所以这里连真实服务跑。
  //
  // 需要一个已登录的会话。用 `YIN_PANEL_E2E_MAIL` / `YIN_PANEL_E2E_PASSWORD` 提供凭据；
  // 没配就跳过，并且写明原因——绝不用一个空断言冒充通过。
  const mail = process.env.YIN_PANEL_E2E_MAIL
  const password = process.env.YIN_PANEL_E2E_PASSWORD

  test.beforeEach(async ({ page }) => {
    test.skip(!mail || !password, '需要 YIN_PANEL_E2E_MAIL / YIN_PANEL_E2E_PASSWORD 才能连真实服务')
    const response = await page.request.post('/api/login', { data: { mail, password } })
    const body = await response.json()
    const token = body?.data?.token
    test.skip(!token, `登录失败：${JSON.stringify(body).slice(0, 160)}`)
    await page.addInitScript((arg: string) => {
      try {
        localStorage.setItem('authStorage', JSON.stringify({
          data: { token: arg, userInfo: { id: 1, name: 'e2e', mail: 'e2e', avatar: '', role: 1, createTime: 0, updateTime: 0 } },
          expire: null,
        }))
      } catch { /* 主题框内没有 storage */ }
    }, token)
  })

  test('点关闭按钮和按 ESC 都能关掉，且关闭后页面恢复可点', async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
    const theme = page.frameLocator(THEME_FRAME)
    await expect(theme.locator('.yin-item').first()).toBeVisible({ timeout: 30_000 })

    await theme.locator('.yin-item').first().click({ button: 'right' })
    await expect(theme.locator('[data-testid="theme-edit-item"]')).toBeVisible({ timeout: 15_000 })
    await theme.locator('[data-testid="theme-edit-item"]').click()
    await expect(page.locator('.n-modal')).toHaveCount(1, { timeout: 15_000 })

    // 关闭按钮。注意 `n-base-close` **就是**那个 button 自己，不是它的子元素，所以
    // `button:has(.n-base-close)` 匹配不到，会把"选择器写错"误判成"关不掉"。
    await page.locator('.n-modal button.n-base-close').first().click()
    await expect(page.locator('.n-modal')).toHaveCount(0, { timeout: 10_000 })
    // 遮罩必须一起消失：它拦截所有指针事件，留在页面上等于整个面板卡死。
    await expect(page.locator('.n-modal-mask')).toHaveCount(0, { timeout: 10_000 })

    // 再开一次走 ESC。
    await theme.locator('.yin-item').first().click({ button: 'right' })
    await expect(theme.locator('[data-testid="theme-edit-item"]')).toBeVisible({ timeout: 15_000 })
    await theme.locator('[data-testid="theme-edit-item"]').click()
    await expect(page.locator('.n-modal')).toHaveCount(1, { timeout: 15_000 })
    await page.keyboard.press('Escape')
    await expect(page.locator('.n-modal')).toHaveCount(0, { timeout: 10_000 })

    // 页面必须恢复可点。
    let clickable = true
    try {
      await theme.locator('.yin-item').first().click({ timeout: 5_000 })
    } catch {
      clickable = false
    }
    expect(clickable, '弹窗关闭后主题框应重新可点').toBe(true)
  })
})