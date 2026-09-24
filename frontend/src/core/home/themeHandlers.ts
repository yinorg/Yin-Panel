import type { ThemeApiHandlers } from '../../theme/api/dispatcher'
import type { ThemeCommand, ThemeGroup, ThemeItem, ThemePage, ThemeSearchPage, ThemeSpace } from '../../theme/api/v1'

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
  selectSpace: (spaceId: number) => Promise<unknown> | unknown
  openItem: (item: CoreItem) => Promise<unknown> | unknown
  openEditor: (input: { item?: CoreItem; groupId?: number }) => Promise<unknown> | unknown
  openCommandCenter: () => Promise<unknown> | unknown
  toggleSide: () => Promise<unknown> | unknown
  refresh: () => Promise<unknown> | unknown
  searchItems: (query: string) => Promise<readonly Panel.ItemInfo[]> | readonly Panel.ItemInfo[]
  getMonitorSnapshot: () => Promise<unknown> | unknown
  submitSearch: (query: string) => Promise<unknown> | unknown
  navigate: (destination: { view: string; spaceId?: string }) => Promise<unknown> | unknown
  getSettings: () => Promise<Record<string, unknown>> | Record<string, unknown>
  patchSettings: (value: Record<string, unknown>) => Promise<unknown> | unknown
  getStorage: (key: string) => Promise<unknown> | unknown
  setStorage: (key: string, value: unknown) => Promise<unknown> | unknown
  removeStorage: (key: string) => Promise<unknown> | unknown
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
      icon: item.icon ? {
        itemType: item.icon.itemType,
        src: item.icon.src,
        fileName: item.icon.fileName,
        text: item.icon.text,
        backgroundColor: item.icon.backgroundColor,
      } : undefined,
      sort: item.sort ?? 0,
      capabilities: ['item.open'],
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
        case 'item.update':
        case 'item.delete':
        case 'items.reorder':
        case 'group.create':
        case 'group.update':
        case 'group.delete':
        case 'groups.reorder':
          throw apiError('UNSUPPORTED_CAPABILITY', 'This home adapter does not expose write operations yet')
        case 'search.submit': return bindings.submitSearch(typeof payload.query === 'string' ? payload.query : '')
        case 'data.refresh': return bindings.refresh()
        case 'editor.open': {
          const itemId = payload.itemId
          const item = itemId === undefined ? undefined : findItem(bindings.getGroups(), itemId)
          if (itemId !== undefined && !item) throw apiError('NOT_FOUND', 'Item is not available')
          const groupId = payload.groupId === undefined ? undefined : parseCoreId(payload.groupId)
          return bindings.openEditor({ item, groupId })
        }
        case 'commandCenter.open': return bindings.openCommandCenter()
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

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
