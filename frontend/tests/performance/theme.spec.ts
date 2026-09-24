import { expect, test } from '@playwright/test'
import { createThemeApiDispatcher, type ThemeApiHandlers } from '../../src/theme/api/dispatcher'
import { createThemeHomeSnapshot } from '../../src/theme/api/homeSnapshot'
import { isThemeApiRequest } from '../../src/theme/api/v1'
import { createHomeThemeHandlers } from '../../src/core/home/themeHandlers'
import { createThemeSandboxDocument } from '../../src/theme/runtime/sandbox'
import { normalizeThemePackageV2, resolvePanelValue, resolveThemeSlots, resolveWallpaper, selectThemeScheme, semanticSlots, type ThemePackage, type ThemePackageV2 } from '../../src/utils/theme'

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
      $schema: 'https://design-tokens.github.io/community-group/format/2025.10/schema.json',
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
    $schema: 'https://design-tokens.github.io/community-group/format/2025.10/schema.json',
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
      light: { $schema: 'https://design-tokens.github.io/community-group/format/2025.10/schema.json', semantic: { ...colors, shape: { cardRadius: { $type: 'dimension', $value: { value: 8, unit: 'px' } } } } },
      dark: { $schema: 'https://design-tokens.github.io/community-group/format/2025.10/schema.json', semantic: colors },
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
  const item = { id: 31, title: 'Console', icon: null, url: 'https://internal.test', openMethod: 3 }
  let opened: Panel.ItemInfo | undefined
  const handlers = createHomeThemeHandlers({
    getSpaces: () => [{ id: 12 }],
    getGroups: () => [{ id: 21, items: [item] }],
    selectSpace: () => undefined,
    openItem: (value) => { opened = value },
    openEditor: () => undefined,
    openCommandCenter: () => undefined,
    toggleSide: () => undefined,
    refresh: () => undefined,
    submitSearch: () => undefined,
    navigate: () => undefined,
    getSettings: () => ({}),
    patchSettings: () => undefined,
    getStorage: () => undefined,
    setStorage: () => undefined,
    removeStorage: () => undefined,
  })
  await handlers.executeCommand('item.open', { itemId: '31', url: 'https://attacker.test' })
  expect(opened).toBe(item)
  await expect(handlers.executeCommand('item.open', { itemId: '999' })).rejects.toMatchObject({ code: 'NOT_FOUND' })
  await expect(handlers.executeCommand('item.delete', { itemId: '31' })).rejects.toMatchObject({ code: 'UNSUPPORTED_CAPABILITY' })
})

test('Theme API v1 dispatcher enforces permission and rejects stale contexts', async () => {
  let contextVersion = 7
  let openedItem = ''
  const handlers: ThemeApiHandlers = {
    executeCommand: (_command, payload) => { openedItem = String(payload.itemId); return { opened: openedItem } },
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

  const denied = await dispatch({ ...base, payload: { command: 'item.delete', arguments: { itemId: '42' } } })
  expect(denied).toMatchObject({ ok: false, error: { code: 'PERMISSION_DENIED' } })

  contextVersion += 1
  const stale = await dispatch(base)
  expect(stale).toMatchObject({ ok: false, error: { code: 'ABORTED' } })
  expect(openedItem).toBe('42')
})
