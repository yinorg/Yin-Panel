import assert from 'node:assert/strict'
import { setTimeout as delay } from 'node:timers/promises'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { createHomeCollectionLoader } from '../../src/core/home/collection.ts'
import { createHomeBootstrap } from '../../src/core/home/bootstrap.ts'
import { createHomeSpaceController } from '../../src/core/home/spaceController.ts'
import { createHomeMutationService } from '../../src/core/home/mutations.ts'
import { createThemeHomeSnapshot } from '../../src/core/home/themeSnapshot.ts'
import { createHomeSearchService } from '../../src/core/home/search.ts'
import { createHomeThemeHandlers } from '../../src/core/home/themeHandlers.ts'
import { executeHomeThemeRequest } from '../../src/core/home/themeRequest.ts'
import { createThemePersistence } from '../../src/core/home/themePersistence.ts'
import { createThemeSettingsStore } from '../../src/core/home/themeSettingsStore.ts'
import { resolveItemOpenUrl } from '../../src/core/items/openPolicy.ts'
import { createSnapshotPoller } from '../../src/core/monitor/snapshotPoller.ts'
import { loadDiskSnapshots } from '../../src/core/monitor/diskSnapshots.ts'
import { normalizeMonitorSnapshot } from '../../src/core/monitor/themeSnapshot.ts'

test('home bootstrap cancels superseded requests and only returns the latest values', async () => {
  let resolveOldSpaces
  let resolveNewSpaces
  const bootstrap = createHomeBootstrap({
    getMonitor: async () => ({ enabled: true }),
    getSpaces: () => new Promise(resolve => {
      if (!resolveOldSpaces) resolveOldSpaces = resolve
      else resolveNewSpaces = resolve
    }),
    refreshConfig: async () => true,
  })

  const oldLoad = bootstrap.load()
  const newLoad = bootstrap.load()
  resolveOldSpaces({ data: [{ id: 1 }] })
  resolveNewSpaces({ data: [{ id: 2 }] })
  assert.deepEqual(await oldLoad, { cancelled: true })
  assert.deepEqual(await newLoad, {
    cancelled: false,
    monitor: { enabled: true },
    spaces: { data: [{ id: 2 }] },
    spacesFailed: false,
  })
})

test('home bootstrap cancellation is prompt and timeout does not leave a live timer', async () => {
  const bootstrap = createHomeBootstrap({
    getMonitor: () => new Promise(() => {}),
    getSpaces: () => new Promise(() => {}),
    refreshConfig: () => new Promise(() => {}),
    timeoutMs: 5,
  })
  const pending = bootstrap.load()
  bootstrap.cancel()
  assert.deepEqual(await pending, { cancelled: true })
})

test('home space controller ignores stale list responses and keeps the active Space selection', async () => {
  const state = { spaces: [], active: undefined, cache: [], loaded: [] }
  const pending = []
  const controller = createHomeSpaceController({
    getSpaces: signal => new Promise(resolve => pending.push({ signal, resolve })),
    sortSpaces: spaces => [...spaces].sort((a, b) => a.id - b.id),
    getCurrentSpaces: () => state.spaces,
    readCachedSpaces: () => state.spaces,
    writeCachedSpaces: spaces => { state.cache = spaces },
    getActiveSpaceId: () => state.active?.id,
    setSpaces: spaces => { state.spaces = spaces },
    setActiveSpace: space => { state.active = space },
    loadSpace: id => { state.loaded.push(id) },
    canWrite: () => true,
  })

  const oldRefresh = controller.refresh()
  const latestRefresh = controller.refresh(true)
  pending[0].resolve({ data: [{ id: 1 }] })
  pending[1].resolve({ data: [{ id: 2 }, { id: 3 }] })
  assert.equal(await oldRefresh, false)
  assert.equal(await latestRefresh, true)
  assert.deepEqual(state.spaces.map(space => space.id), [2, 3])
  assert.equal(state.active.id, 3)
  assert.deepEqual(state.cache.map(space => space.id), [2, 3])
  assert.deepEqual(state.loaded, [3])
  assert.equal(pending[0].signal.aborted, true)
})

