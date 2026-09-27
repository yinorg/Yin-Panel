import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { setTimeout as delay } from 'node:timers/promises'
// The frontend does not depend on a unit-test framework; use Node's built-in runner.
// eslint-disable-next-line test/no-import-node-test
import test from 'node:test'
import { createHomeCollectionLoader } from '../../src/core/home/collection.ts'
import { createHomeBootstrap } from '../../src/core/home/bootstrap.ts'
import { createHomeSpaceController } from '../../src/core/home/spaceController.ts'
import { createHomeMutationService } from '../../src/core/home/mutations.ts'
import { createThemeHomeSnapshot } from '../../src/core/home/themeSnapshot.ts'
import { createIconifyResourceResolver, isValidIconifyIdentifier } from '../../src/core/home/iconifyResource.ts'
import { createHomeSearchService } from '../../src/core/home/search.ts'
import { createHomeCommandSearch, filterHomeCommandItems, filterHomeCommands, findHomeCommandItem, getInitialHomeCommandSelection, isHomeCommandWrite, moveHomeCommandSelection, parseHomeCommand } from '../../src/core/home/commandCenter.ts'
import { buildHomeGroupTree, getDirectoryGroups, getHomeGroupRoots, getInitiallyCollapsedGroups, isHomeGroupHidden, resolveActiveDirectoryId } from '../../src/core/home/groupTree.ts'
import { createHomeThemeHandlers } from '../../src/core/home/themeHandlers.ts'
import { executeHomeThemeRequest } from '../../src/core/home/themeRequest.ts'
import { createThemePersistence } from '../../src/core/home/themePersistence.ts'
import { createThemeSettingsSchemaValidator } from '../../src/core/home/themeSettingsSchema.ts'
import { createThemeSettingsStore } from '../../src/core/home/themeSettingsStore.ts'
import { resolveItemOpenUrl } from '../../src/core/items/openPolicy.ts'
import { createSnapshotPoller } from '../../src/core/monitor/snapshotPoller.ts'
import { loadDiskSnapshots } from '../../src/core/monitor/diskSnapshots.ts'
import { createMonitorSnapshotController } from '../../src/core/monitor/snapshotController.ts'
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

test('home group tree preserves hierarchy order and attaches loaded items', () => {
  const source = [
    { id: 10, title: 'Root', parentId: null },
    { id: 12, title: 'Child', parentId: 10 },
    { id: 11, title: 'Root sibling', parentId: null },
    { id: 13, title: 'Grandchild', parentId: 12 },
  ]
  const { groups, childrenById } = buildHomeGroupTree(source, new Map([[12, [{ id: 120 }]]]))
  assert.deepEqual(groups.map(group => [group.id, group.depth]), [[10, 0], [12, 1], [13, 2], [11, 0]])
  assert.deepEqual(groups.find(group => group.id === 12).items, [{ id: 120 }])
  assert.deepEqual(childrenById.get(10).map(group => group.id), [12])
})

test('directory selection shows one root and all descendants, and falls back when selection disappears', () => {
  const groups = [
    { id: 10, parentId: null },
    { id: 11, parentId: 10 },
    { id: 12, parentId: 11 },
    { id: 20, parentId: null },
    { id: 21, parentId: 20 },
  ]
  assert.deepEqual(getHomeGroupRoots(groups).map(group => group.id), [10, 20])
  assert.deepEqual(getDirectoryGroups(groups, 20).map(group => group.id), [20, 21])
  assert.equal(resolveActiveDirectoryId(groups, 20), 20)
  assert.equal(resolveActiveDirectoryId(groups, 99), 10)
  assert.equal(resolveActiveDirectoryId([], 99), null)
})

test('standard group collapse hides descendants and protects against malformed cycles', () => {
  const groups = [
    { id: 10, parentId: null, items: [] },
    { id: 11, parentId: 10, items: [] },
    { id: 12, parentId: 11, items: Array.from({ length: 41 }, (_, id) => ({ id })) },
  ]
  const collapsed = getInitiallyCollapsedGroups(groups, true)
  assert.deepEqual([...collapsed], [10, 11, 12])
  assert.equal(isHomeGroupHidden(groups, 1, collapsed), true)
  assert.equal(isHomeGroupHidden(groups, 0, collapsed), false)
  assert.deepEqual([...getInitiallyCollapsedGroups(groups, false)], [])
  assert.equal(isHomeGroupHidden([{ id: 1, parentId: 2 }, { id: 2, parentId: 1 }], 0, new Set()), false)
})

