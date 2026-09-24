import assert from 'node:assert/strict'
import { setTimeout as delay } from 'node:timers/promises'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { createHomeCollectionLoader } from '../../src/core/home/collection.ts'
import { createHomeSearchService } from '../../src/core/home/search.ts'
import { createHomeThemeHandlers } from '../../src/core/home/themeHandlers.ts'
import { executeHomeThemeRequest } from '../../src/core/home/themeRequest.ts'
import { createThemeSettingsStore } from '../../src/core/home/themeSettingsStore.ts'
import { resolveItemOpenUrl } from '../../src/core/items/openPolicy.ts'
import { createSnapshotPoller } from '../../src/core/monitor/snapshotPoller.ts'
import { loadDiskSnapshots } from '../../src/core/monitor/diskSnapshots.ts'
import { normalizeMonitorSnapshot } from '../../src/core/monitor/themeSnapshot.ts'

test('home collection loads groups with bounded concurrency and retains partial failure state', async () => {
  let active = 0
  let peak = 0
  let saved
  const loader = createHomeCollectionLoader({
    getGroups: async () => ({ data: [{ id: 1 }, { id: 2 }, { id: 3 }] }),
    getItems: async (_spaceId, groupId, page, pageSize) => {
      assert.equal(page, 1)
      assert.equal(pageSize, 100)
      active++
      peak = Math.max(peak, active)
      await delay(5)
      active--
      if (groupId === 2) throw new Error('group failed')
      return { data: [{ id: groupId * 10 }] }
    },
    readCache: () => null,
    writeCache: (_spaceId, cache) => { saved = cache },
    isOnline: () => true,
  })

  const result = await loader.load(7)
  assert.equal(peak, 2)
  assert.equal(result.failedGroups, 1)
  assert.equal(result.stale, true)
  assert.equal(result.error.code, 'PARTIAL_LOAD')
  assert.deepEqual([...result.itemsByGroup.keys()], [1, 3])
  assert.equal(saved.items['1'][0].id, 10)
})

test('cancelled home collection cannot publish or save a stale Space response', async () => {
  let finishGroups
  let writes = 0
  let progress = 0
  const loader = createHomeCollectionLoader({
    getGroups: () => new Promise(resolve => { finishGroups = resolve }),
    getItems: async () => ({ data: [] }),
    readCache: () => null,
    writeCache: () => { writes++ },
    isOnline: () => true,
    onProgress: () => { progress++ },
  })

  const pending = loader.load(1)
  loader.cancel()
  finishGroups({ data: [{ id: 1 }] })
  const result = await pending
  assert.equal(result.cancelled, true)
  assert.equal(writes, 0)
  assert.equal(progress, 0)
})

test('home collection follows item pages and deduplicates records across page boundaries', async () => {
  const requestedPages = []
  const loader = createHomeCollectionLoader({
    getGroups: async () => ({ data: [{ id: 4 }] }),
    getItems: async (_spaceId, _groupId, page, pageSize) => {
      requestedPages.push(page)
      assert.equal(pageSize, 2)
      if (page === 1) return { data: [{ id: 1 }, { id: 2 }] }
      if (page === 2) return { data: [{ id: 2 }, { id: 3 }] }
      return { data: [{ id: 4 }] }
    },
    readCache: () => null,
    writeCache: () => {},
    isOnline: () => true,
    pageSize: 2,
  })

  const result = await loader.load(8)
  assert.deepEqual(requestedPages, [1, 2, 3])
  assert.deepEqual(result.itemsByGroup.get(4).map(item => item.id), [1, 2, 3, 4])
})

test('home search walks pages, deduplicates IDs and caches the complete Space result', async () => {
  const requests = []
  const pages = {
    1: [{ id: 1, title: 'first' }, { id: 2, title: 'second' }],
    2: [{ id: 2, title: 'duplicate' }, { id: 3, title: 'third' }],
    3: [{ id: 4, title: 'needle' }],
  }
  const search = createHomeSearchService({
    pageSize: 2,
    fetchPage: async (_spaceId, page, _pageSize, signal) => {
      assert.equal(signal.aborted, false)
      requests.push(page)
      return { data: pages[page] || [] }
    },
    isOnline: () => true,
  })

  const found = await search.query(9, 'needle')
  assert.deepEqual(found.map(item => item.id), [4])
  assert.deepEqual(requests, [1, 2, 3])
  const cached = await search.query(9, 'first')
  assert.deepEqual(cached.map(item => item.id), [1])
  assert.deepEqual(requests, [1, 2, 3])
})

