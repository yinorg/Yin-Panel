import { expect, test } from '@playwright/test'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { createThemeApiDispatcher, type ThemeApiHandlers } from '../../src/theme/api/dispatcher'
import { createThemeHomeSnapshot } from '../../src/core/home/themeSnapshot'
import { isThemeApiRequest } from '../../src/theme/api/v1'
import { THEME_API_VERSION as publicThemeApiVersion, isThemeApiRequest as isPublicThemeApiRequest } from '../../packages/theme-sdk/src/index'
import { createHomeThemeHandlers } from '../../src/core/home/themeHandlers'
import { createThemeSandboxDocument } from '../../src/theme/runtime/sandbox'
import type { ThemePermission } from '../../src/theme/api/v1'
import { normalizeThemePackageV2, resolvePanelValue, resolveThemeSlots, resolveWallpaper, selectThemeScheme, semanticSlots, validateDTCGDocument202510, type ThemePackage, type ThemePackageV2 } from '../../src/utils/theme'

const palettes: Record<string, Record<string, string>> = {
  light: { canvas: '#ffffff', surface: '#f3f6f8', surfaceElevated: '#ffffff', text: '#172126', textMuted: '#53636a', border: '#d5dfe2', primary: '#075b68', onPrimary: '#ffffff', secondary: '#8b4412', success: '#176b45', warning: '#805200', danger: '#a12627', focusRing: '#075b68' },
  dark: { canvas: '#171d20', surface: '#222a2e', surfaceElevated: '#2b353a', text: '#f1f5f6', textMuted: '#b0bec3', border: '#536168', primary: '#72d6df', onPrimary: '#102326', secondary: '#f0a66d', success: '#71d8a0', warning: '#f2c46c', danger: '#ff9792', focusRing: '#72d6df' },
}

function makePackage(schemes = ['light', 'dark']): ThemePackage {
  const documents = Object.fromEntries(schemes.map((scheme) => {
    const colors: Record<string, any> = { $type: 'color' }
    for (const slot of semanticSlots)
      colors[slot] = { $value: palettes[scheme][slot] }
    const warning = palettes[scheme].warning
    colors.warning.$value = {
      colorSpace: 'srgb',
      components: [parseInt(warning.slice(1, 3), 16) / 255, parseInt(warning.slice(3, 5), 16) / 255, parseInt(warning.slice(5, 7), 16) / 255],
      alpha: 1,
    }
    colors.focusRing.$value = '{semantic.color.primary}'
    return [scheme, {
      $schema: 'https://www.designtokens.org/schemas/2025.10/format.json',
      primitive: { color: colors },
      semantic: { color: structuredClone(colors) },
    }]
  }))
  return {
    manifest: {
      id: 'example.theme', name: 'Example', packageVersion: '1.0.0', schemes,
      documents: Object.fromEntries(schemes.map(scheme => [scheme, `tokens/${scheme}.json`])),
    },
    documents,
    verified: true,
  }
}

test('resolves bound tokens and DTCG references for the selected scheme', () => {
  const pkg = makePackage()
  expect(resolveThemeSlots(pkg, 'dark')).toMatchObject({
    canvas: '#171d20',
    text: '#f1f5f6',
    focusRing: '#72d6df',
    warning: '#f2c46c',
  })
})

test('single-scheme packages stay on their only scheme', () => {
  expect(resolveThemeSlots(makePackage(['light']), 'dark').canvas).toBe('#ffffff')
  expect(selectThemeScheme(['light'], 'dark', 'dark')).toBe('light')
})

test('auto mode follows the operating-system scheme', () => {
  expect(selectThemeScheme(['light', 'dark'], 'auto', 'dark')).toBe('dark')
  expect(selectThemeScheme(['light', 'dark'], 'auto', 'light')).toBe('light')
  expect(selectThemeScheme(['light', 'dark'], 'dark', 'light')).toBe('dark')
})

test('rejects missing token references and cycles', () => {
  const pkg = makePackage(['light'])
  pkg.documents.light.semantic.color.canvas.$value = '{semantic.color.unknown}'
  expect(() => resolveThemeSlots(pkg, 'light')).toThrow(/Unknown token/)

  pkg.documents.light.semantic.color.canvas.$value = '{semantic.color.surface}'
  pkg.documents.light.semantic.color.surface.$value = '{semantic.color.canvas}'
  expect(() => resolveThemeSlots(pkg, 'light')).toThrow(/cycle/i)
})

test('inherits DTCG types and rejects references that change token type', () => {
  const pkg = makePackage(['light'])
  expect(resolveThemeSlots(pkg, 'light').canvas).toBe('#ffffff')

  pkg.documents.light.semantic.typography = {
    $type: 'fontFamily',
    body: { $value: 'Inter' },
  }
  pkg.documents.light.semantic.color.focusRing.$value = '{semantic.typography.body}'
  expect(() => resolveThemeSlots(pkg, 'light')).toThrow(/type/i)
})