test('home group tree keeps orphaned and cyclic groups reachable without recursing forever', () => {
  const { groups } = buildHomeGroupTree([
    { id: 1, parentId: 99 },
    { id: 2, parentId: 3 },
    { id: 3, parentId: 2 },
  ])
  assert.deepEqual(groups.map(group => group.id), [1, 2, 3])
  assert.ok(groups.every(group => Number.isInteger(group.depth)))
  assert.deepEqual(getHomeGroupRoots(groups).map(group => group.id), [1, 2])
  assert.deepEqual(getDirectoryGroups(groups, 1).map(group => group.id), [1])
})

test('home command center filters commands, deduplicates bookmark matches, and parses command arguments', () => {
  const commands = [
    { key: 'add', label: 'Add bookmark' },
    { key: 'open', label: 'Open bookmark' },
  ]
  const remote = [{ id: 1, title: 'Alpha', url: 'https://alpha.test' }]
  const local = [
    { id: 1, title: 'Alpha local copy', url: 'https://alpha.test' },
    { id: 2, title: 'Beta bookmark', url: 'https://beta.test' },
  ]
  assert.deepEqual(filterHomeCommands(commands, '/ADD'), [commands[0]])
  assert.deepEqual(filterHomeCommandItems('alpha', remote, local), [remote[0]])
  assert.deepEqual(filterHomeCommandItems('/open', remote, local), [])
  assert.equal(findHomeCommandItem('beta', remote, local).id, 2)
  assert.deepEqual(parseHomeCommand(' /open  alpha  bookmark '), { command: 'open', argument: 'alpha bookmark' })
  assert.equal(isHomeCommandWrite('edit'), true)
  assert.equal(isHomeCommandWrite('open'), false)
})

test('home command center wraps selection and uses query type to choose its initial index', () => {
  assert.equal(getInitialHomeCommandSelection('/'), 0)
  assert.equal(getInitialHomeCommandSelection('alpha'), -1)
  assert.equal(moveHomeCommandSelection(-1, 1, 3), 0)
  assert.equal(moveHomeCommandSelection(0, -1, 3), 2)
  assert.equal(moveHomeCommandSelection(4, 1, 0), 4)
})

test('home command search only publishes the latest query for the still-active Space', async () => {
  let spaceId = 7
  const pending = []
  const service = createHomeCommandSearch({
    getSpaceId: () => spaceId,
    search: (id, query) => new Promise(resolve => pending.push({ id, query, resolve })),
  })
  const stale = service.search('alpha')
  const latest = service.search('beta')
  pending[0].resolve([{ id: 1, title: 'Alpha' }])
  pending[1].resolve([{ id: 2, title: 'Beta' }])
  assert.equal(await stale, undefined)
  assert.deepEqual(await latest, [{ id: 2, title: 'Beta' }])

  const changedSpace = service.search('gamma')
  spaceId = 9
  pending[2].resolve([{ id: 3, title: 'Gamma' }])
  assert.equal(await changedSpace, undefined)
  assert.deepEqual(await service.search('/open'), [])
})

