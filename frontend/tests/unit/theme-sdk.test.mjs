import assert from 'node:assert/strict'
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { isJsonRecord, isThemeApiRequest, isThemeEventName } from '../../packages/theme-sdk/src/index.ts'

test('Theme SDK accepts only bounded v1 API request envelopes', () => {
  const request = {
    protocol: 'yin-theme-api',
    version: 1,
    requestId: 'request-123',
    contextVersion: 3,
    method: 'commands.execute',
    payload: { command: 'space.select', payload: { spaceId: '12' } },
  }
  assert.equal(isThemeApiRequest(request), true)
  assert.equal(isThemeApiRequest({ ...request, method: 'items.list', payload: { limit: 50 } }), true)
  assert.equal(isThemeApiRequest({ ...request, contextVersion: -1 }), false)
  assert.equal(isThemeApiRequest({ ...request, method: 'fetch' }), false)
  assert.equal(isThemeApiRequest({ ...request, requestId: 'short' }), false)
  assert.equal(isThemeApiRequest({ ...request, payload: { value: 'x'.repeat(1_048_576) } }), false)
})

test('Theme SDK validates JSON records and stable event names', () => {
  assert.equal(isJsonRecord({ value: [1, true, null] }), true)
  assert.equal(isJsonRecord([]), false)
  assert.equal(isJsonRecord(null), false)
  assert.equal(isThemeEventName('space.changed'), true)
  assert.equal(isThemeEventName('internal.store.mutated'), false)
})