test('rejects missing schemes, invalid semantic token types, and unsupported color values', () => {
  const missing = makePackage(['light'])
  delete missing.documents.light
  expect(() => resolveThemeSlots(missing, 'light')).toThrow(/missing/i)

  const wrongType = makePackage(['light'])
  wrongType.documents.light.semantic.color.primary.$type = 'string'
  wrongType.documents.light.semantic.color.primary.$value = 'invalid'
  expect(() => resolveThemeSlots(wrongType, 'light')).toThrow(/type/i)

  const invalidColor = makePackage(['light'])
  invalidColor.documents.light.semantic.color.primary.$value = 'rgb(1, 2, 3)'
  expect(() => resolveThemeSlots(invalidColor, 'light')).toThrow(/unsupported color/i)
})

test('stored panel values remain overrides until theme defaults are adopted', () => {
  expect(resolvePanelValue('#ffffff', '#fa00aa', false)).toBe('#fa00aa')
  expect(resolvePanelValue('var(--yin-text)', '#fa00aa', true)).toBe('var(--yin-text)')
  expect(resolvePanelValue('#ffffff', undefined, false)).toBe('#ffffff')
})

test('resolves grouped DTCG semantic and component tokens without controlling layout', () => {
  const pkg = makePackage(['light'])
  const palette = pkg.documents.light.semantic.color
  palette.focusRing.$value = '{primitive.color.primary}'
  pkg.documents.light = {
    $schema: 'https://www.designtokens.org/schemas/2025.10/format.json',
    primitive: { color: pkg.documents.light.primitive.color },
    semantic: { color: palette, typography: {
      body: { $type: 'fontFamily', $value: ['Inter', 'system-ui'] },
    } },
    component: {
      card: {
        $type: 'dimension',
        padding: { $value: { value: 22, unit: 'px' } },
        radius: { $value: { value: 18, unit: 'px' } },
        surfaceMode: { $type: 'string', $value: 'glass' },
      },
      searchBox: {
        $type: 'dimension',
        optionSize: { $value: { value: 44, unit: 'px' } },
        optionGap: { $value: { value: 12, unit: 'px' } },
      },
    },
    background: { texture: { $type: 'string', $value: 'grid' } },
  }

  const slots = resolveThemeSlots(pkg, 'light')
  expect(slots).toMatchObject({
    canvas: '#ffffff',
    'color-primary': '#075b68',
    fontBody: 'Inter, system-ui',
    'component-card-padding': '22px',
    'component-card-radius': '18px',
    'component-card-surface-mode': 'glass',
    'component-search-box-option-size': '44px',
    'component-search-box-option-gap': '12px',
    'background-texture': 'grid',
  })
  expect(slots.layoutTemplate).toBeUndefined()
})

test('resolves DTCG group root tokens and inherited tokens with local overrides', () => {
  const pkg = makePackage(['light'])
  pkg.documents.light = {
    $schema: 'https://www.designtokens.org/schemas/2025.10/format.json',
    primitive: {
      shape: {
        $type: 'dimension',
        $root: { $value: { value: 4, unit: 'px' } },
        small: { $value: { value: 8, unit: 'px' } },
        radius: { $value: { value: 16, unit: 'px' } },
      },
    },
    semantic: {
      shape: {
        $extends: '{primitive.shape}',
        radius: { $value: { value: 12, unit: 'px' } },
      },
    },
  }

  expect(resolveThemeSlots(pkg, 'light')).toMatchObject({
    'shape': '4px',
    'shape-small': '8px',
    'shape-radius': '12px',
  })
})

test('maps DTCG 2025.10 wide-gamut, polar, and alpha colors to browser-supported CSS Color 4', async ({ page }) => {
  const document = JSON.parse(readFileSync(resolve(process.cwd(), '../shared/theme/dtcg-conformance/color-spaces.json'), 'utf8'))
  const pkg = makePackage(['light'])
  pkg.documents.light = document

  const slots = resolveThemeSlots(pkg, 'light')
  expect(slots).toMatchObject({
    'primitive-color-p3-accent': 'color(display-p3 1 0.2 0 / 0.75)',
    'primitive-color-hsl-accent': 'hsl(210 60% 40%)',
  })
  expect(await page.evaluate(values => values.map(value => CSS.supports('color', value)), [slots['primitive-color-p3-accent'], slots['primitive-color-hsl-accent']])).toEqual([true, true])
})

