import type { ThemeApiHandlers } from '../../theme/api/dispatcher'
import type { ThemeCommand, ThemeGroup, ThemeItem, ThemePage, ThemeSearchPage, ThemeSpace } from '../../theme/api/v1'
// @ts-expect-error Node's strip-types runner requires an explicit source extension.
import { toThemeItemIcon } from './iconifyResource.ts'

interface CoreSpace {
  id: number
  name?: string
  side?: 'yin' | 'yang'
  pairedSpaceId?: number
}

type CoreItem = Panel.ItemInfo

interface CoreGroup {
  id: number
  parentId?: number | null
  title?: string
  icon?: string
  sort?: number
  items?: readonly CoreItem[]
}

export interface HomeThemeActionBindings {
  getSpaces: () => readonly CoreSpace[]
  getGroups: () => readonly CoreGroup[]
  getActiveSpaceId: () => number | undefined
  canWriteGroups?: () => boolean
  selectSpace: (spaceId: number) => Promise<unknown> | unknown
  openItem: (item: CoreItem) => Promise<unknown> | unknown
  openEditor: (input: { item?: CoreItem; groupId?: number }) => Promise<unknown> | unknown
  createItem: (input: Record<string, unknown>) => Promise<unknown> | unknown
  updateItem: (itemId: number, input: Record<string, unknown>) => Promise<unknown> | unknown
  deleteItem: (item: CoreItem) => Promise<unknown> | unknown
  reorderItems: (groupId: number, itemIds: number[]) => Promise<unknown> | unknown
  createGroup: (input: { title: string; icon?: string; parentId?: number | null }) => Promise<unknown> | unknown
  updateGroup: (groupId: number, input: { title: string; icon?: string; parentId?: number | null }) => Promise<unknown> | unknown
  deleteGroup: (group: CoreGroup) => Promise<unknown> | unknown
  reorderGroups: (parentId: number | null, groupIds: number[]) => Promise<unknown> | unknown
  openCommandCenter: () => Promise<unknown> | unknown
  toggleSide: () => Promise<unknown> | unknown
  setNetworkMode: (mode: 'lan' | 'wan') => Promise<unknown> | unknown
  /** The sandbox reports the measured bottom edge of its search box. */
  reportLayout: (input: { searchBottom: number }) => Promise<unknown> | unknown
  forwardKey: (key: string) => Promise<unknown> | unknown
  openLink: (url: string) => Promise<unknown> | unknown
  refresh: () => Promise<unknown> | unknown
  searchItems: (query: string) => Promise<readonly Panel.ItemInfo[]> | readonly Panel.ItemInfo[]
  getMonitorSnapshot: () => Promise<unknown> | unknown
  submitSearch: (query: string) => Promise<unknown> | unknown
  getSearchConfiguration?: () => {
    engines: readonly { id: string }[]
    currentEngineId?: string
  }
  submitSearchWithEngine?: (query: string, engineId: string) => Promise<unknown> | unknown
  navigate: (destination: { view: string; spaceId?: string }) => Promise<unknown> | unknown
  getSettings: () => Promise<Record<string, unknown>> | Record<string, unknown>
  patchSettings: (value: Record<string, unknown>) => Promise<unknown> | unknown
  getStorage: (key: string) => Promise<unknown> | unknown
  setStorage: (key: string, value: unknown) => Promise<unknown> | unknown
  removeStorage: (key: string) => Promise<unknown> | unknown
  openCoreSurface?: (surface: string, payload: Record<string, unknown>) => Promise<unknown> | unknown
  networkFetch?: (input: string, init: Record<string, unknown>) => Promise<unknown> | unknown
  reportDiagnostic?: (entry: Record<string, unknown>) => Promise<unknown> | unknown
}

