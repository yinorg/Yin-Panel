import assert from 'node:assert/strict'
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { isJsonRecord, isThemeApiRequest, isThemeEventName } from '../../packages/theme-sdk/src/index.ts'
import { requiredCommandPermission } from '../../src/theme/api/permissions.ts'
import { validateDTCGDocument202510 } from '../../src/utils/theme.ts'
import { rewriteThemeStylesheet } from '../../src/theme/runtime/resources.ts'
import { createThemeSandboxDocument } from '../../src/theme/runtime/sandboxDocument.ts'
import { readFileSync } from 'node:fs'

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

test('every Theme API command has an explicit required capability', () => {
  const commands = [
    'space.select', 'space.toggleSide', 'item.open', 'item.create', 'item.update', 'item.delete',
    'items.reorder', 'group.create', 'group.update', 'group.delete', 'groups.reorder', 'search.submit',
    'data.refresh', 'editor.open', 'commandCenter.open', 'ui.openCoreSurface',
  ]
  for (const command of commands)
    assert.equal(typeof requiredCommandPermission(command), 'string', `missing permission for ${command}`)
  assert.equal(requiredCommandPermission('commandCenter.open'), 'items.read')
})

test('DTCG 2025.10 validator matches the pinned official format suite', () => {
  const fixture = JSON.parse(readFileSync(new URL('../../../shared/theme/dtcg-conformance/official-2025.10.json', import.meta.url), 'utf8'))
  assert.equal(fixture.cases.length, 220)
  for (const testCase of fixture.cases) {
    let valid = true
    try { validateDTCGDocument202510(testCase.document) }
    catch { valid = false }
    assert.equal(valid, testCase.valid, `${testCase.id} expected valid=${testCase.valid}`)
  }
})

test('Theme CSS rewrites only declared packaged media assets', async () => {
  const resources = [{
    path: 'assets/background.png',
    url: '/api/theme/v2/assets/revision/assets/background.png',
    mediaType: 'image/png',
  }, {
    path: 'assets/mark.svg',
    url: '/api/theme/v2/assets/revision/assets/mark.svg',
    mediaType: 'image/svg+xml',
  }]
  const css = '/* url(https://invalid.example/tracker) */ .panel { background-image: url("../assets/background.png"), url("../assets/mark.svg") }'
  const result = await rewriteThemeStylesheet(
    css,
    'https://yin.test/api/theme/v2/assets/revision/styles/home.css',
    resources,
    {
      'assets/background.png': 'blob:https://yin.test/asset-id',
      'assets/mark.svg': 'https://yin.test/api/theme/v2/assets/revision/assets/mark.svg',
    },
  )
  assert.match(result, /blob:https:\/\/yin\.test\/asset-id/)
  assert.match(result, /https:\/\/yin\.test\/api\/theme\/v2\/assets\/revision\/assets\/mark\.svg/)
  assert.doesNotMatch(result, /invalid\.example/)
})

test('Theme CSS rejects imports, undeclared URLs and image-set string fetches', async () => {
  const args = ['.panel { color: red }', 'https://yin.test/api/theme/v2/assets/revision/styles/home.css', [], {}]
  await assert.rejects(rewriteThemeStylesheet('@import url("/theme.css");', ...args.slice(1)), /@import/)
  await assert.rejects(rewriteThemeStylesheet('@im\\port url("/theme.css");', ...args.slice(1)), /escaped at-rule/)
  await assert.rejects(rewriteThemeStylesheet('.panel { background: url(https://outside.test/x.png) }', ...args.slice(1)), /undeclared asset/)
  await assert.rejects(rewriteThemeStylesheet('.panel { background: image-set("https://outside.test/x.png" 1x) }', ...args.slice(1)), /image-set/)
  await assert.rejects(rewriteThemeStylesheet('.panel { background: url("/private/path") }', ...args.slice(1)), /undeclared asset/)
})

test('Theme sandbox CSP blocks external styles, scripts, connections and base URL changes', () => {
  const document = createThemeSandboxDocument('https://yin.test', 'bootstrap()')
  assert.match(document, /connect-src 'none'/)
  assert.match(document, /style-src 'unsafe-inline' blob:/)
  assert.match(document, /script-src 'unsafe-inline' blob:/)
  assert.match(document, /frame-src 'none'/)
  assert.match(document, /base-uri 'none'/)
  assert.doesNotMatch(document, /style-src[^;]*yin\.test/)
})