test('matches the shared DTCG 2025.10 document acceptance fixture', () => {
  const fixture = JSON.parse(readFileSync(resolve(process.cwd(), '../shared/theme/dtcg-conformance/documents.json'), 'utf8'))
  for (const entry of fixture.cases) {
    const pkg = makePackage(['light'])
    pkg.documents.light = entry.document
    if (entry.valid) {
      expect(() => validateDTCGDocument202510(entry.document), entry.name).not.toThrow()
      expect(() => resolveThemeSlots(pkg, 'light'), entry.name).not.toThrow()
    }
    else expect(() => validateDTCGDocument202510(entry.document), entry.name).toThrow()
  }
})

test('current package DTCG semantic tokens map to runtime slots', () => {
  const color = (hex: string) => ({
    $type: 'color',
    $value: {
      colorSpace: 'srgb',
      components: [1, 3, 5].map(index => Number.parseInt(hex.slice(index, index + 2), 16) / 255),
      alpha: 1,
    },
  })
  const colors = {
    canvas: color('#ffffff'), surface: color('#f5f5f5'), text: color('#202020'), muted: color('#666666'),
    border: color('#dddddd'), primary: color('#245b72'), onPrimary: color('#ffffff'), secondary: color('#526b5d'),
    success: color('#28734d'), warning: color('#805500'), danger: color('#a63338'), focusRing: color('#245b72'),
  }
  const pkg: ThemePackageV2 = {
    manifest: {
      id: 'org.yin.default', name: 'Yin', version: '2.0.0', defaultScheme: 'light',
      themeApi: '^1.0.0', core: '>=0.4.0',
      entrypoints: { script: 'scripts/theme.mjs', styles: ['styles/theme.css'] },
      runtime: { supportedModes: ['sandbox'] },
      contributes: { views: ['home'] },
      permissions: { required: [{ name: 'spaces.read' }, { name: 'groups.read' }, { name: 'items.read' }] },
      tokens: { format: 'DTCG', version: '2025.10', documents: { light: 'tokens/light.json', dark: 'tokens/dark.json' } },
      resources: [],
    },
    tokens: {
      light: { $schema: 'https://www.designtokens.org/schemas/2025.10/format.json', semantic: { ...colors, shape: { cardRadius: { $type: 'dimension', $value: { value: 8, unit: 'px' } } } } },
      dark: { $schema: 'https://www.designtokens.org/schemas/2025.10/format.json', semantic: colors },
    },
    revision: 'revision', verified: true,
  }
  const normalized = normalizeThemePackageV2(pkg)
  expect(normalized.manifest.schemes).toEqual(['light', 'dark'])
  expect(normalized.manifest).toMatchObject({
    themeApi: '^1.0.0',
    entrypoints: { script: 'scripts/theme.mjs' },
    runtime: { supportedModes: ['sandbox'] },
    contributes: { views: ['home'] },
    permissions: { required: [{ name: 'spaces.read' }, { name: 'groups.read' }, { name: 'items.read' }] },
  })
  expect(resolveThemeSlots(normalized, 'light')).toMatchObject({ canvas: '#ffffff', surface: '#f5f5f5', radiusCard: '8px' })
})

test('optional component tokens fall back to Core defaults when absent', () => {
  const pkg = makePackage(['light'])
  expect(resolveThemeSlots(pkg, 'light')).toMatchObject({ canvas: '#ffffff', text: '#172126' })
  expect(resolveThemeSlots(pkg, 'light')['component-search-box-option-size']).toBeUndefined()
})

test('wallpaper uses the selected scheme and versioned assets', () => {
  const pkg = makePackage()
  pkg.manifest.wallpapers = {
    light: { kind: 'image', source: 'wallpaper/light.webp' },
    dark: { kind: 'externalUrl', source: 'https://example.test/live', poster: 'wallpaper/poster.png' },
  }
  pkg.manifest.resources = [
    { path: 'wallpaper/light.webp', sha256: '', mediaType: 'image/webp', url: '/assets/light-v1.webp' },
    { path: 'wallpaper/poster.png', sha256: '', mediaType: 'image/png', url: '/assets/poster-v2.png' },
  ]
  expect(resolveWallpaper(pkg, 'light')).toMatchObject({ kind: 'image', source: '/assets/light-v1.webp' })
  expect(resolveWallpaper(pkg, 'dark')).toEqual({ kind: 'image', source: '/assets/poster-v2.png', poster: '/assets/poster-v2.png', overlayOpacity: undefined })
})