export function createHomeThemeHandlers(bindings: HomeThemeActionBindings): ThemeApiHandlers {
  const page = <T>(values: readonly T[], options: { limit: number; cursor?: string }): ThemePage<T> => {
    const offset = options.cursor ? Number(options.cursor) : 0
    const items = values.slice(offset, offset + options.limit)
    return { items, total: values.length, nextCursor: offset + items.length < values.length ? String(offset + items.length) : undefined }
  }

  const toThemeItem = (item: CoreItem): ThemeItem | undefined => {
    if (!Number.isSafeInteger(Number(item.id)) || !Number.isSafeInteger(Number(item.itemIconGroupId))) return undefined
    return {
      id: String(item.id),
      groupId: String(item.itemIconGroupId),
      title: item.title,
      description: item.description,
      icon: toThemeItemIcon(item.icon),
      sort: item.sort ?? 0,
      capabilities: ['item.open'],
      // 主题在自己的点击同步栈里开窗——只有收到手势的框能开，Core 代开在手机上会被拒。
      // 因此 `openMethod` 与 `url` 一起交给主题。
      //
      // 注意这只是 command 响应这条路径。主题 home 快照走的是 ThemeHost 的
      // `scopedSnapshot`，它逐字段重建 item——那里也必须列上这两个字段。
      openMethod: item.openMethod ?? 1,
      url: item.url || undefined,
    }
  }

  return {
    async executeCommand(command: ThemeCommand, payload: Record<string, unknown>) {
      switch (command) {
        case 'space.select': {
          const spaceId = parseCoreId(payload.spaceId)
          if (!bindings.getSpaces().some(space => space.id === spaceId)) throw apiError('NOT_FOUND', 'Space is not available')
          return bindings.selectSpace(spaceId)
        }
        case 'space.toggleSide': return bindings.toggleSide()
        case 'item.open': {
          const item = findItem(bindings.getGroups(), payload.itemId)
          if (!item) throw apiError('NOT_FOUND', 'Item is not available')
          return bindings.openItem(item)
        }
        case 'item.create':
          return bindings.createItem(toItemWriteInput(payload, undefined, bindings.getGroups()))
        case 'item.update': {
          const item = findItem(bindings.getGroups(), payload.itemId)
          if (!item) throw apiError('NOT_FOUND', 'Item is not available in the active Space')
          return bindings.updateItem(parseCoreId(item.id), toItemWriteInput(payload, item, bindings.getGroups()))
        }
        case 'item.delete': {
          const item = findItem(bindings.getGroups(), payload.itemId)
          if (!item) throw apiError('NOT_FOUND', 'Item is not available in the active Space')
          return bindings.deleteItem(item)
        }
        case 'items.reorder': {
          const groupId = parseCoreId(payload.groupId)
          const group = bindings.getGroups().find(candidate => candidate.id === groupId)
          const itemIds = parseIdList(payload.itemIds)
          if (!group || !isPermutation(group.items?.map(item => parseCoreId(item.id)) || [], itemIds))
            throw apiError('INVALID_ARGUMENT', 'Item order must contain every item in the active group exactly once')
          return bindings.reorderItems(groupId, itemIds)
        }
        case 'group.create': {
          const title = parseTitle(payload.title, 50)
          const parentId = parseOptionalParentId(payload.parentId, bindings.getGroups())
          return bindings.createGroup({ title, icon: parseOptionalString(payload.icon, 240), parentId })
        }
        case 'group.update': {
          const groupId = parseCoreId(payload.groupId)
          const group = bindings.getGroups().find(candidate => candidate.id === groupId)
          if (!group) throw apiError('NOT_FOUND', 'Group is not available in the active Space')
          const title = payload.title === undefined ? group.title || '' : parseTitle(payload.title, 50)
          const icon = payload.icon === undefined ? group.icon || '' : parseOptionalString(payload.icon, 240)
          const parentId = payload.parentId === undefined
            ? group.parentId ?? null
            : parseOptionalParentId(payload.parentId, bindings.getGroups(), groupId)
          return bindings.updateGroup(groupId, { title, icon, parentId })
        }
        case 'group.delete': {
          const groupId = parseCoreId(payload.groupId)
          const group = bindings.getGroups().find(candidate => candidate.id === groupId)
          if (!group) throw apiError('NOT_FOUND', 'Group is not available in the active Space')
          return bindings.deleteGroup(group)
        }
        case 'groups.reorder': {
          const parentId = parseOptionalParentId(payload.parentId, bindings.getGroups())
          const siblings = bindings.getGroups().filter(group => (group.parentId ?? null) === parentId)
          const groupIds = parseIdList(payload.groupIds)
          if (!isPermutation(siblings.map(group => group.id), groupIds))
            throw apiError('INVALID_ARGUMENT', 'Group order must contain every sibling in the active Space exactly once')
          return bindings.reorderGroups(parentId, groupIds)
        }
        case 'search.submit': {
          const query = typeof payload.query === 'string' ? payload.query : ''
          const action = payload.action === undefined ? 'filter' : payload.action
          if (action === 'filter') return bindings.submitSearch(query)
          if (action === 'clear') return bindings.submitSearch('')
          if (action === 'engine') {
            const engineId = typeof payload.engineId === 'string' ? payload.engineId : ''
            const configuration = bindings.getSearchConfiguration?.()
            if (!configuration?.engines.some(engine => engine.id === engineId))
              throw apiError('INVALID_ARGUMENT', 'Search engine is not available in the active Space')
            if (!bindings.submitSearchWithEngine)
              throw apiError('UNSUPPORTED_CAPABILITY', 'External search submission is not available in this adapter')
            return bindings.submitSearchWithEngine(query, engineId)
          }
          throw apiError('INVALID_ARGUMENT', 'Unsupported search action')
        }
        case 'data.refresh': return bindings.refresh()
        case 'editor.open': {
          const itemId = payload.itemId
          const item = itemId === undefined ? undefined : findItem(bindings.getGroups(), itemId)
          if (itemId !== undefined && !item) throw apiError('NOT_FOUND', 'Item is not available')
          const groupId = payload.groupId === undefined ? undefined : parseCoreId(payload.groupId)
          if (groupId !== undefined && !bindings.getGroups().some(group => group.id === groupId))
            throw apiError('NOT_FOUND', 'Group is not available in the active Space')
          return bindings.openEditor({ item, groupId })
        }
        case 'commandCenter.open': return bindings.openCommandCenter()
        case 'network.setMode': {
          const mode = payload.mode
          if (mode !== 'lan' && mode !== 'wan')
            throw apiError('INVALID_ARGUMENT', 'Network mode must be "lan" or "wan"')
          return bindings.setNetworkMode(mode)
        }
        case 'layout.report': {
          // Geometry hint from the sandbox, used to place the Core's monitor
          // layer. Bounded so a misbehaving theme cannot move it absurdly.
          const searchBottom = payload.searchBottom
          if (typeof searchBottom !== 'number' || !Number.isFinite(searchBottom) || searchBottom < 0 || searchBottom > 100000)
            throw apiError('INVALID_ARGUMENT', 'Search bottom must be a finite, non-negative number')
          return bindings.reportLayout({ searchBottom })
        }
        case 'input.forwardKey': {
          const key = payload.key
          if (typeof key !== 'string' || key.length !== 1)
            throw apiError('INVALID_ARGUMENT', 'Forwarded key must be a single character')
          return bindings.forwardKey(key)
        }
        case 'link.open': {
          // Only ever hand the Core an ordinary web link. Anything else
          // (`javascript:`, `data:`, a bare protocol-relative URL) is refused
          // here rather than reaching window.open.
          const url = typeof payload.url === 'string' ? payload.url.trim() : ''
          if (!url) throw apiError('INVALID_ARGUMENT', 'Link URL is required')
          let parsed: URL
          try { parsed = new URL(url) }
          catch { throw apiError('INVALID_ARGUMENT', 'Link URL is not absolute') }
          if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:')
            throw apiError('INVALID_ARGUMENT', 'Only http and https links can be opened')
          return bindings.openLink(parsed.toString())
        }
        case 'ui.openCoreSurface': throw apiError('UNSUPPORTED_CAPABILITY', 'Core surface routing is not available in this adapter')
      }
    },
    async searchItems(query, paging): Promise<ThemeSearchPage> {
      const items = await bindings.searchItems(query)
      const safeItems = items.flatMap(item => toThemeItem(item) || [])
      const offset = paging.cursor ? Number(paging.cursor) : 0
      const pageItems = safeItems.slice(offset, offset + paging.limit)
      return {
        query,
        items: pageItems,
        total: safeItems.length,
        nextCursor: offset + pageItems.length < safeItems.length ? String(offset + pageItems.length) : undefined,
      }
    },
    listSpaces(options) {
      const values: ThemeSpace[] = bindings.getSpaces().flatMap((space) => {
        if (!Number.isSafeInteger(space.id) || space.id <= 0) return []
        return [{
          id: String(space.id),
          name: space.name || '',
          side: space.side,
          pairedSpaceId: space.pairedSpaceId === undefined ? undefined : String(space.pairedSpaceId),
          capabilities: ['space.select'],
        }]
      })
      return page(values, options)
    },
    listGroups(options) {
      const spaceId = bindings.getActiveSpaceId()
      const values: ThemeGroup[] = bindings.getGroups().flatMap((group) => {
        if (!Number.isSafeInteger(group.id) || group.id <= 0) return []
        return [{
          id: String(group.id),
          spaceId: spaceId ? String(spaceId) : '',
          parentId: group.parentId == null ? undefined : String(group.parentId),
          title: group.title || '',
          icon: group.icon,
          itemIds: (group.items || []).flatMap(item => Number.isSafeInteger(Number(item.id)) ? [String(item.id)] : []),
          capabilities: bindings.canWriteGroups?.() ? ['groups.write'] : [],
        }]
      })
      return page(values, options)
    },
    listItems(options) {
      const groupId = options.groupId
      const values = bindings.getGroups()
        .filter(group => groupId === undefined || String(group.id) === groupId)
        .flatMap(group => (group.items || []).flatMap(item => toThemeItem(item) || []))
      return page(values, options)
    },
    getMonitorSnapshot: bindings.getMonitorSnapshot,
    navigate: bindings.navigate,
    getSettings: bindings.getSettings,
    patchSettings: bindings.patchSettings,
    getStorage: bindings.getStorage,
    setStorage: bindings.setStorage,
    removeStorage: bindings.removeStorage,
    openCoreSurface: bindings.openCoreSurface,
    networkFetch: bindings.networkFetch,
    reportDiagnostic: bindings.reportDiagnostic,
  }
}

