import assert from 'node:assert/strict'
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { disableThemeSafeMode, enableThemeSafeMode, isThemeSafeMode, THEME_SAFE_MODE_KEY } from '../../src/theme/recovery/safeMode.ts'
import { createThemeRequestGuard } from '../../src/theme/runtime/requestGuard.ts'

test('theme safe mode is scoped to session storage and can be cleared', () => {
  const values = new Map()
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: key => values.delete(key),
  }

  assert.equal(isThemeSafeMode(storage), false)
  enableThemeSafeMode(storage)
  assert.equal(values.get(THEME_SAFE_MODE_KEY), '1')
  assert.equal(isThemeSafeMode(storage), true)
  disableThemeSafeMode(storage)
  assert.equal(isThemeSafeMode(storage), false)
})

test('theme request IDs are single-use and request history is bounded', () => {
  const guard = createThemeRequestGuard(2)
  assert.equal(guard.remember('request-1'), true)
  assert.equal(guard.remember('request-1'), false)
  assert.equal(guard.remember('request-2'), true)
  assert.throws(() => guard.remember('request-3'), /history limit/i)
})