test('Theme API v1 validates protocol envelopes and bounds payload size', () => {
  const valid = { protocol: 'yin-theme-api', version: 1, requestId: 'request-123', contextVersion: 4, method: 'commands.execute', payload: { command: 'item.open' } }
  expect(publicThemeApiVersion).toBe('1.0.0')
  expect(isPublicThemeApiRequest(valid)).toBe(true)
  expect(isThemeApiRequest(valid)).toBe(true)
  expect(isThemeApiRequest({ ...valid, version: 2 })).toBe(false)
  expect(isThemeApiRequest({ ...valid, payload: { value: 'x'.repeat(1_048_577) } })).toBe(false)
})

test('theme sandbox CSP permits only packaged runtime resources and blocks network access', () => {
  const document = createThemeSandboxDocument('https://panel.example.test')
  expect(document).toContain("script-src 'unsafe-inline' blob:")
  expect(document).toContain("connect-src 'none'")
  expect(document).toContain('img-src data: blob: https://panel.example.test')
  expect(document).not.toContain('allow-same-origin')
  expect(() => createThemeSandboxDocument('javascript:alert(1)')).toThrow(/unsupported/i)
})

test('SVG theme assets cannot execute scripts after navigation inside the sandbox frame', async ({ page }) => {
  await page.route('**/active-theme.svg', route => route.fulfill({
    status: 200,
    contentType: 'image/svg+xml',
    headers: {
      'X-Content-Type-Options': 'nosniff',
      'Content-Security-Policy': "sandbox; default-src 'none'; script-src 'none'; style-src 'unsafe-inline'; img-src data: blob:; object-src 'none'; form-action 'none'; base-uri 'none'",
    },
    body: '<svg xmlns="http://www.w3.org/2000/svg"><script>parent.postMessage("svg-script-ran", "*")</script><text>Icon</text></svg>',
  }))
  await page.goto('/')
  await page.evaluate(() => {
    ;(window as Window & { __svgScriptMessages?: number }).__svgScriptMessages = 0
    window.addEventListener('message', (event) => {
      if (event.data === 'svg-script-ran') {
        const state = window as Window & { __svgScriptMessages?: number }
        state.__svgScriptMessages = (state.__svgScriptMessages || 0) + 1
      }
    })
    const frame = document.createElement('iframe')
    frame.setAttribute('sandbox', 'allow-scripts')
    frame.src = '/active-theme.svg'
    document.body.append(frame)
  })
  await page.waitForTimeout(100)
  expect(await page.evaluate(() => (window as Window & { __svgScriptMessages?: number }).__svgScriptMessages)).toBe(0)
})

test('Theme home DTO uses string IDs and excludes Core-only item URLs', () => {
  const sourceGroup = {
    id: 21,
    title: 'Tools',
    items: [{ id: 31, title: 'Console', url: 'https://internal.test', lanUrl: 'http://lan.test', mobileUrl: 'https://mobile.test', openMethod: 3, icon: { itemType: 1, text: 'C', backgroundColor: '#123456' } }],
  }
  const snapshot = createThemeHomeSnapshot({
    version: 8,
    status: 'ready',
    spaces: [{ id: 12, name: 'Primary', side: 'yin', pairedSpaceId: 13 }],
    activeSpaceId: 12,
    groups: [sourceGroup],
    canWrite: false,
  })
  expect(snapshot).toMatchObject({
    version: 8,
    status: 'ready',
    activeSpaceId: '12',
    spaces: [{ id: '12', pairedSpaceId: '13', capabilities: ['space.select'] }],
    groups: [{ id: '21', spaceId: '12', itemIds: ['31'] }],
    items: [{ id: '31', groupId: '21', capabilities: ['item.open'] }],
  })
  expect(snapshot.items[0]).not.toHaveProperty('url')
  expect(snapshot.items[0]).not.toHaveProperty('openMethod')
})

