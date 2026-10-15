/**
 * The wallpaper the home paints is a precedence decision, so it is tested as
 * one: no DOM, no browser, just the resolver.
 */
import assert from 'node:assert/strict'
import { test } from 'node:test'

const { resolveWallpaper } = await import('../../src/core/home/wallpaperResolve.ts')

const FOREST = '/assets/bg-forest.webp'
const LIBRARY = {
  'dynamic-002': { kind: 'image', source: '/api/theme/v2/wallpaper/library/assets/1/effects/002.png' },
  'dynamic-003': { kind: 'page', source: '/api/theme/v2/wallpaper/library/assets/1/effects/003.html' },
}
const findLibraryEntry = id => (id ? LIBRARY[id] : undefined)

function resolve(overrides = {}) {
  return resolveWallpaper({
    panelConfig: {},
    spaceWallpaper: null,
    findLibraryEntry,
    themeWallpaper: null,
    ...overrides,
  })
}

test('a Space default outranks the visitor’s own global wallpaper', () => {
  // The reported bug: the visitor had picked a wallpaper of their own, so the
  // Space’s default never showed.
  const choice = resolve({
    panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-003' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-003'].source)
})

test('a Space default outranks an explicit “no wallpaper”', () => {
  const choice = resolve({
    panelConfig: { wallpaperMode: 'none' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-002' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('a Space that follows the theme leaves the visitor’s choice standing', () => {
  const choice = resolve({
    panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' },
    spaceWallpaper: { mode: 'theme' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('with no Space default the visitor’s own wallpaper applies', () => {
  const choice = resolve({ panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' } })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('an explicit “no wallpaper” still paints nothing', () => {
  assert.equal(resolve({ panelConfig: { wallpaperMode: 'none' } }), null)
})

test('a Space entry the library no longer carries falls through', () => {
  const choice = resolve({
    panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'gone' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('a Space custom source is used as given', () => {
  const choice = resolve({ spaceWallpaper: { mode: 'custom', source: '/assets/bg-forest.webp?from=space' } })
  assert.equal(choice.source, '/assets/bg-forest.webp?from=space')
  assert.equal(choice.poster, '/assets/bg-forest.webp?from=space')
})

test('?themePreview ignores both the Space default and the visitor’s choice', () => {
  const choice = resolve({
    previewThemeDefaults: true,
    panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-003' },
    themeWallpaper: { kind: 'image', source: '/theme/own.png', poster: '/theme/own.png', overlayOpacity: 0 },
  })
  assert.equal(choice.source, '/theme/own.png')
})

test('nothing configured falls back to the packaged artwork', () => {
  assert.equal(resolve().source, FOREST)
})