test('home space controller restores cache, selects known Spaces and rejects unavailable selections', () => {
  const state = { spaces: [{ id: 7 }, { id: 4 }], active: undefined, loaded: [] }
  const controller = createHomeSpaceController({
    getSpaces: async () => ({ data: [] }),
    sortSpaces: spaces => [...spaces].sort((a, b) => a.id - b.id),
    getCurrentSpaces: () => state.spaces,
    readCachedSpaces: () => state.spaces,
    writeCachedSpaces: () => {},
    getActiveSpaceId: () => state.active?.id,
    setSpaces: spaces => { state.spaces = spaces },
    setActiveSpace: space => { state.active = space },
    loadSpace: id => { state.loaded.push(id) },
    canWrite: () => true,
  })

  assert.equal(controller.restoreCached().id, 4)
  assert.equal(controller.select(7), true)
  assert.equal(controller.select(99), false)
  assert.equal(state.active.id, 7)
  assert.deepEqual(state.loaded, [4, 7])
})

test('home mutations require write access and refresh only the Space that was mutated', async () => {
  let activeSpaceId = 7
  let writable = true
  const calls = []
  const service = createHomeMutationService({
    api: {
      createItem: async (spaceId, input) => { calls.push(['create', spaceId, input]); return { code: 0 } },
      updateItem: async (spaceId, itemId, input) => { calls.push(['update', spaceId, itemId, input]); return { code: 0 } },
      deleteItem: async (spaceId, itemId) => { calls.push(['deleteItem', spaceId, itemId]); return { code: 0 } },
      reorderItems: async (spaceId, groupId, itemIds) => { calls.push(['reorderItems', spaceId, groupId, itemIds]); return { code: 0 } },
      createGroup: async (spaceId, input) => { calls.push(['createGroup', spaceId, input]); return { code: 0 } },
      updateGroup: async (spaceId, groupId, input) => { calls.push(['updateGroup', spaceId, groupId, input]); return { code: 0 } },
      deleteGroup: async (spaceId, groupId) => { calls.push(['deleteGroup', spaceId, groupId]); return { code: 0 } },
      reorderGroups: async (spaceId, parentId, groupIds) => { calls.push(['reorderGroups', spaceId, parentId, groupIds]); return { code: 0 } },
    },
    getActiveSpaceId: () => activeSpaceId,
    canWrite: () => writable,
    invalidateSpace: spaceId => calls.push(['invalidate', spaceId]),
    refreshSpace: spaceId => calls.push(['refresh', spaceId]),
  })

  await service.createItem({ title: 'Created' })
  assert.deepEqual(calls, [['create', 7, { title: 'Created' }], ['invalidate', 7], ['refresh', 7]])
  writable = false
  await assert.rejects(service.deleteItem(2), error => error.code === 'PERMISSION_DENIED')
  assert.equal(calls.length, 3)
})

test('home mutations do not refresh a newly selected Space after an older write finishes', async () => {
  let activeSpaceId = 7
  let finishWrite
  const calls = []
  const service = createHomeMutationService({
    api: {
      createItem: () => new Promise(resolve => { finishWrite = resolve }),
      updateItem: async () => ({ code: 0 }), deleteItem: async () => ({ code: 0 }),
      reorderItems: async () => ({ code: 0 }), createGroup: async () => ({ code: 0 }),
      updateGroup: async () => ({ code: 0 }), deleteGroup: async () => ({ code: 0 }),
      reorderGroups: async () => ({ code: 0 }),
    },
    getActiveSpaceId: () => activeSpaceId,
    canWrite: () => true,
    invalidateSpace: id => calls.push(['invalidate', id]),
    refreshSpace: id => calls.push(['refresh', id]),
  })
  const pending = service.createItem({ title: 'Created' })
  activeSpaceId = 9
  finishWrite({ code: 0 })
  await pending
  assert.deepEqual(calls, [['invalidate', 7]])
})

test('home delete confirmation aborts if its original Space is no longer active', async () => {
  let activeSpaceId = 7
  const calls = []
  const service = createHomeMutationService({
    api: {
      createItem: async () => ({ code: 0 }), updateItem: async () => ({ code: 0 }),
      deleteItem: async (spaceId, itemId) => { calls.push([spaceId, itemId]); return { code: 0 } },
      reorderItems: async () => ({ code: 0 }), createGroup: async () => ({ code: 0 }),
      updateGroup: async () => ({ code: 0 }), deleteGroup: async () => ({ code: 0 }),
      reorderGroups: async () => ({ code: 0 }),
    },
    getActiveSpaceId: () => activeSpaceId,
    canWrite: () => true,
    invalidateSpace: () => {},
    refreshSpace: () => {},
  })
  activeSpaceId = 9
  await assert.rejects(service.deleteItem(12, 7), error => error.code === 'ABORTED')
  assert.deepEqual(calls, [])
})