test('Core Home command adapter resolves IDs from live data and ignores theme-provided URLs', async () => {
  const item = { id: 31, itemIconGroupId: 21, title: 'Console', icon: null, url: 'https://internal.test', openMethod: 3 }
  let opened: Panel.ItemInfo | undefined
  let deletedItemId = 0
  let requestedMode = ''
  let reportedBottom = -1
  const forwardedKeys: string[] = []
  const openedLinks: string[] = []
  const handlers = createHomeThemeHandlers({
    getSpaces: () => [{ id: 12 }],
    getGroups: () => [{ id: 21, items: [item] }],
    getActiveSpaceId: () => 12,
    selectSpace: () => undefined,
    openItem: (value) => { opened = value },
    openEditor: () => undefined,
    createItem: () => undefined,
    updateItem: () => undefined,
    deleteItem: (value) => { deletedItemId = Number(value.id) },
    reorderItems: () => undefined,
    createGroup: () => undefined,
    updateGroup: () => undefined,
    deleteGroup: () => undefined,
    reorderGroups: () => undefined,
    openCommandCenter: () => undefined,
    toggleSide: () => undefined,
    refresh: () => undefined,
    searchItems: () => [item],
    getMonitorSnapshot: () => ({}),
    submitSearch: () => undefined,
    navigate: () => undefined,
    getSettings: () => ({}),
    patchSettings: () => undefined,
    getStorage: () => undefined,
    setStorage: () => undefined,
    removeStorage: () => undefined,
    setNetworkMode: (mode) => { requestedMode = mode },
    reportLayout: ({ searchBottom }) => { reportedBottom = searchBottom },
    forwardKey: (key) => { forwardedKeys.push(key) },
    openLink: (url) => { openedLinks.push(url) },
  })
  await handlers.executeCommand('item.open', { itemId: '31', url: 'https://attacker.test' })
  expect(opened).toBe(item)
  await expect(handlers.searchItems('Console', { limit: 1 })).resolves.toMatchObject({
    query: 'Console',
    total: 1,
    items: [{ id: '31', groupId: '21', title: 'Console', capabilities: ['item.open'] }],
  })
  expect(handlers.listSpaces({ limit: 1 })).toMatchObject({
    total: 1,
    items: [{ id: '12', capabilities: ['space.select'] }],
  })
  expect(handlers.listItems({ limit: 1 })).toMatchObject({
    total: 1,
    items: [{ id: '31', groupId: '21', title: 'Console', capabilities: ['item.open'] }],
  })
  await expect(handlers.executeCommand('item.open', { itemId: '999' })).rejects.toMatchObject({ code: 'NOT_FOUND' })
  await handlers.executeCommand('item.delete', { itemId: '31', url: 'https://attacker.test' })
  expect(deletedItemId).toBe(31)
  // Both commands added for the theme runtime must stay wired through the adapter.
  await handlers.executeCommand('network.setMode', { mode: 'lan' })
  expect(requestedMode).toBe('lan')
  await handlers.executeCommand('layout.report', { searchBottom: 328 })
  expect(reportedBottom).toBe(328)
  // The sandbox cannot deliver key events to the Core, so the theme's shortcut
  // only works while this binding stays wired through the adapter.
  await handlers.executeCommand('input.forwardKey', { key: 'a' })
  expect(forwardedKeys).toEqual(['a'])
  await expect(handlers.executeCommand('input.forwardKey', { key: 'Enter' }))
    .rejects.toMatchObject({ code: 'INVALID_ARGUMENT' })
  // The sandbox cannot open windows, so links come back to the Core; anything
  // that is not a plain web link is refused before it reaches window.open.
  await handlers.executeCommand('link.open', { url: 'https://github.com/yinorg/Yin-Panel' })
  expect(openedLinks).toEqual(['https://github.com/yinorg/Yin-Panel'])
  for (const url of ['javascript:alert(1)', 'data:text/html,x', '/relative', 'not a url']) {
    await expect(handlers.executeCommand('link.open', { url })).rejects.toMatchObject({ code: 'INVALID_ARGUMENT' })
  }
  expect(openedLinks).toHaveLength(1)
})

