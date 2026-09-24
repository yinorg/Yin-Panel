import type { ThemeApiHandlers } from '../../theme/api/dispatcher'
import type { ThemeCommand } from '../../theme/api/v1'

interface CoreSpace {
  id: number
}

type CoreItem = Panel.ItemInfo

interface CoreGroup {
  id: number
  items?: readonly CoreItem[]
}

export interface HomeThemeActionBindings {
  getSpaces(): readonly CoreSpace[]
  getGroups(): readonly CoreGroup[]
  selectSpace(spaceId: number): Promise<unknown> | unknown
  openItem(item: CoreItem): Promise<unknown> | unknown
  openEditor(input: { item?: CoreItem; groupId?: number }): Promise<unknown> | unknown
  openCommandCenter(): Promise<unknown> | unknown
  toggleSide(): Promise<unknown> | unknown
  refresh(): Promise<unknown> | unknown
  submitSearch(query: string): Promise<unknown> | unknown
  navigate(destination: { view: string; spaceId?: string }): Promise<unknown> | unknown
  getSettings(): Promise<Record<string, unknown>> | Record<string, unknown>
  patchSettings(value: Record<string, unknown>): Promise<unknown> | unknown
  getStorage(key: string): Promise<unknown> | unknown
  setStorage(key: string, value: unknown): Promise<unknown> | unknown
  removeStorage(key: string): Promise<unknown> | unknown
}

export function createHomeThemeHandlers(bindings: HomeThemeActionBindings): ThemeApiHandlers {
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