function findItem(groups: readonly CoreGroup[], value: unknown): CoreItem | undefined {
  let id: number
  try {
    id = parseCoreId(value)
  }
  catch {
    return undefined
  }
  for (const group of groups) {
    const item = group.items?.find(candidate => candidate.id === id)
    if (item) return item
  }
}

function parseCoreId(value: unknown): number {
  const id = typeof value === 'number' ? value : typeof value === 'string' && /^\d+$/.test(value) ? Number(value) : NaN
  if (!Number.isSafeInteger(id) || id <= 0) throw apiError('INVALID_ARGUMENT', 'Expected a positive Core ID')
  return id
}

function toItemWriteInput(payload: Record<string, unknown>, existing: CoreItem | undefined, groups: readonly CoreGroup[]): Record<string, unknown> {
  const groupId = payload.groupId === undefined
    ? existing?.itemIconGroupId
    : parseCoreId(payload.groupId)
  if (!groupId || !groups.some(group => group.id === groupId))
    throw apiError('NOT_FOUND', 'Target group is not available in the active Space')
  const title = payload.title === undefined && existing ? existing.title : parseTitle(payload.title, 20)
  const url = payload.url === undefined && existing ? existing.url : parseHttpUrl(payload.url)
  const icon = payload.icon === undefined ? existing?.icon || { itemType: 4 } : parseItemIcon(payload.icon)
  return {
    ...(existing?.id === undefined ? {} : { id: existing.id }),
    itemIconGroupId: groupId,
    title,
    url,
    lanUrl: payload.lanUrl === undefined && existing ? existing.lanUrl : parseOptionalHttpUrl(payload.lanUrl),
    mobileUrl: payload.mobileUrl === undefined && existing ? existing.mobileUrl : parseOptionalHttpUrl(payload.mobileUrl),
    description: payload.description === undefined && existing ? existing.description : parseOptionalString(payload.description, 2000),
    openMethod: payload.openMethod === undefined ? existing?.openMethod ?? 2 : parseOpenMethod(payload.openMethod),
    sort: payload.sort === undefined ? existing?.sort ?? 0 : parseSort(payload.sort),
    icon: icon || null,
  }
}

