import { expect, test } from '@playwright/test'
import { resolvePanelValue, resolveThemeSlots, selectThemeScheme, semanticSlots, type ThemePackage } from '../../src/utils/theme'

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
    colors.focusRing.$value = '{color.primary}'
    return [scheme, { color: colors }]
  }))
  return {
    manifest: {
      id: 'example.theme', name: 'Example', packageVersion: '1.0.0', schemes,
      documents: Object.fromEntries(schemes.map(scheme => [scheme, `tokens/${scheme}.json`])),
      bindings: Object.fromEntries(semanticSlots.map(slot => [slot, `/color/${slot}`])),
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
  pkg.documents.light.color.canvas.$value = '{color.unknown}'
  expect(() => resolveThemeSlots(pkg, 'light')).toThrow(/Unknown token/)

  pkg.documents.light.color.canvas.$value = '{color.surface}'
  pkg.documents.light.color.surface.$value = '{color.canvas}'
  expect(() => resolveThemeSlots(pkg, 'light')).toThrow(/cycle/i)
})

test('inherits DTCG types and rejects references that change token type', () => {
  const pkg = makePackage(['light'])
  expect(resolveThemeSlots(pkg, 'light').canvas).toBe('#ffffff')

  pkg.documents.light.typography = {
    $type: 'fontFamily',
    body: { $value: 'Inter' },
  }
  pkg.documents.light.color.focusRing.$value = '{typography.body}'
  expect(() => resolveThemeSlots(pkg, 'light')).toThrow(/type/i)
})

test('rejects missing schemes, non-color bindings, and unsupported color values', () => {
  const missing = makePackage(['light'])
  delete missing.documents.light
  expect(() => resolveThemeSlots(missing, 'light')).toThrow(/missing/i)

  const wrongType = makePackage(['light'])
  wrongType.documents.light.color.primary.$type = 'string'
  expect(() => resolveThemeSlots(wrongType, 'light')).toThrow(/not a color/i)

  const invalidColor = makePackage(['light'])
  invalidColor.documents.light.color.primary.$value = 'rgb(1, 2, 3)'
  expect(() => resolveThemeSlots(invalidColor, 'light')).toThrow(/unsupported color/i)
})

test('stored panel values remain overrides until theme defaults are adopted', () => {
  expect(resolvePanelValue('#ffffff', '#fa00aa', false)).toBe('#fa00aa')
  expect(resolvePanelValue('var(--yin-text)', '#fa00aa', true)).toBe('var(--yin-text)')
  expect(resolvePanelValue('#ffffff', undefined, false)).toBe('#ffffff')
})