test('home mutations require write access and refresh only the Space that was mutated', async () => {
  const activeSpaceId = 7
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

test('item editor mutations preserve the opened Space and return created item data', async () => {
  let activeSpaceId = 7
  const calls = []
  const service = createHomeMutationService({
    api: {
      createItem: async (spaceId, input) => { calls.push(['create', spaceId, input]); return { code: 0, data: { id: 12 } } },
      createItemWithIcon: async (spaceId, input, file) => { calls.push(['createWithIcon', spaceId, input, file]); return { code: 0, data: { id: 13 } } },
      updateItem: async (spaceId, itemId, input) => { calls.push(['update', spaceId, itemId, input]); return { code: 0, data: { id: itemId } } },
      deleteItem: async () => ({ code: 0 }), reorderItems: async () => ({ code: 0 }),
      createGroup: async () => ({ code: 0 }), updateGroup: async () => ({ code: 0 }),
      deleteGroup: async () => ({ code: 0 }), reorderGroups: async () => ({ code: 0 }),
    },
    getActiveSpaceId: () => activeSpaceId,
    canWrite: () => true,
    invalidateSpace: id => calls.push(['invalidate', id]),
    refreshSpace: id => calls.push(['refresh', id]),
  })
  const file = { name: 'icon.svg' }
  const result = await service.saveItem({ title: 'Created' }, { iconFile: file, expectedSpaceId: 7 })
  assert.deepEqual(result.data, { id: 13 })
  assert.deepEqual(calls.slice(0, 3), [['createWithIcon', 7, { title: 'Created' }, file], ['invalidate', 7], ['refresh', 7]])

  activeSpaceId = 9
  await assert.rejects(service.saveItem({ title: 'Updated' }, { itemId: 12, expectedSpaceId: 7 }), error => error.code === 'ABORTED')
  assert.equal(calls.some(call => call[0] === 'update'), false)
})

test('item icon uploads require the opened writable Space and reject late responses', async () => {
  let activeSpaceId = 7
  let finishUpload
  let uploads = 0
  const service = createHomeMutationService({
    api: {
      createItem: async () => ({ code: 0 }), createItemWithIcon: async () => ({ code: 0 }), updateItem: async () => ({ code: 0 }),
      uploadItemIcon: () => {
        uploads++
        return new Promise(resolve => { finishUpload = resolve })
      },
      deleteItem: async () => ({ code: 0 }), reorderItems: async () => ({ code: 0 }),
      createGroup: async () => ({ code: 0 }), updateGroup: async () => ({ code: 0 }),
      deleteGroup: async () => ({ code: 0 }), reorderGroups: async () => ({ code: 0 }),
    },
    getActiveSpaceId: () => activeSpaceId,
    canWrite: () => true,
    invalidateSpace: () => {},
    refreshSpace: () => {},
  })

  const pending = service.uploadItemIcon({ name: 'icon.svg' }, 7)
  activeSpaceId = 9
  finishUpload({ code: 0, data: { imageUrl: '/uploads/icon.svg', fileName: 'icon.svg' } })
  await assert.rejects(pending, error => error.code === 'ABORTED')
  await assert.rejects(service.uploadItemIcon({ name: 'other.svg' }, 7), error => error.code === 'ABORTED')
  assert.equal(uploads, 1)
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
  assert.deepEqual(snapshot.groups[0], { id: '20', spaceId: '10', parentId: undefined, title: 'Tools', icon: undefined, itemIds: ['30'], capabilities: [] })
  assert.equal(snapshot.items[0].id, '30')
  assert.deepEqual(snapshot.items[0].capabilities, ['item.open'])
  assert.equal('url' in snapshot.items[0], false)
})

test('Core Home snapshot keeps Item and Group write capabilities independent and read-only by default', () => {
  const input = {
    version: 1,
    status: 'ready',
    spaces: [],
    activeSpaceId: 4,
    groups: [{ id: 8, title: 'Tools', items: [{ id: 12, title: 'Admin' }] }],
  }

  const itemOnly = createThemeHomeSnapshot({ ...input, activeSpaceCanEdit: true, canWrite: true, canWriteGroups: false })
  assert.deepEqual(itemOnly.capabilities, ['items.write'])
  assert.deepEqual(itemOnly.items[0].capabilities, ['item.open', 'item.update', 'item.delete'])
  assert.deepEqual(itemOnly.groups[0].capabilities, [])

  const groupOnly = createThemeHomeSnapshot({ ...input, activeSpaceCanEdit: true, canWrite: false, canWriteGroups: true })
  assert.deepEqual(groupOnly.capabilities, ['groups.write'])
  assert.deepEqual(groupOnly.items[0].capabilities, ['item.open'])
  assert.deepEqual(groupOnly.groups[0].capabilities, ['groups.write'])

  const readOnly = createThemeHomeSnapshot({ ...input, activeSpaceCanEdit: false, canWrite: true, canWriteGroups: true })
  assert.equal(readOnly.capabilities, undefined)
  assert.deepEqual(readOnly.items[0].capabilities, ['item.open'])
  assert.deepEqual(readOnly.groups[0].capabilities, [])
})

test('Core Home presentation maps panel configuration, search engines, and safe numeric defaults', () => {
  const panelConfig = {
    homeLayout: 'directory', iconStyle: 0, iconTextColor: '#abc', iconTextInfoHideDescription: true,
    iconTextIconHideTitle: true, logoText: 'Workspace', logoImageSrc: '/logo.svg', clockShowSecond: false,
    clockColor: '#123456', searchBoxShow: false, searchBoxSearchIcon: false,
    marginTop: 18, marginBottom: 27, maxWidth: 88, maxWidthUnit: '%', marginX: 24,
    footerHtml: '<strong>Footer</strong>', systemMonitorShow: true, systemMonitorShowTitle: false,
  }
  const engine = { id: 'bing', title: 'Bing', iconSrc: '/bing.svg', url: 'https://search.example/?q=%s' }
  const snapshot = createThemeHomeSnapshot({
    version: 4, status: 'ready', spaces: [], groups: [], canWrite: false,
    monitorReservedHeight: 184,
    presentation: panelConfig,
    searchConfiguration: { engines: [engine], currentSearchEngine: engine },
  })
  assert.deepEqual(snapshot.presentation, {
    layout: 'directory', iconStyle: 'info', iconTextColor: '#abc', iconTextInfoHideDescription: true,
    iconTextIconHideTitle: true, logoText: 'Workspace', logoImageSrc: '/logo.svg',
    clock: { visible: true, showSeconds: false, color: '#123456' },
    search: { visible: false, itemFilterEnabled: false, engines: [{ id: 'bing', title: 'Bing', iconSrc: '/bing.svg' }], currentEngineId: 'bing' },
    content: { marginTopPercent: 18, marginBottomPercent: 27, maxWidth: 88, maxWidthUnit: '%', marginX: 24 },
    footerHtml: '<strong>Footer</strong>', monitor: { visible: true, showTitle: false, reservedHeight: 184 },
  })
  assert.deepEqual(panelConfig, {
    homeLayout: 'directory', iconStyle: 0, iconTextColor: '#abc', iconTextInfoHideDescription: true,
    iconTextIconHideTitle: true, logoText: 'Workspace', logoImageSrc: '/logo.svg', clockShowSecond: false,
    clockColor: '#123456', searchBoxShow: false, searchBoxSearchIcon: false,
    marginTop: 18, marginBottom: 27, maxWidth: 88, maxWidthUnit: '%', marginX: 24,
    footerHtml: '<strong>Footer</strong>', systemMonitorShow: true, systemMonitorShowTitle: false,
  })

  const defaults = createThemeHomeSnapshot({ version: 5, status: 'ready', spaces: [], groups: [], canWrite: false, presentation: { marginTop: NaN, marginBottom: 140, maxWidth: -2, maxWidthUnit: 'invalid', marginX: Infinity } }).presentation
  assert.equal(defaults.layout, 'standard')
  assert.equal(defaults.iconStyle, 'icon')
  assert.deepEqual(defaults.content, { marginTopPercent: 10, marginBottomPercent: 10, maxWidth: 1200, maxWidthUnit: 'px', marginX: 5 })
})

test('Core Home snapshot preserves text, image, and Iconify item icon representations', () => {
  const safeIconUri = `data:image/svg+xml;charset=utf-8,${encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0"/></svg>')}`
  const snapshot = createThemeHomeSnapshot({
    version: 1, status: 'ready', spaces: [], activeSpaceId: 1, canWrite: false,
    groups: [{ id: 1, items: [
      { id: 1, title: 'Text', icon: { itemType: 1, text: 'TX', backgroundColor: '#123456' } },
      { id: 2, title: 'Image', icon: { itemType: 2, src: '/icons/image.png', fileName: 'image.png' } },
      { id: 3, title: 'Iconify unresolved', icon: { itemType: 3, text: 'mdi:home' } },
      { id: 4, title: 'Iconify resolved', icon: { itemType: 3, text: 'mdi:home', resolvedSrc: safeIconUri, backgroundColor: '#654321' } },
    ] }],
  })
  assert.deepEqual(snapshot.items.map(item => item.icon), [
    { itemType: 1, text: 'TX', backgroundColor: '#123456' },
    { itemType: 2, src: '/icons/image.png', fileName: 'image.png', backgroundColor: undefined },
    { itemType: 4, backgroundColor: undefined },
    { itemType: 3, src: safeIconUri, backgroundColor: '#654321' },
  ])
  assert.equal(JSON.stringify(snapshot).includes('mdi:home'), false)
})

test('Core Iconify resource resolver validates identifiers, sanitizes SVG, caches resources, and ignores stale generations', async () => {
  assert.equal(isValidIconifyIdentifier('mdi:home-outline'), true)
  for (const value of ['https://attacker.test/icon.svg', '../path:home', 'mdi:<script>', 'mdi:home?x=1', 'mdi:Home'])
    assert.equal(isValidIconifyIdentifier(value), false)

  const calls = []
  const safeTree = makeSvgDocument({
    name: 'svg', attrs: { xmlns: 'http://www.w3.org/2000/svg', width: '1em', height: '1em', viewBox: '0 0 24 24' },
    children: [{ name: 'path', attrs: { d: 'M0 0L24 24', fill: 'currentColor' } }],
  })
  const resolver = createIconifyResourceResolver({
    fetch: async (input, init) => {
      calls.push([input, init])
      return { ok: true, headers: { get: () => 'image/svg+xml; charset=utf-8' }, text: async () => '<svg>ignored by parser</svg>' }
    },
    parseSvg: () => safeTree,
  })
  const generation = resolver.beginGeneration()
  const [resource, duplicate] = await Promise.all([
    resolver.resolve('mdi:home-outline', generation),
    resolver.resolve('mdi:home-outline', generation),
  ])
  assert.equal(resource, duplicate)
  assert.match(resource, /^data:image\/svg\+xml;charset=utf-8,/)
  assert.match(decodeURIComponent(resource), /width="1em" height="1em"/)
  assert.match(decodeURIComponent(resource), /<path d="M0 0L24 24" fill="currentColor"><\/path>/)
  assert.doesNotMatch(resource, /api\.iconify\.design/)
  assert.equal(calls.length, 1)
  assert.equal(calls[0][0], 'https://api.iconify.design/mdi/home-outline.svg')
  assert.equal(calls[0][1].credentials, 'omit')
  assert.equal(calls[0][1].redirect, 'error')

  const nextGeneration = resolver.beginGeneration()
  assert.equal(await resolver.resolve('mdi:home-outline', nextGeneration), resource)
  assert.equal(calls.length, 1)

  const malicious = createIconifyResourceResolver({
    fetch: async () => ({ ok: true, headers: { get: () => 'image/svg+xml' }, text: async () => '<svg/>' }),
    parseSvg: () => makeSvgDocument({ name: 'svg', attrs: {}, children: [{ name: 'script', attrs: {}, children: [] }] }),
  })
  const maliciousGeneration = malicious.beginGeneration()
  assert.equal(await malicious.resolve('mdi:home', maliciousGeneration), undefined)

  const attributeInjection = createIconifyResourceResolver({
    fetch: async () => ({ ok: true, headers: { get: () => 'image/svg+xml' }, text: async () => '<svg/>' }),
    parseSvg: () => makeSvgDocument({ name: 'svg', attrs: { width: 'expression(alert(1))' }, children: [{ name: 'path', attrs: { d: 'M0 0', onload: 'alert(1)' }, children: [] }] }),
  })
  const injectionGeneration = attributeInjection.beginGeneration()
  assert.equal(await attributeInjection.resolve('mdi:alert', injectionGeneration), undefined)
})

test('Core Iconify resource resolver aborts previous Space work and never publishes late results', async () => {
  let finishRequest
  let receivedSignal
  const resolver = createIconifyResourceResolver({
    fetch: (_input, init) => {
      receivedSignal = init.signal
      return new Promise(resolve => { finishRequest = resolve })
    },
    parseSvg: () => makeSvgDocument({ name: 'svg', attrs: {}, children: [{ name: 'path', attrs: { d: 'M0 0' }, children: [] }] }),
  })
  const firstGeneration = resolver.beginGeneration()
  const stale = resolver.resolve('mdi:home', firstGeneration)
  await Promise.resolve()
  const currentGeneration = resolver.beginGeneration()
  assert.equal(receivedSignal.aborted, true)
  finishRequest({ ok: true, headers: { get: () => 'image/svg+xml' }, text: async () => '<svg/>' })
  assert.equal(await stale, undefined)
  assert.equal(resolver.isCurrentGeneration(currentGeneration), true)
  assert.equal(await resolver.resolve('mdi:home', firstGeneration), undefined)
})

function makeSvgDocument({ name, attrs, children = [] }) {
  const makeElement = element => ({
    nodeType: 1,
    localName: element.name,
    namespaceURI: 'http://www.w3.org/2000/svg',
    attributes: Object.entries(element.attrs).map(([attrName, value]) => ({ name: attrName, value })),
    childNodes: (element.children || []).map(makeElement),
  })
  return { documentElement: makeElement({ name, attrs, children }), doctype: null }
}

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
  assert.deepEqual(groups.items[0], { id: '5', spaceId: '10', parentId: undefined, title: 'Bookmarks', icon: undefined, itemIds: ['1', '2'], capabilities: [] })
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

test('Theme search.submit filters, clears, and submits only an allowed Space search engine', async () => {
  const submitted = []
  const external = []
  let activeEngines = [{ id: 'bing' }, { id: 'google' }]
  const handlers = createHomeThemeHandlers({
    getSpaces: () => [], getGroups: () => [], getActiveSpaceId: () => undefined,
    selectSpace: () => {}, openItem: () => {}, openEditor: () => {}, createItem: () => {}, updateItem: () => {},
    deleteItem: () => {}, reorderItems: () => {}, createGroup: () => {}, updateGroup: () => {}, deleteGroup: () => {}, reorderGroups: () => {},
    openCommandCenter: () => {}, toggleSide: () => {}, refresh: () => {}, searchItems: () => [], getMonitorSnapshot: () => ({}),
    submitSearch: query => submitted.push(query),
    getSearchConfiguration: () => ({ engines: activeEngines, currentEngineId: activeEngines[0]?.id }),
    submitSearchWithEngine: (query, engineId) => external.push([query, engineId]),
    navigate: () => {}, getSettings: () => ({}), patchSettings: () => {},
    getStorage: () => undefined, setStorage: () => {}, removeStorage: () => {},
  })

  await handlers.executeCommand('search.submit', { query: 'icon' })
  await handlers.executeCommand('search.submit', { action: 'clear', query: 'ignored' })
  await handlers.executeCommand('search.submit', { action: 'engine', query: 'bookmarks', engineId: 'google' })
  assert.deepEqual(submitted, ['icon', ''])
  assert.deepEqual(external, [['bookmarks', 'google']])
  activeEngines = [{ id: 'duckduckgo' }]
  await assert.rejects(
    handlers.executeCommand('search.submit', { action: 'engine', query: 'bookmarks', engineId: 'google' }),
    error => error.code === 'INVALID_ARGUMENT',
  )
  await assert.rejects(
    handlers.executeCommand('search.submit', { action: 'invalid', query: 'bookmarks' }),
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

test('Theme groups.list exposes groups.write only when Core write access and the Yin grant are both active', async () => {
  let canWriteGroups = false
  const handlers = createHomeThemeHandlers({
    getSpaces: () => [], getGroups: () => [{ id: 5, title: 'Bookmarks', items: [{ id: 1 }] }], getActiveSpaceId: () => 10,
    canWriteGroups: () => canWriteGroups,
    selectSpace: () => {}, openItem: () => {}, openEditor: () => {}, createItem: () => {}, updateItem: () => {},
    deleteItem: () => {}, reorderItems: () => {}, createGroup: () => {}, updateGroup: () => {}, deleteGroup: () => {}, reorderGroups: () => {},
    openCommandCenter: () => {}, toggleSide: () => {}, refresh: () => {}, searchItems: () => [], getMonitorSnapshot: () => ({}),
    submitSearch: () => {}, navigate: () => {}, getSettings: () => ({}), patchSettings: () => {},
    getStorage: () => undefined, setStorage: () => {}, removeStorage: () => {},
  })

  assert.deepEqual((await handlers.listGroups({ limit: 50 })).items[0].capabilities, [])
  canWriteGroups = true
  assert.deepEqual((await handlers.listGroups({ limit: 50 })).items[0].capabilities, ['groups.write'])
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

test('theme persistence isolates settings by revision and storage by user and package', async () => {
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

  await persistence.patchSettings({ density: 'compact' })
  persistence.setStorage('state', { count: 1 })
  identity.revision = 'revision-2'
  assert.deepEqual(persistence.getSettings(), {})
  assert.deepEqual(persistence.getStorage('state'), { count: 1 })
  identity.packageId = 'theme-b'
  assert.equal(persistence.getStorage('state'), undefined)
  identity.userId = 'user-2'
  assert.equal(persistence.getStorage('state'), undefined)
})

test('theme settings schema rejection leaves prior persisted settings unchanged', async () => {
  const values = new Map()
  const storage = {
    getItem: key => values.get(key) ?? null,
    setItem: (key, value) => values.set(key, value),
    removeItem: key => values.delete(key),
  }
  const persistence = createThemePersistence(storage, {
    userId: () => 'user-1', packageId: () => 'theme-a', revision: () => 'revision-1',
    validateSettings: async settings => {
      if (!['compact', 'comfortable'].includes(settings.density)) throw Object.assign(new Error('invalid settings'), { code: 'INVALID_ARGUMENT' })
    },
  })
  await persistence.patchSettings({ density: 'compact' })
  await assert.rejects(persistence.patchSettings({ density: 'loose' }), error => error.code === 'INVALID_ARGUMENT')
  assert.deepEqual(persistence.getSettings(), { density: 'compact' })
})

test('theme settings schema service verifies resource digest, compiles once per revision and validates patches', async () => {
  const Ajv2020 = (await import('ajv/dist/2020.js')).default
  const bytes = Buffer.from(JSON.stringify({
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    $defs: { density: { enum: ['compact', 'comfortable'] } },
    type: 'object', properties: { density: { $ref: '#/$defs/density' } }, additionalProperties: false,
  }))
  const digest = createHash('sha256').update(bytes).digest('hex')
  let compileCount = 0
  let fetchOptions
  const validate = createThemeSettingsSchemaValidator({
    origin: 'https://yin.test',
    fetcher: async (_url, options) => {
      fetchOptions = options
      return new Response(bytes, { status: 200 })
    },
    loadAjv: async () => ({ default: class extends Ajv2020 { constructor(options) { super(options); compileCount++ } } }),
  })
  const theme = {
    revision: 'revision-1',
    manifest: {
      settings: { schema: 'settings/schema.json', schemaVersion: 1 },
      resources: [{ path: 'settings/schema.json', url: '/api/theme/v2/assets/revision/settings/schema.json', mediaType: 'application/schema+json', sha256: digest }],
    },
  }
  await validate(theme, { density: 'compact' })
  await validate(theme, { density: 'comfortable' })
  assert.equal(compileCount, 1)
  assert.deepEqual(fetchOptions, { credentials: 'omit', redirect: 'error', cache: 'force-cache' })
  await assert.rejects(validate(theme, { density: 'loose' }), error => error.code === 'INVALID_ARGUMENT')
  await validate({ ...theme, revision: 'revision-2' }, { density: 'compact' })
  assert.equal(compileCount, 2)
})

test('theme settings schema service rejects external references, unsafe URLs and bad digests', async () => {
  const Ajv2020 = (await import('ajv/dist/2020.js')).default
  const bytes = Buffer.from(JSON.stringify({
    $schema: 'https://json-schema.org/draft/2020-12/schema',
    $ref: 'https://example.test/schema.json',
  }))
  const digest = createHash('sha256').update(bytes).digest('hex')
  let fetchCount = 0
  const validate = createThemeSettingsSchemaValidator({
    origin: 'https://yin.test',
    fetcher: async () => { fetchCount++; return new Response(bytes, { status: 200 }) },
    loadAjv: async () => ({ default: Ajv2020 }),
  })
  const base = {
    revision: 'revision-1',
    manifest: {
      settings: { schema: 'settings/schema.json', schemaVersion: 1 },
      resources: [{ path: 'settings/schema.json', url: '/api/theme/v2/assets/revision/schema.json', mediaType: 'application/schema+json', sha256: digest }],
    },
  }
  await assert.rejects(validate(base, {}), /external documents/)
  await assert.rejects(validate({ ...base, manifest: { ...base.manifest, resources: [{ ...base.manifest.resources[0], url: 'https://evil.test/api/theme/schema.json' }] } }, {}), error => error.code === 'PERMISSION_DENIED')
  await assert.rejects(validate({ ...base, manifest: { ...base.manifest, resources: [{ ...base.manifest.resources[0], url: '/api/theme/v2/admin/packages' }] } }, {}), error => error.code === 'PERMISSION_DENIED')
  await assert.rejects(validate({ ...base, manifest: { ...base.manifest, resources: [{ ...base.manifest.resources[0], url: '/api/theme/v2/assets/revision/schema.json?redirect=https://evil.test' }] } }, {}), error => error.code === 'PERMISSION_DENIED')
  await assert.rejects(validate({ ...base, manifest: { ...base.manifest, resources: [{ ...base.manifest.resources[0], sha256: '0'.repeat(64) }] } }, {}), /digest/)
  assert.equal(fetchCount, 2)
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

test('monitor polling ignores a stopped run and reuses its in-flight request after restart', async () => {
  let releaseSnapshot
  let fetches = 0
  const snapshots = []
  const poller = createSnapshotPoller({
    fetchSnapshot: () => {
      fetches++
      return new Promise(resolve => { releaseSnapshot = resolve })
    },
    getInterval: async () => 10000,
    onSnapshot: snapshot => snapshots.push(snapshot),
  })

  poller.start()
  await Promise.resolve()
  await Promise.resolve()
  assert.equal(fetches, 1)
  poller.stop()
  poller.start()
  await Promise.resolve()
  assert.equal(fetches, 1)

  releaseSnapshot({ sequence: 1 })
  await delay(0)
  poller.stop()
  assert.deepEqual(snapshots, [{ sequence: 1 }])
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

test('monitor snapshot controller deduplicates reads and follows disk configuration changes in flight', async () => {
  const requested = []
  let releaseOldDisk
  let snapshotCalls = 0
  const controller = createMonitorSnapshotController({
    fetchSnapshot: async () => { snapshotCalls++; return { CPU_INFO: { model: 'CPU' } } },
    fetchDisk: async (path) => {
      requested.push(path)
      if (path === '/old') return new Promise(resolve => { releaseOldDisk = () => resolve({ code: 0, data: { used: 1 } }) })
      return { code: 0, data: { used: 2 } }
    },
    getInterval: async () => 10000,
  })

  controller.setDiskPaths(['/old'])
  const first = controller.fetchSnapshot()
  const duplicate = controller.fetchSnapshot()
  await Promise.resolve()
  await Promise.resolve()
  assert.equal(snapshotCalls, 1)
  controller.setDiskPaths(['/new'])
  releaseOldDisk()

  const [snapshot, sameSnapshot] = await Promise.all([first, duplicate])
  assert.deepEqual(snapshot, { CPU_INFO: { model: 'CPU' }, DISK_INFO: { '/new': { used: 2 } } })
  assert.deepEqual(sameSnapshot, snapshot)
  assert.deepEqual(requested, ['/old', '/new'])
})