test('Theme API v1 dispatcher enforces permission and rejects stale contexts', async () => {
  let contextVersion = 7
  let openedItem = ''
  const handlers: ThemeApiHandlers = {
    executeCommand: (_command, payload) => { openedItem = String(payload.itemId); return { opened: openedItem } },
    searchItems: (query, paging) => ({ query, items: [{ id: query }], total: 1, paging }),
    listSpaces: paging => ({ items: [], total: 0, ...paging }),
    listGroups: paging => ({ items: [], total: 0, ...paging }),
    listItems: paging => ({ items: [], total: 0, ...paging }),
    getMonitorSnapshot: () => ({ capturedAt: '2026-09-24T00:00:00.000Z' }),
    navigate: () => undefined,
    getSettings: () => ({}),
    patchSettings: () => undefined,
    getStorage: () => undefined,
    setStorage: () => undefined,
    removeStorage: () => undefined,
  }
  const dispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['items.read']), handlers })
  const base = { protocol: 'yin-theme-api', version: 1, requestId: 'request-123', contextVersion: 7, method: 'commands.execute', payload: { command: 'item.open', arguments: { itemId: '42' } } }

  const opened = await dispatch(base)
  expect(opened).toMatchObject({ ok: true, result: { opened: '42' } })
  expect(openedItem).toBe('42')

  const search = await dispatch({ ...base, method: 'search.query', payload: { query: 'needle' } })
  expect(search).toMatchObject({ ok: true, result: { query: 'needle', items: [{ id: 'needle' }], total: 1, paging: { limit: 50 } } })
  const invalidSearch = await dispatch({ ...base, method: 'search.query', payload: { query: 'x'.repeat(201) } })
  expect(invalidSearch).toMatchObject({ ok: false, error: { code: 'INVALID_ARGUMENT' } })
  const maxPage = await dispatch({ ...base, method: 'search.query', payload: { query: 'needle', limit: 200 } })
  expect(maxPage).toMatchObject({ ok: true, result: { paging: { limit: 200 } } })
  const oversizedPage = await dispatch({ ...base, method: 'search.query', payload: { query: 'needle', limit: 201 } })
  expect(oversizedPage).toMatchObject({ ok: false, error: { code: 'INVALID_ARGUMENT' } })
  const itemListDeniedDispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['spaces.read']), handlers })
  const itemListDenied = await itemListDeniedDispatch({ ...base, method: 'items.list', payload: { limit: 1 } })
  expect(itemListDenied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })
  const itemListDispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['items.read']), handlers })
  const itemList = await itemListDispatch({ ...base, method: 'items.list', payload: { limit: 1 } })
  expect(itemList).toMatchObject({ ok: true, result: { items: [], total: 0 } })
  const spaceListDispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['spaces.read']), handlers })
  const spaceList = await spaceListDispatch({ ...base, method: 'spaces.list', payload: { limit: 25 } })
  expect(spaceList).toMatchObject({ ok: true, result: { items: [], total: 0, limit: 25 } })
  const maxSpaceList = await spaceListDispatch({ ...base, method: 'spaces.list', payload: { limit: 200 } })
  expect(maxSpaceList).toMatchObject({ ok: true, result: { items: [], total: 0, limit: 200 } })
  const oversizedSpaceList = await spaceListDispatch({ ...base, method: 'spaces.list', payload: { limit: 201 } })
  expect(oversizedSpaceList).toMatchObject({ ok: false, error: { code: 'INVALID_ARGUMENT' } })
  const groupsDenied = await itemListDispatch({ ...base, method: 'groups.list', payload: undefined })
  expect(groupsDenied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })
  const invalidGroupFilter = await itemListDispatch({ ...base, method: 'items.list', payload: { groupId: '../other-space' } })
  expect(invalidGroupFilter).toMatchObject({ ok: false, error: { code: 'INVALID_ARGUMENT' } })

  const denied = await dispatch({ ...base, payload: { command: 'item.delete', arguments: { itemId: '42' } } })
  expect(denied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })

  const settingsDenied = await dispatch({ ...base, method: 'settings.get', payload: undefined })
  expect(settingsDenied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })

  contextVersion += 1
  const stale = await dispatch(base)
  expect(stale).toMatchObject({ ok: false, error: { code: 'ABORTED' } })
  expect(openedItem).toBe('42')

  contextVersion = 8
  const contextChangingDispatch = createThemeApiDispatcher({
    getContextVersion: () => contextVersion,
    getPermissions: () => new Set(['items.read', 'groups.read']),
    handlers: {
      ...handlers,
      executeCommand: async () => {
        await Promise.resolve()
        contextVersion += 1
        return { refreshed: true }
      },
    },
  })
  const completedAction = await contextChangingDispatch({ ...base, contextVersion, payload: { command: 'data.refresh' } })
  expect(completedAction).toMatchObject({ ok: true, result: { refreshed: true } })

  const preferencesDispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['preferences.read']), handlers })
  const settingsAllowed = await preferencesDispatch({ ...base, contextVersion, method: 'settings.get', payload: undefined })
  expect(settingsAllowed).toMatchObject({ ok: true, result: {} })
  const settingsWriteDenied = await preferencesDispatch({ ...base, contextVersion, method: 'settings.patch', payload: { value: { layout: 'grid' } } })
  expect(settingsWriteDenied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })
  const preferencesWriteDispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['preferences.write']), handlers })
  const settingsWriteAllowed = await preferencesWriteDispatch({ ...base, contextVersion, method: 'settings.patch', payload: { value: { layout: 'grid' } } })
  expect(settingsWriteAllowed).toMatchObject({ ok: true })

  const monitorDenied = await dispatch({ ...base, contextVersion, method: 'monitor.getSnapshot', payload: undefined })
  expect(monitorDenied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })
  const monitorDispatch = createThemeApiDispatcher({ getContextVersion: () => contextVersion, getPermissions: () => new Set(['monitor.read']), handlers })
  const monitorAllowed = await monitorDispatch({ ...base, contextVersion, method: 'monitor.getSnapshot', payload: undefined })
  expect(monitorAllowed).toMatchObject({ ok: true, result: { capturedAt: '2026-09-24T00:00:00.000Z' } })

  const boundedDispatch = createThemeApiDispatcher({
    getContextVersion: () => contextVersion,
    getPermissions: () => new Set(['items.read']),
    handlers: { ...handlers, listItems: () => new Promise(() => {}) },
    maxInFlight: 1,
    timeoutMs: 5,
  })
  const timedOut = await boundedDispatch({ ...base, contextVersion, method: 'items.list', payload: undefined })
  expect(timedOut).toMatchObject({ ok: false, error: { code: 'TIMEOUT' } })
  const stillInFlight = await boundedDispatch({ ...base, requestId: 'request-124', contextVersion, method: 'items.list', payload: undefined })
  expect(stillInFlight).toMatchObject({ ok: false, error: { code: 'RATE_LIMITED' } })

  let finishLateRead: ((value: unknown) => void) | undefined
  let currentPermissions = new Set<ThemePermission>(['items.read'])
  const lateReadDispatch = createThemeApiDispatcher({
    getContextVersion: () => contextVersion,
    getPermissions: () => currentPermissions,
    handlers: { ...handlers, listItems: () => new Promise(resolve => { finishLateRead = resolve }) },
  })
  const lateRead = lateReadDispatch({ ...base, contextVersion, method: 'items.list', payload: undefined })
  contextVersion += 1
  finishLateRead?.({ items: [{ id: 'from-old-space' }] })
  expect(await lateRead).toMatchObject({ ok: false, error: { code: 'ABORTED' } })

  const latePermissionDispatch = createThemeApiDispatcher({
    getContextVersion: () => contextVersion,
    getPermissions: () => currentPermissions,
    handlers: { ...handlers, listItems: () => new Promise(resolve => { finishLateRead = resolve }) },
  })
  const latePermissionRead = latePermissionDispatch({ ...base, contextVersion, method: 'items.list', payload: undefined })
  currentPermissions = new Set()
  finishLateRead?.({ items: [{ id: 'after-revocation' }] })
  expect(await latePermissionRead).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })
})