test('ThemeHost Core bridge routes paginated search requests through the Theme API handler', async () => {
  let activeSpaceId = 10
  let themeSettings = {}
  let groupsForSpace = [{ id: 5, parentId: null, title: 'Bookmarks', items: [
    { id: 1, itemIconGroupId: 5, title: 'Theme search result', sort: 1, url: 'https://core-only.example' },
    { id: 2, itemIconGroupId: 5, title: 'Theme search result 2', sort: 2, url: 'https://core-only.example/2' },
  ] }]
  const handlers = createHomeThemeHandlers({
    getSpaces: () => [{ id: 10, name: 'Work', side: 'yin', pairedSpaceId: 11 }],
    getGroups: () => groupsForSpace,
    getActiveSpaceId: () => activeSpaceId,
    selectSpace: () => {},
    openItem: () => {},
    openEditor: () => {},
    openCommandCenter: () => {},
    toggleSide: () => {},
    refresh: () => {},
    searchItems: async () => [
      { id: 1, itemIconGroupId: 5, title: 'Theme search result', sort: 1 },
      { id: 2, itemIconGroupId: 5, title: 'Theme search result 2', sort: 2 },
    ],
    getMonitorSnapshot: () => ({ cpu: { usagePercent: 27 } }),
    submitSearch: () => {},
    navigate: () => {},
    getSettings: () => themeSettings,
    patchSettings: value => { themeSettings = { ...themeSettings, ...value } },
    getStorage: () => undefined,
    setStorage: () => {},
    removeStorage: () => {},
  })

  const page = await executeHomeThemeRequest(handlers, {
    method: 'search.query',
    query: 'Theme',
    limit: 1,
  })
  assert.equal(page.total, 2)
  assert.equal(page.items[0].id, '1')
  assert.equal(page.nextCursor, '1')
  const monitor = await executeHomeThemeRequest(handlers, { method: 'monitor.getSnapshot' })
  assert.deepEqual(monitor, { cpu: { usagePercent: 27 } })
  const spaces = await executeHomeThemeRequest(handlers, { method: 'spaces.list', limit: 1 })
  assert.equal(spaces.total, 1)
  assert.equal(spaces.items[0].pairedSpaceId, '11')
  const groups = await executeHomeThemeRequest(handlers, { method: 'groups.list' })
  assert.deepEqual(groups.items[0], { id: '5', spaceId: '10', parentId: undefined, title: 'Bookmarks', icon: undefined, itemIds: ['1', '2'] })
  const itemPage = await executeHomeThemeRequest(handlers, { method: 'items.list', groupId: '5', limit: 1 })
  assert.equal(itemPage.total, 2)
  assert.equal(itemPage.items[0].id, '1')
  assert.equal(itemPage.items[0].url, undefined)
  assert.equal(itemPage.nextCursor, '1')
  activeSpaceId = 20
  groupsForSpace = [{ id: 8, parentId: null, title: 'Other Space', items: [] }]
  const switchedSpaceGroups = await executeHomeThemeRequest(handlers, { method: 'groups.list' })
  assert.equal(switchedSpaceGroups.items[0].spaceId, '20')
  assert.equal(switchedSpaceGroups.items[0].id, '8')
  await executeHomeThemeRequest(handlers, { method: 'settings.patch', value: { density: 'compact' } })
  assert.deepEqual(await executeHomeThemeRequest(handlers, { method: 'settings.get' }), { density: 'compact' })
  await assert.rejects(
    executeHomeThemeRequest(handlers, { method: 'search.query', query: 'Theme', limit: 101 }),
    error => error.code === 'INVALID_ARGUMENT',
  )
})