function parseTitle(value: unknown, maxLength: number): string {
  if (typeof value !== 'string') throw apiError('INVALID_ARGUMENT', 'Title must be a string')
  const title = value.trim()
  if (!title || [...title].length > maxLength) throw apiError('INVALID_ARGUMENT', `Title must contain 1 to ${maxLength} characters`)
  return title
}

function parseOptionalString(value: unknown, maxLength: number): string {
  if (value === undefined || value === null) return ''
  if (typeof value !== 'string' || value.length > maxLength) throw apiError('INVALID_ARGUMENT', 'Text value is invalid or too long')
  return value
}

function parseHttpUrl(value: unknown): string {
  if (typeof value !== 'string' || value.length > 4096) throw apiError('INVALID_ARGUMENT', 'Item URL must be an HTTP or HTTPS URL')
  try {
    const url = new URL(value)
    if (url.protocol === 'http:' || url.protocol === 'https:') return value
  }
  catch { /* Invalid URL */ }
  throw apiError('INVALID_ARGUMENT', 'Item URL must be an HTTP or HTTPS URL')
}

function parseOptionalHttpUrl(value: unknown): string {
  if (value === undefined || value === null || value === '') return ''
  return parseHttpUrl(value)
}

function parseItemIcon(value: unknown): Panel.ItemIcon | null {
  if (value === null) return null
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw apiError('INVALID_ARGUMENT', 'Item icon is invalid')
  const icon = value as Record<string, unknown>
  if (!Number.isSafeInteger(icon.itemType) || Number(icon.itemType) < 0 || Number(icon.itemType) > 4)
    throw apiError('INVALID_ARGUMENT', 'Item icon type is invalid')
  return {
    itemType: Number(icon.itemType),
    src: parseOptionalString(icon.src, 2048) || undefined,
    fileName: parseOptionalString(icon.fileName, 240) || undefined,
    text: parseOptionalString(icon.text, 80) || undefined,
    backgroundColor: parseOptionalString(icon.backgroundColor, 80) || undefined,
  }
}

