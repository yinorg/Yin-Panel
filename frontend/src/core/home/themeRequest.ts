import type { ThemeApiHandlers } from '../../theme/api/dispatcher'
import type { ThemeCommand } from '../../theme/api/v1'

export async function executeHomeThemeRequest(handlers: ThemeApiHandlers, value: unknown): Promise<unknown> {
  if (!isRecord(value) || typeof value.method !== 'string')
    throw apiError('INVALID_ARGUMENT', 'Invalid Theme API request')

  switch (value.method) {
    case 'commands.execute': {
      if (typeof value.command !== 'string') throw apiError('INVALID_ARGUMENT', 'Theme command is required')
      return handlers.executeCommand(value.command as ThemeCommand, isRecord(value.payload) ? value.payload : {})
    }
    case 'search.query': {
      if (typeof value.query !== 'string' || value.query.length > 200)
        throw apiError('INVALID_ARGUMENT', 'Search query must be a string of at most 200 characters')
      const limit = value.limit === undefined ? 50 : value.limit
      if (!Number.isSafeInteger(limit) || Number(limit) < 1 || Number(limit) > 100)
        throw apiError('INVALID_ARGUMENT', 'Search page limit must be between 1 and 100')
      const cursor = value.cursor
      if (cursor !== undefined && (typeof cursor !== 'string' || !/^\d{1,12}$/.test(cursor)))
        throw apiError('INVALID_ARGUMENT', 'Search cursor is invalid')
      return handlers.searchItems(value.query, { limit: Number(limit), cursor })
    }
    case 'spaces.list':
    case 'groups.list':
    case 'items.list': {
      const options = parsePageOptions(value)
      if (value.method === 'spaces.list') return handlers.listSpaces(options)
      if (value.method === 'groups.list') return handlers.listGroups(options)
      if (value.groupId !== undefined && (typeof value.groupId !== 'string' || !/^\d{1,16}$/.test(value.groupId)))
        throw apiError('INVALID_ARGUMENT', 'Group ID is invalid')
      return handlers.listItems({ ...options, groupId: value.groupId as string | undefined })
    }
    case 'monitor.getSnapshot': return handlers.getMonitorSnapshot()
    case 'navigation.navigate':
      if (!isRecord(value.destination) || typeof value.destination.view !== 'string')
        throw apiError('INVALID_ARGUMENT', 'Invalid navigation destination')
      return handlers.navigate({
        view: value.destination.view,
        spaceId: typeof value.destination.spaceId === 'string' ? value.destination.spaceId : undefined,
      })
    case 'settings.get': return handlers.getSettings()
    case 'settings.patch':
      if (!isRecord(value.value)) throw apiError('INVALID_ARGUMENT', 'Settings patch must be an object')
      return handlers.patchSettings(value.value)
    case 'storage.get':
    case 'storage.remove':
    case 'storage.set': {
      if (typeof value.key !== 'string' || value.key.length < 1 || value.key.length > 160)
        throw apiError('INVALID_ARGUMENT', 'Invalid theme storage key')
      if (value.method === 'storage.get') return handlers.getStorage(value.key)
      if (value.method === 'storage.remove') return handlers.removeStorage(value.key)
      return handlers.setStorage(value.key, value.value)
    }
    default:
      throw apiError('UNSUPPORTED_CAPABILITY', 'Theme API operation is unavailable')
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function parsePageOptions(value: Record<string, unknown>) {
  const limit = value.limit === undefined ? 50 : value.limit
  if (!Number.isSafeInteger(limit) || Number(limit) < 1 || Number(limit) > 100)
    throw apiError('INVALID_ARGUMENT', 'List page limit must be between 1 and 100')
  const cursor = value.cursor
  if (cursor !== undefined && (typeof cursor !== 'string' || !/^\d{1,12}$/.test(cursor)))
    throw apiError('INVALID_ARGUMENT', 'List cursor is invalid')
  return { limit: Number(limit), cursor }
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