test('theme settings are revision scoped and oversized patches leave stored data unchanged', () => {
  const values = new Map()
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
  }
  const settings = createThemeSettingsStore(storage, 'user-1:theme-a:revision-1', 32)
  settings.patch({ density: 'compact' })
  assert.deepEqual(settings.get(), { density: 'compact' })
  assert.throws(() => settings.patch({ copy: 'x'.repeat(64) }), error => error.code === 'INVALID_ARGUMENT')
  assert.deepEqual(settings.get(), { density: 'compact' })
  assert.deepEqual(createThemeSettingsStore(storage, 'user-1:theme-a:revision-2').get(), {})
  assert.deepEqual(createThemeSettingsStore(storage, 'user-2:theme-a:revision-1').get(), {})
})

test('monitor DTO maps the actual backend snapshot fields without exposing unrelated data', () => {
  const result = normalizeMonitorSnapshot({
    CPU_INFO: { coreCount: 8, cpuNum: 16, model: 'CPU', usages: [42.5] },
    MEMORY_INFO: { total: 1000, used: 400, free: 600, usedPercent: 40 },
    NETWORK_INFO: [{ name: 'eth0', bytesSent: 90, bytesRecv: 110 }],
  }, '2026-09-24T00:00:00.000Z')

  assert.deepEqual(result, {
    capturedAt: '2026-09-24T00:00:00.000Z',
    cpu: { coreCount: 8, model: 'CPU', usagePercent: 42.5 },
    memory: { total: 1000, used: 400, free: 600, usedPercent: 40 },
    network: [{ name: 'eth0', bytesSent: 90, bytesRecv: 110 }],
  })
})

test('invalidating a Space aborts its full-search request with ABORTED', async () => {
  const search = createHomeSearchService({
    fetchPage: (_spaceId, _page, _pageSize, signal) => new Promise((_resolve, reject) => {
      signal.addEventListener('abort', () => reject(new Error('request cancelled')), { once: true })
    }),
    isOnline: () => true,
  })
  const pending = search.query(1, 'query')
  await Promise.resolve()
  search.invalidate(1)
  await assert.rejects(pending, error => error.code === 'ABORTED')
})

test('Core item open policy preserves LAN, WAN and mobile URL selection', () => {
  const item = { url: 'https://wan.example', lanUrl: 'http://lan.local', mobileUrl: 'https://m.example' }
  assert.equal(resolveItemOpenUrl(item, { networkMode: 'lan', isMobile: false }), item.lanUrl)
  assert.equal(resolveItemOpenUrl(item, { networkMode: 'lan', isMobile: true, forceWan: true }), item.mobileUrl)
  assert.equal(resolveItemOpenUrl(item, { networkMode: 'wan', isMobile: true }), item.mobileUrl)
  assert.equal(resolveItemOpenUrl(item, { networkMode: 'wan', isMobile: false }), item.url)
})

test('monitor snapshot polling is serial, reads the interval once and stops cleanly', async () => {
  let active = 0
  let peak = 0
  let fetches = 0
  let intervalReads = 0
  let snapshots = 0
  const poller = createSnapshotPoller({
    fetchSnapshot: async () => {
      active++
      peak = Math.max(peak, active)
      fetches++
      await delay(5)
      active--
      return { sequence: fetches }
    },
    getInterval: async () => { intervalReads++; return 250 },
    onSnapshot: () => { snapshots++ },
  })

  poller.start()
  await delay(300)
  poller.stop()
  const stoppedAt = fetches
  await delay(280)
  assert.equal(peak, 1)
  assert.equal(intervalReads, 1)
  assert.equal(fetches, stoppedAt)
  assert.ok(snapshots >= 2)
})

test('monitor disk snapshots deduplicate paths and retain values after a failed request', async () => {
  const requested = []
  const result = await loadDiskSnapshots(
    ['/data', '/backup', '/data'],
    async (path) => {
      requested.push(path)
      if (path === '/backup') throw new Error('disk stats unavailable')
      return { code: 0, data: { used: 12 } }
    },
    { '/backup': { used: 8 }, '/removed': { used: 99 } },
  )

  assert.deepEqual(requested.sort(), ['/backup', '/data'])
  assert.deepEqual(result, {
    '/backup': { used: 8 },
    '/data': { used: 12 },
  })
})