test('sandbox Theme API forwards search through its isolated MessageChannel runtime', async ({ page }) => {
  await page.goto('/')
  await page.evaluate(async () => {
    const sandboxPath = '/src/theme/runtime/sandbox.ts'
    const { mountThemeSandbox } = await import(sandboxPath)
    const frame = document.createElement('iframe')
    frame.setAttribute('data-theme-sandbox-under-test', '')
    document.body.appendChild(frame)
    const script = `export default {
      apiVersion: '1.0.0',
      setup(api) {
        return { views: { home(root) {
          root.textContent = 'sandbox ready'
          Promise.all([
            api.search.query('needle', { limit: 1 }),
            api.spaces.list({ limit: 25 }),
            api.groups.list(),
            api.items.list(),
          ]).then(results => {
            root.setAttribute('data-result', results.map(result => result.method + ':' + result.total).join('|'))
          }).catch(error => root.setAttribute('data-error', error.code))
          return { unmount() { root.replaceChildren() } }
        } } }
      }
    }`
    const runtime = await mountThemeSandbox(frame, {
      script,
      styles: [],
      tokens: '',
      snapshot: { version: 1, status: 'ready', spaces: [], groups: [], items: [] },
      environment: {
        coreVersion: '0.4.0', language: 'en', colorScheme: 'light', reducedMotion: false,
        online: true, viewport: { width: 1280, height: 800 }, assets: {},
      },
      permissions: new Set(['items.read', 'spaces.read', 'groups.read']),
      execute: (request: unknown) => {
        const method = (request as { method: string }).method
        return { method, items: [], total: method === 'spaces.list' ? 3 : 0 }
      },
      onError: (error: Error) => { document.documentElement.dataset.themeError = error.message },
    })
    ;(window as Window & { __testThemeRuntime?: { dispose: () => Promise<void> } }).__testThemeRuntime = runtime
  })
  // The home page now hosts the real theme frame, so a bare `iframe` selector
  // matches more than one element. The wallpaper and window frames are also
  // plain iframes with no distinguishing test id, so the sandbox under test is
  // marked at creation time and selected by that instead.
  const themedRoot = page.frameLocator('iframe[data-theme-sandbox-under-test]').locator('#theme-root')
  await expect(themedRoot).toHaveAttribute('data-result', 'search.query:0|spaces.list:3|groups.list:0|items.list:0')
  await page.evaluate(async () => {
    await (window as Window & { __testThemeRuntime?: { dispose: () => Promise<void> } }).__testThemeRuntime?.dispose()
  })
})

