import assert from 'node:assert/strict'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { normalizeIconBackground, readableTextColor } from '../../src/utils/color.ts'

test('a pale tile gets dark text and a dark tile gets light text', () => {
  assert.equal(readableTextColor('#ffffff'), '#101317')
  assert.equal(readableTextColor('#d0d0d0'), '#101317')
  assert.equal(readableTextColor('#e3e3e3'), '#101317')
  assert.equal(readableTextColor('#111111'), '#ffffff')
  assert.equal(readableTextColor('#18A058'), '#101317')
  assert.equal(readableTextColor('rgba(211, 47, 47, 1)'), '#ffffff')
  assert.equal(readableTextColor('#fff'), '#101317')
  assert.equal(readableTextColor('#000000ff'), '#ffffff')
})

test('translucent or unparseable colours keep the theme default', () => {
  assert.equal(readableTextColor('#2a2a2a6b'), undefined)
  assert.equal(readableTextColor('rgba(0, 0, 0, 0.5)'), undefined)
  assert.equal(readableTextColor('rgb(255, 255, 255, 50%)'), undefined)
  assert.equal(readableTextColor('red'), undefined)
  assert.equal(readableTextColor(''), undefined)
  assert.equal(readableTextColor(undefined), undefined)
  assert.equal(readableTextColor(null), undefined)
})

test('a translucent tile is flattened over the surface it is painted on', () => {
  // The text icons default to #2a2a2a6b, which renders pale over a light panel.
  assert.equal(readableTextColor('#2a2a2a6b', '#ffffff'), '#101317')
  assert.equal(readableTextColor('#2a2a2a6b', '#0f1319'), '#ffffff')
  assert.equal(readableTextColor('rgba(0, 0, 0, 0.5)', '#ffffff'), '#101317')
  assert.equal(readableTextColor('#2a2a2a6b', 'transparent'), undefined)
})

test('the editor legacy default tile counts as no colour', () => {
  assert.equal(normalizeIconBackground('#2a2a2a6b'), undefined)
  assert.equal(normalizeIconBackground('#2A2A2A6B'), undefined)
  assert.equal(normalizeIconBackground('  '), undefined)
  assert.equal(normalizeIconBackground(undefined), undefined)
  assert.equal(normalizeIconBackground('#ffffff'), '#ffffff')
})