test('home mutation API failures do not invalidate or refresh cached data', async () => {
  const calls = []
  const service = createHomeMutationService({
    api: {
      createItem: async () => ({ code: 1, msg: 'rejected' }),
      updateItem: async () => ({ code: 0 }), deleteItem: async () => ({ code: 0 }),
      reorderItems: async () => ({ code: 0 }), createGroup: async () => ({ code: 0 }),
      updateGroup: async () => ({ code: 0 }), deleteGroup: async () => ({ code: 0 }),
      reorderGroups: async () => ({ code: 0 }),
    },
    getActiveSpaceId: () => 4,
    canWrite: () => true,
    invalidateSpace: id => calls.push(['invalidate', id]),
    refreshSpace: id => calls.push(['refresh', id]),
  })
  await assert.rejects(service.createItem({}), error => error.code === 'RUNTIME_UNAVAILABLE')
  assert.deepEqual(calls, [])
})

test('Core Home snapshot exposes stable DTO IDs and omits internal item URLs', () => {
  const snapshot = createThemeHomeSnapshot({
    version: 3,
    status: 'ready',
    spaces: [{ id: 10, name: 'Work', side: 'yin', pairedSpaceId: 11 }],
    activeSpaceId: 10,
    groups: [{
      id: 20,
      title: 'Tools',
      items: [{ id: 30, title: 'Admin', url: 'https://internal.test', openMethod: 1, sort: 2 }],
    }],
    canWrite: false,
  })

  assert.deepEqual(snapshot.spaces[0], { id: '10', name: 'Work', side: 'yin', pairedSpaceId: '11', capabilities: ['space.select'] })
  assert.deepEqual(snapshot.groups[0], { id: '20', spaceId: '10', parentId: undefined, title: 'Tools', icon: undefined, itemIds: ['30'] })
  assert.equal(snapshot.items[0].id, '30')
  assert.deepEqual(snapshot.items[0].capabilities, ['item.open'])
  assert.equal('url' in snapshot.items[0], false)
})

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
    createItem: () => {},
    updateItem: () => {},
    deleteItem: () => {},
    reorderItems: () => {},
    createGroup: () => {},
    updateGroup: () => {},
    deleteGroup: () => {},
    reorderGroups: () => {},
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
  const largerPage = await executeHomeThemeRequest(handlers, { method: 'search.query', query: 'Theme', limit: 150 })
  assert.equal(largerPage.total, 2)
  await assert.rejects(
    executeHomeThemeRequest(handlers, { method: 'search.query', query: 'Theme', limit: 201 }),
    error => error.code === 'INVALID_ARGUMENT',
  )
})

