import assert from 'node:assert/strict'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { DARK_LUMA, suggestedForeground, toneForLuma } from '../../src/core/home/wallpaperTone.ts'

// The Core reads the wallpaper and hands the theme a tone plus a suggested
// foreground. The theme decides what to draw with it, so the mapping itself is
// the contract worth pinning: get it backwards and every theme draws dark text
// on dark artwork.
test('average luminance maps to the wallpaper tone', () => {
  assert.equal(toneForLuma(0), 'dark')
  assert.equal(toneForLuma(DARK_LUMA - 1), 'dark')
  assert.equal(toneForLuma(DARK_LUMA), 'light')
  assert.equal(toneForLuma(255), 'light')
})

test('the suggested foreground contrasts with the wallpaper', () => {
  assert.equal(suggestedForeground('dark').onWallpaper, '#ffffff')
  assert.equal(suggestedForeground('light').onWallpaper, '#101418')
  assert.match(suggestedForeground('dark').onWallpaperMuted, /^rgb\(/)
  assert.match(suggestedForeground('light').onWallpaperMuted, /^rgb\(/)
})
