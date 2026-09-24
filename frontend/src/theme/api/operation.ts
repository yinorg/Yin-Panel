// @ts-expect-error Node's strip-types runner requires an explicit source extension.
import { THEME_API_DEFAULT_PAGE_SIZE, THEME_API_MAX_PAGE_SIZE } from '../../../packages/theme-sdk/src/index.ts'
import type { ThemeApiRequest, ThemeCommand } from './v1'

export type ThemeApiMethod = ThemeApiRequest['method']

export interface ThemeApiOperationHandlers {
  executeCommand: (command: ThemeCommand, payload: Record<string, unknown>) => Promise<unknown> | unknown
  searchItems: (query: string, options: { limit: number; cursor?: string }) => Promise<unknown> | unknown
  listSpaces: (options: { limit: number; cursor?: string }) => Promise<unknown> | unknown
  listGroups: (options: { limit: number; cursor?: string }) => Promise<unknown> | unknown
  listItems: (options: { limit: number; cursor?: string; groupId?: string }) => Promise<unknown> | unknown
  getMonitorSnapshot: () => Promise<unknown> | unknown
  navigate: (destination: { view: string; spaceId?: string }) => Promise<unknown> | unknown
  getSettings: () => Promise<Record<string, unknown>> | Record<string, unknown>
  patchSettings: (value: Record<string, unknown>) => Promise<unknown> | unknown
  getStorage: (key: string) => Promise<unknown> | unknown
  setStorage: (key: string, value: unknown) => Promise<unknown> | unknown
  removeStorage: (key: string) => Promise<unknown> | unknown
}

export async function executeThemeApiOperation(handlers: ThemeApiOperationHandlers, request: Pick<ThemeApiRequest, 'method' | 'payload'>): Promise<unknown> {
  const payload = request.payload || {}
  switch (request.method) {
    case 'commands.execute': {
      const command = payload.command
      if (typeof command !== 'string' || !isThemeCommand(command)) throw apiError('INVALID_ARGUMENT', 'Unknown theme command')
      return handlers.executeCommand(command, payload.arguments && isRecord(payload.arguments) ? payload.arguments : {})
    }
    case 'search.query': {
      if (typeof payload.query !== 'string' || payload.query.length > 200)
        throw apiError('INVALID_ARGUMENT', 'Search query must be a string of at most 200 characters')
      const { limit, cursor } = parsePageOptions(payload, 'Search')
      return handlers.searchItems(payload.query, { limit, cursor })
    }
    case 'spaces.list':
    case 'groups.list':
    case 'items.list': {
      const { limit, cursor } = parsePageOptions(payload, 'List')
      if (request.method === 'spaces.list') return handlers.listSpaces({ limit, cursor })
      if (request.method === 'groups.list') return handlers.listGroups({ limit, cursor })
      const groupId = payload.groupId
      if (groupId !== undefined && (typeof groupId !== 'string' || !/^\d{1,16}$/.test(groupId)))
        throw apiError('INVALID_ARGUMENT', 'Group ID is invalid')
      return handlers.listItems({ limit, cursor, groupId: groupId as string | undefined })
    }
    case 'monitor.getSnapshot': return handlers.getMonitorSnapshot()
    case 'navigation.navigate': {
      if (typeof payload.view !== 'string' || payload.view.length > 100)
        throw apiError('INVALID_ARGUMENT', 'Invalid navigation destination')
      const spaceId = typeof payload.spaceId === 'string' ? payload.spaceId : undefined
      return handlers.navigate({ view: payload.view, spaceId })
    }
    case 'settings.get': return handlers.getSettings()
    case 'settings.patch':
      if (!isRecord(payload.value)) throw apiError('INVALID_ARGUMENT', 'Settings patch must be an object')
      return handlers.patchSettings(payload.value)
    case 'storage.get':
    case 'storage.remove':
    case 'storage.set': {
      const key = payload.key
      if (typeof key !== 'string' || key.length < 1 || key.length > 160)
        throw apiError('INVALID_ARGUMENT', 'Invalid theme storage key')
      if (request.method === 'storage.get') return handlers.getStorage(key)
      if (request.method === 'storage.remove') return handlers.removeStorage(key)
      return handlers.setStorage(key, payload.value)
    }
  }
}

function parsePageOptions(payload: Record<string, unknown>, label: 'Search' | 'List') {
  const limit = payload.limit === undefined ? THEME_API_DEFAULT_PAGE_SIZE : payload.limit
  if (!Number.isSafeInteger(limit) || Number(limit) < 1 || Number(limit) > THEME_API_MAX_PAGE_SIZE)
    throw apiError('INVALID_ARGUMENT', `${label} page limit must be between 1 and ${THEME_API_MAX_PAGE_SIZE}`)
  const cursor = payload.cursor
  if (cursor !== undefined && (typeof cursor !== 'string' || !/^\d{1,12}$/.test(cursor)))
    throw apiError('INVALID_ARGUMENT', `${label} cursor is invalid`)
  return { limit: Number(limit), cursor: cursor as string | undefined }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

export function isThemeCommand(value: string): value is ThemeCommand {
  return ['space.select', 'space.toggleSide', 'item.open', 'item.create', 'item.update', 'item.delete', 'items.reorder', 'group.create', 'group.update', 'group.delete', 'groups.reorder', 'search.submit', 'data.refresh', 'editor.open', 'commandCenter.open', 'ui.openCoreSurface'].includes(value)
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
