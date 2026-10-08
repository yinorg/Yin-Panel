import { defineConfig, devices } from '@playwright/test'

/**
 * End-to-end behaviour that the pixel suite cannot express: whether an action the
 * Core performs on the theme's behalf survives the browser's own gates.
 *
 * The single most important line here is `ignoreDefaultArgs`. Playwright launches
 * Chromium with `--disable-popup-blocking` (verified in `playwright-core`'s
 * `coreBundle.js`), which switches the popup blocker off entirely. Every assertion
 * written against that launch was vacuous: a popup the browser would refuse still
 * opened, so a suite full of them passed while the shipped app was broken on
 * mobile. Removing the flag is what makes these tests mean anything.
 *
 * `tests/visual/` keeps its own config; this one is for tests that assert on browser
 * policy rather than on pixels, and it must not be merged into the visual projects
 * or they would silently inherit the very flag this file removes.
 */
export default defineConfig({
  testDir: './tests/e2e',
  // The theme boots in a frame and then loads further async chunks, and a grant
  // response races that boot. The visual suite saw one flaky run in three when
  // parallel, so this one is serial too: correctness here depends on the browser's
  // transient activation, and a parallel run changes the timing that grants it.
  workers: 1,
  fullyParallel: false,
  timeout: 90_000,
  expect: { timeout: 15_000 },
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:3002',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
    launchOptions: {
      // The blocker stays on. Anything that relies on a popup being opened outside a
      // user gesture must now be refused, which is the whole point of this suite.
      ignoreDefaultArgs: ['--disable-popup-blocking'],
    },
    // The service worker's precache must not reach these tests.
    //
    // It outlives a deployment: the worker keeps serving the chunk it cached, so a
    // freshly deployed build renders as the old one and a correct change reads as
    // "no effect". That is not hypothetical — the sandbox-token change in
    // `ThemeHost.vue` was verified present in both the deployed bundle and the HTML
    // the server returns, and the page still rendered the previous value purely
    // because of this cache. It is also why a sandboxed document cannot clear its
    // own storage, which is what the `/clear.html` escape hatch exists for.
    //
    // `serviceWorkers: 'block'` keeps the worker from registering at all, so these
    // tests always assert on the deployed artifact. Offline behaviour has its own
    // suite under `tests/visual/`, which drives it deliberately.
    serviceWorkers: 'block',
  },
  projects: [
    {
      name: 'desktop',
      use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 } },
    },
    {
      // 手机 UA + 触屏，跑在 Chromium 上即可：这两个 bug 的判定条件由
      // `installMobilePopupGate` 按移动端策略模拟，不依赖某个引擎的原生行为，
      // 因此不必引入 WebKit，也不该把"某引擎恰好放行"当成通过。
      name: 'mobile',
      use: {
        ...devices['Pixel 7'],
        // Pixel 7 的描述符默认就是 Chromium；显式写出来是为了让意图明确，
        // 免得日后有人换成 iPhone 描述符而悄悄触发 WebKit。
        browserName: 'chromium',
      },
    },
  ],
  webServer: {
    command: 'echo "Using existing server"',
    port: 3002,
    reuseExistingServer: true,
    timeout: 10_000,
  },
})