import assert from 'node:assert/strict'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { createThemeHomeSnapshot } from '../../src/core/home/themeSnapshot.ts'

// The theme receives the brand text through the presentation snapshot. The
// pre-theme default is not a user choice, so it must follow the active panel
// side; before this the value was hardcoded and switching to Yang-Panel left
// the logo reading "Yin-Panel".
function snapshotFor(side, logoText) {
  return createThemeHomeSnapshot({
    version: 1,
    status: 'ready',
    spaces: [{ id: 1, name: 'Sample', side: 'yin' }],
    activeSpaceId: side === 'yang' ? 2 : 1,
    activeSpaceSide: side,
    groups: [],
    canWrite: true,
    presentation: { logoText },
  })
}

test('the default brand follows the active panel side', () => {
  assert.equal(snapshotFor('yin', 'Yin-Panel').presentation.logoText, 'Yin-Panel')
  assert.equal(snapshotFor('yang', 'Yin-Panel').presentation.logoText, 'Yang-Panel')
})

test('a missing logo text still resolves per side', () => {
  assert.equal(snapshotFor('yin', undefined).presentation.logoText, 'Yin-Panel')
  assert.equal(snapshotFor('yang', undefined).presentation.logoText, 'Yang-Panel')
})

test('a customised logo text is passed through untouched', () => {
  assert.equal(snapshotFor('yin', 'My Panel').presentation.logoText, 'My Panel')
  assert.equal(snapshotFor('yang', 'My Panel').presentation.logoText, 'My Panel')
})