function parseOpenMethod(value: unknown): number {
  if (!Number.isSafeInteger(value) || ![1, 2, 3].includes(Number(value))) throw apiError('INVALID_ARGUMENT', 'Open method is invalid')
  return Number(value)
}

function parseSort(value: unknown): number {
  if (!Number.isSafeInteger(value) || Number(value) < 0) throw apiError('INVALID_ARGUMENT', 'Sort value is invalid')
  return Number(value)
}

function parseOptionalParentId(value: unknown, groups: readonly CoreGroup[], selfId?: number): number | null {
  if (value === undefined || value === null || value === '') return null
  const parentId = parseCoreId(value)
  const groupById = new Map(groups.map(group => [group.id, group]))
  if (parentId === selfId || !groupById.has(parentId)) throw apiError('NOT_FOUND', 'Parent group is not available in the active Space')
  let current: number | undefined = parentId
  const visited = new Set<number>()
  while (current !== undefined) {
    if (current === selfId || visited.has(current)) throw apiError('INVALID_ARGUMENT', 'Group parent would create a cycle')
    visited.add(current)
    const ancestorId: number | null | undefined = groupById.get(current)?.parentId
    current = ancestorId == null ? undefined : ancestorId
  }
  return parentId
}

function parseIdList(value: unknown): number[] {
  if (!Array.isArray(value) || value.length > 10000) throw apiError('INVALID_ARGUMENT', 'Expected a bounded ID list')
  return value.map(parseCoreId)
}

function isPermutation(expected: readonly number[], actual: readonly number[]): boolean {
  if (expected.length !== actual.length) return false
  const values = new Set(expected)
  if (values.size !== expected.length) return false
  return actual.length === values.size && new Set(actual).size === actual.length && actual.every(id => values.has(id))
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
