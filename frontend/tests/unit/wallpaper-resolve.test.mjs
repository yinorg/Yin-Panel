/**
 * The wallpaper the home paints is a precedence decision, so it is tested as
 * one: no DOM, no browser, just the resolver.
 *
 * The order is: the visitor's own choice first — a Space's default is a default,
 * not an override — then the Space's default, then the theme's own artwork.
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

test('the visitor’s own wallpaper outranks the Space default', () => {
  // The reported bug: a Space default displaced the wallpaper the visitor had
  // just picked, so switching it did nothing and only refreshes ever showed it.
  const choice = resolve({
    panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-003' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('an explicit “no wallpaper” outranks the Space default', () => {
  // Turning wallpapers off is a choice too: the Space must not re-enable them.
  assert.equal(resolve({
    panelConfig: { wallpaperMode: 'none' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-002' },
  }), null)
})

test('a Space default applies while the visitor follows the theme', () => {
  const choice = resolve({
    panelConfig: { wallpaperMode: 'theme' },
    spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-002' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('a Space default applies when the visitor has no wallpaper setting', () => {
  const choice = resolve({ spaceWallpaper: { mode: 'library', libraryEntry: 'dynamic-002' } })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('with no Space default the visitor’s own wallpaper applies', () => {
  const choice = resolve({ panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' } })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('a Space that follows the theme leaves the visitor’s choice standing', () => {
  const choice = resolve({
    panelConfig: { wallpaperMode: 'library', wallpaperLibraryEntry: 'dynamic-002' },
    spaceWallpaper: { mode: 'theme' },
  })
  assert.equal(choice.source, LIBRARY['dynamic-002'].source)
})

test('an explicit “no wallpaper” still paints nothing', () => {
  assert.equal(resolve({ panelConfig: { wallpaperMode: 'none' } }), null)
})

test('a Space entry the library no longer carries falls through', () => {
  const choice = resolve({
    spaceWallpaper: { mode: 'library', libraryEntry: 'gone' },
    themeWallpaper: { kind: 'image', source: '/theme/own.png', poster: '/theme/own.png', overlayOpacity: 0 },
  })
  assert.equal(choice.source, '/theme/own.png')
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