test('trusted Theme runtime uses the shared API and cleans up its view and host styles', async ({ page }) => {
  await page.goto('/')
  const result = await page.evaluate(async () => {
    const runtimePath = '/src/theme/runtime/direct.ts'
    const { mountThemeDirect } = await import(/* @vite-ignore */ runtimePath)
    const host = document.createElement('div')
    document.body.appendChild(host)
    const runtime = await mountThemeDirect({
      host,
      script: `export default {
        apiVersion: '1.0.0',
        setup(api) {
          window.__trustedThemeLifecycle = { unmounted: 0, disposed: 0 }
          return {
            views: { home: async (root, api) => {
              root.textContent = 'trusted ready'
              root.style.color = getComputedStyle(root).getPropertyValue('--yin-text').trim()
              const search = await api.search.query('trusted')
              root.setAttribute('data-search-total', String(search.total))
              return { unmount() { window.__trustedThemeLifecycle.unmounted++; root.replaceChildren() } }
            } },
            dispose() { window.__trustedThemeLifecycle.disposed++ },
          }
        },
      }`,
      styles: [{ text: '#theme-root { color: var(--yin-text); }' }],
      tokens: ':root { --yin-text: rgb(12, 34, 56); }',
      snapshot: { version: 1, status: 'ready', spaces: [], groups: [], items: [] },
      environment: {
        coreVersion: '0.4.0', language: 'en', colorScheme: 'light', reducedMotion: false,
        online: true, viewport: { width: 1280, height: 800 }, assets: {},
      },
      permissions: new Set(['items.read']),
      execute: (request: unknown) => ({ total: (request as { method: string }).method === 'search.query' ? 2 : 0 }),
      onError: (error: Error) => { document.documentElement.dataset.trustedThemeError = error.message },
    })
    const shadow = host.shadowRoot!
    runtime.updateTokens(':root { --yin-text: rgb(12, 34, 56); }')
    const themedRoot = shadow.querySelector('#theme-root')!
    const color = getComputedStyle(themedRoot).color
    const text = themedRoot.textContent
    const searchTotal = themedRoot.getAttribute('data-search-total')
    await runtime.dispose()
    const lifecycle = (window as Window & { __trustedThemeLifecycle?: { unmounted: number; disposed: number } }).__trustedThemeLifecycle
    host.remove()
    return { color, text, searchTotal, cleared: shadow.childElementCount === 0, lifecycle }
  })
  expect(result).toEqual({
    color: 'rgb(12, 34, 56)',
    text: 'trusted ready',
    searchTotal: '2',
    cleared: true,
    lifecycle: { unmounted: 1, disposed: 1 },
  })
  const failedMount = await page.evaluate(async () => {
    const runtimePath = '/src/theme/runtime/direct.ts'
    const { mountThemeDirect } = await import(/* @vite-ignore */ runtimePath)
    const host = document.createElement('div')
    document.body.appendChild(host)
    let message = ''
    try {
      await mountThemeDirect({
        host,
        script: `export default { apiVersion: '1.0.0', setup() { throw new Error('setup failed') } }`,
        styles: [], tokens: '',
        snapshot: { version: 1, status: 'ready', spaces: [], groups: [], items: [] },
        environment: {
          coreVersion: '0.4.0', language: 'en', colorScheme: 'light', reducedMotion: false,
          online: true, viewport: { width: 1280, height: 800 }, assets: {},
        },
        permissions: new Set(), execute: async () => ({}), onError: () => undefined,
      })
    }
    catch (error) { message = error instanceof Error ? error.message : String(error) }
    const empty = host.shadowRoot?.childElementCount === 0
    host.remove()
    return { message, empty }
  })
  expect(failedMount).toEqual({ message: 'setup failed', empty: true })
})

test('theme recovery bypasses active package loading and restores Yin without changing color mode', async ({ page }) => {
  let currentThemeRequests = 0
  let mineRequests = 0
  let savedPreference: Record<string, string> | undefined
  await page.route('**/api/theme/v2/current', route => {
    currentThemeRequests++
    return route.fulfill({ json: { code: 0, data: {} } })
  })
  await page.route('**/api/theme/v2/mine', route => {
    mineRequests++
    return route.fulfill({ status: 500, json: { code: -1 } })
  })
  await page.route('**/api/theme/v2/preference', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({ json: { code: 0, data: { mode: 'dark' } } })
      return
    }
    savedPreference = route.request().postDataJSON()
    await route.fulfill({ json: { code: 0 } })
  })

  await page.goto('/__yin/theme-recovery')
  await expect(page.getByTestId('theme-recovery')).toBeVisible()
  expect(currentThemeRequests).toBe(0)
  expect(mineRequests).toBe(0)
  await page.getByTestId('theme-recovery-restore-yin').click()
  await expect.poll(() => savedPreference).toEqual({ packageId: 'org.yin.default', mode: 'dark' })
  await expect(page).toHaveURL(/\/$/)
  expect(await page.evaluate(() => sessionStorage.getItem('yin-theme-safe-mode'))).toBe('1')
})