test('Theme write commands stay inside the active Space and validate complete reorder sets', async () => {
  const existingItem = { id: 1, itemIconGroupId: 5, title: 'Existing', url: 'https://safe.example', openMethod: 2 }
  const calls = []
  const groups = [
    { id: 5, title: 'Root', parentId: null, items: [existingItem, { id: 2, itemIconGroupId: 5, title: 'Second', url: 'https://second.example' }] },
    { id: 6, title: 'Child', parentId: 5, items: [] },
    { id: 7, title: 'Other root', parentId: null, items: [] },
  ]
  const handlers = createHomeThemeHandlers({
    getSpaces: () => [{ id: 10 }],
    getGroups: () => groups,
    getActiveSpaceId: () => 10,
    selectSpace: () => {},
    openItem: () => {},
    openEditor: () => {},
    createItem: input => calls.push(['item.create', input]),
    updateItem: (id, input) => calls.push(['item.update', id, input]),
    deleteItem: item => calls.push(['item.delete', item.id]),
    reorderItems: (id, ids) => calls.push(['items.reorder', id, ids]),
    createGroup: input => calls.push(['group.create', input]),
    updateGroup: (id, input) => calls.push(['group.update', id, input]),
    deleteGroup: group => calls.push(['group.delete', group.id]),
    reorderGroups: (parentId, ids) => calls.push(['groups.reorder', parentId, ids]),
    openCommandCenter: () => {},
    toggleSide: () => {},
    refresh: () => {},
    searchItems: () => [],
    getMonitorSnapshot: () => ({}),
    submitSearch: () => {},
    navigate: () => {},
    getSettings: () => ({}),
    patchSettings: () => {},
    getStorage: () => undefined,
    setStorage: () => {},
    removeStorage: () => {},
  })

  await handlers.executeCommand('item.create', { title: 'Created', url: 'https://new.example', groupId: '5' })
  assert.deepEqual(calls[0], ['item.create', {
    itemIconGroupId: 5, title: 'Created', url: 'https://new.example', lanUrl: '', mobileUrl: '',
    description: '', openMethod: 2, sort: 0, icon: { itemType: 4 },
  }])
  await handlers.executeCommand('item.update', { itemId: '1', title: 'Renamed' })
  assert.equal(calls[1][1], 1)
  assert.equal(calls[1][2].url, 'https://safe.example')
  await assert.rejects(handlers.executeCommand('item.create', { title: 'Unsafe', url: 'javascript:alert(1)', groupId: '5' }), error => error.code === 'INVALID_ARGUMENT')
  await assert.rejects(handlers.executeCommand('item.create', { title: 'Foreign', url: 'https://example.test', groupId: '999' }), error => error.code === 'NOT_FOUND')
  await assert.rejects(handlers.executeCommand('items.reorder', { groupId: '5', itemIds: ['1'] }), error => error.code === 'INVALID_ARGUMENT')
  await handlers.executeCommand('items.reorder', { groupId: '5', itemIds: ['2', '1'] })
  await handlers.executeCommand('group.create', { title: 'Nested', parentId: '5' })
  await assert.rejects(handlers.executeCommand('group.update', { groupId: '5', parentId: '6' }), error => error.code === 'INVALID_ARGUMENT')
  await assert.rejects(handlers.executeCommand('group.create', { title: 'Foreign child', parentId: '999' }), error => error.code === 'NOT_FOUND')
  await assert.rejects(handlers.executeCommand('editor.open', { groupId: '999' }), error => error.code === 'NOT_FOUND')
  await handlers.executeCommand('groups.reorder', { groupIds: ['7', '5'] })
  await assert.rejects(handlers.executeCommand('groups.reorder', { groupIds: ['5'] }), error => error.code === 'INVALID_ARGUMENT')
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

test('theme persistence isolates settings by revision and storage by user and package', () => {
  const values = new Map()
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: key => values.delete(key),
  }
  const identity = { userId: 'user-1', packageId: 'theme-a', revision: 'revision-1' }
  const persistence = createThemePersistence(storage, {
    userId: () => identity.userId,
    packageId: () => identity.packageId,
    revision: () => identity.revision,
  })

  persistence.patchSettings({ density: 'compact' })
  persistence.setStorage('state', { count: 1 })
  identity.revision = 'revision-2'
  assert.deepEqual(persistence.getSettings(), {})
  assert.deepEqual(persistence.getStorage('state'), { count: 1 })
  identity.packageId = 'theme-b'
  assert.equal(persistence.getStorage('state'), undefined)
  identity.userId = 'user-2'
  assert.equal(persistence.getStorage('state'), undefined)
})

test('theme persistence rejects unserializable and oversized data by UTF-8 byte size', () => {
  const values = new Map()
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: key => values.delete(key),
  }
  const persistence = createThemePersistence(storage, {
    userId: () => 'user-1',
    packageId: () => 'theme-a',
    revision: () => 'revision-1',
  }, 12)

  persistence.setStorage('state', 'ab')
  assert.throws(() => persistence.setStorage('state', '中文中文'), error => error.code === 'INVALID_ARGUMENT')
  assert.throws(() => persistence.setStorage('state', undefined), error => error.code === 'INVALID_ARGUMENT')
  const circular = {}
  circular.self = circular
  assert.throws(() => persistence.setStorage('state', circular), error => error.code === 'INVALID_ARGUMENT')
  assert.equal(persistence.getStorage('state'), 'ab')
})

test('theme persistence treats corrupt stored JSON as unavailable and supports removal', () => {
  const values = new Map([['yin-theme-storage:user-1:theme-a:state', '{']])
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: key => values.delete(key),
  }
  const persistence = createThemePersistence(storage, {
    userId: () => 'user-1',
    packageId: () => 'theme-a',
    revision: () => 'revision-1',
  })

  assert.equal(persistence.getStorage('state'), undefined)
  persistence.setStorage('state', true)
  assert.equal(persistence.getStorage('state'), true)
  persistence.removeStorage('state')
  assert.equal(persistence.getStorage('state'), undefined)
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
