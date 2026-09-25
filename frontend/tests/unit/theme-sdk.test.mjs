import assert from 'node:assert/strict'
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { isJsonRecord, isThemeApiRequest, isThemeEventName } from '../../packages/theme-sdk/src/index.ts'
import { createThemeApiClient } from '../../packages/theme-sdk/src/index.ts'
import { requiredCommandPermission } from '../../src/theme/api/permissions.ts'
import { createThemeApiDirectTransport } from '../../src/theme/api/directTransport.ts'
import { validateDTCGDocument202510 } from '../../src/utils/theme.ts'
import { rewriteThemeStylesheet } from '../../src/theme/runtime/resources.ts'
import { createThemeSandboxDocument } from '../../src/theme/runtime/sandboxDocument.ts'
import { readFileSync } from 'node:fs'
import { MessageChannel } from 'node:worker_threads'

test('Theme API client preserves one contract over direct and MessageChannel transports', async () => {
  for (const transportKind of ['direct', 'message-channel']) {
    let contextVersion = 1
    const calls = []
    const dispatcher = async request => {
      const response = { protocol: 'yin-theme-api', version: 1, requestId: request.requestId, contextVersion: request.contextVersion, ok: true }
      calls.push([request.method, request.payload])
      if (request.contextVersion !== contextVersion)
        return { ...response, ok: false, error: { code: 'ABORTED', message: 'Theme context is no longer current' } }
      if (request.method === 'groups.list')
        return { ...response, ok: false, error: { code: 'PERMISSION_DENIED', message: 'Theme permission required: groups.read' } }
      if (request.method === 'spaces.list') {
        return { ...response, result: { items: [], total: 0, nextCursor: '10' } }
      }
      if (request.method === 'commands.execute') return { ...response, result: { command: request.payload.command, arguments: request.payload.arguments } }
      if (request.method === 'search.query') return { ...response, result: { query: request.payload.query, items: [], total: 0 } }
      if (request.method === 'items.list') return { ...response, result: { items: [], total: 0, groupId: request.payload.groupId } }
      if (request.method === 'monitor.getSnapshot') return { ...response, result: { capturedAt: '2026-09-25T00:00:00Z' } }
      if (request.method === 'navigation.navigate') return { ...response, result: request.payload }
      if (request.method === 'settings.get') return { ...response, result: { compact: true } }
      if (request.method === 'settings.patch') return { ...response, result: request.payload.value }
      if (request.method === 'storage.get') return { ...response, result: request.payload.key }
      if (request.method === 'storage.set') return { ...response, result: { key: request.payload.key, value: request.payload.value } }
      if (request.method === 'storage.remove') return { ...response, result: request.payload.key }
      if (request.method === 'ui.openCoreSurface') return { ...response, result: request.payload }
      if (request.method === 'network.fetch') return { ...response, result: { status: 200, headers: {}, body: '{}' } }
      if (request.method === 'diagnostics.report') return { ...response, result: undefined }
      return { ...response, ok: false, error: { code: 'INVALID_ARGUMENT', message: 'Unknown operation' } }
    }
    let request
    let close
    if (transportKind === 'direct') {
      request = createThemeApiDirectTransport(dispatcher)
      close = () => {}
    }
    else {
      const { port1, port2 } = new MessageChannel()
      const pending = new Map()
      port1.on('message', response => {
        const operation = pending.get(response.requestId)
        if (!operation) return
        pending.delete(response.requestId)
        if (response.ok) operation.resolve(response.result)
        else operation.reject(Object.assign(new Error(response.error.message), { code: response.error.code }))
      })
      port2.on('message', async envelope => port2.postMessage(await dispatcher(envelope)))
      request = envelope => new Promise((resolve, reject) => {
        pending.set(envelope.requestId, { resolve, reject })
        port1.postMessage(envelope)
      })
      close = () => { port1.close(); port2.close() }
    }

    let snapshot = { version: 1, status: 'ready', spaces: [], groups: [], items: [] }
    const client = createThemeApiClient({
      request,
      getSnapshot: () => snapshot,
      getEnvironment: () => ({ apiVersion: '1.0.0', coreVersion: '0.4.0', language: 'en', colorScheme: 'light', reducedMotion: false, online: true, viewport: { width: 1280, height: 800 }, assets: { 'background.png': '/assets/background.png' } }),
    })
    const exposedSnapshot = client.api.state.getSnapshot()
    exposedSnapshot.status = 'error'
    assert.equal(client.api.state.getSnapshot().status, 'ready', `${transportKind} snapshots must be copied`)
    assert.deepEqual(await client.api.spaces.list(), { items: [], total: 0, nextCursor: '10' })
    assert.deepEqual(calls.find(([method]) => method === 'spaces.list'), ['spaces.list', {}])
    assert.equal(client.api.assets.resolve('background.png'), '/assets/background.png')
    await assert.rejects(client.api.groups.list(), error => error.code === 'PERMISSION_DENIED')
    assert.deepEqual(await client.api.commands.execute('space.select', { spaceId: '7' }), { command: 'space.select', arguments: { spaceId: '7' } })
    assert.deepEqual(await client.api.search.query('notes', { limit: 20 }), { query: 'notes', items: [], total: 0 })
    assert.deepEqual(await client.api.items.list({ groupId: '9' }), { items: [], total: 0, groupId: '9' })
    assert.equal((await client.api.monitor.getSnapshot()).capturedAt, '2026-09-25T00:00:00Z')
    assert.deepEqual(client.api.navigation.get(), { view: 'home', spaceId: undefined })
    await client.api.navigation.navigate({ view: 'theme-page', spaceId: '7' })
    assert.deepEqual(await client.api.settings.get(), { compact: true })
    await client.api.settings.patch({ compact: false })
    assert.equal(await client.api.storage.get('active-tab'), 'active-tab')
    await client.api.storage.set('active-tab', 2)
    await client.api.storage.remove('active-tab')
    await client.api.ui.openCoreSurface('theme-settings', { tab: 'tokens' })
    assert.equal((await client.api.network.fetch('/api/theme/v2/current')).status, 200)
    await client.api.diagnostics.report({ level: 'info', message: 'test' })

    snapshot = { ...snapshot, version: 2, status: 'ready', activeSpaceId: '7' }
    contextVersion = 2
    client.updateSnapshot(snapshot)
    let received
    client.api.events.subscribe('space.changed', event => { received = event })
    assert.equal(client.emit('space.changed', { contextVersion: 1, sequence: 1, payload: {} }), false)
    assert.equal(client.emit('space.changed', { contextVersion: 2, sequence: 1, payload: { activeSpaceId: '7' } }), true)
    assert.equal(client.emit('space.changed', { contextVersion: 2, sequence: 1, payload: {} }), false)
    assert.equal(received.payload.activeSpaceId, '7')
    assert.deepEqual(client.api.navigation.get(), { view: 'home', spaceId: '7' })
    assert.deepEqual(await client.api.spaces.list({ limit: 12 }), { items: [], total: 0, nextCursor: '10' })
    assert.deepEqual(calls.findLast(([method]) => method === 'spaces.list'), ['spaces.list', { limit: 12 }])

    client.dispose()
    close()
  }
})

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

test('Theme CSS accepts same-origin relative preview resource URLs', async () => {
  const result = await rewriteThemeStylesheet(
    '.panel { background: url("../assets/background.png") }',
    '/api/theme/v2/preview/token/assets/styles/home.css',
    [{ path: 'assets/background.png', url: '/api/theme/v2/preview/token/assets/assets/background.png', mediaType: 'image/png' }],
    { 'assets/background.png': 'blob:https://yin.test/asset-id' },
    'https://yin.test',
  )
  assert.match(result, /blob:https:\/\/yin\.test\/asset-id/)
})

test('Trusted Theme CSS maps document roots to the Shadow DOM host', async () => {
  const result = await rewriteThemeStylesheet(
    ':root, html, body { color: red } .panel { color: blue }',
    'https://yin.test/api/theme/v2/assets/revision/styles/home.css',
    [],
    {},
    undefined,
    true,
  )
  assert.match(result, /:host/)
  assert.doesNotMatch(result, /(^|[,\s])html([,\s{]|$)|(^|[,\s])body([,\s{]|$)|:root/)
  assert.match(result, /\.panel/)
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
