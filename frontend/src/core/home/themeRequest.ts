// @ts-expect-error Node's strip-types runner requires an explicit source extension.
import { executeThemeApiOperation, type ThemeApiOperationHandlers, type ThemeApiMethod } from '../../theme/api/operation.ts'

export async function executeHomeThemeRequest(handlers: ThemeApiOperationHandlers, value: unknown): Promise<unknown> {
  if (!isRecord(value) || typeof value.method !== 'string')
    throw apiError('INVALID_ARGUMENT', 'Invalid Theme API request')
  const method = value.method
  if (!isThemeApiMethod(method)) throw apiError('UNSUPPORTED_CAPABILITY', 'Theme API operation is unavailable')
  return executeThemeApiOperation(handlers, { method, payload: normalizePayload(method, value) })
}

function normalizePayload(method: ThemeApiMethod, value: Record<string, unknown>): Record<string, unknown> | undefined {
  switch (method) {
    case 'commands.execute':
      if (typeof value.command !== 'string') throw apiError('INVALID_ARGUMENT', 'Theme command is required')
      return { command: value.command, arguments: isRecord(value.payload) ? value.payload : {} }
    case 'search.query':
      return { query: value.query, limit: value.limit, cursor: value.cursor }
    case 'spaces.list':
    case 'groups.list':
      return { limit: value.limit, cursor: value.cursor }
    case 'items.list':
      return { limit: value.limit, cursor: value.cursor, groupId: value.groupId }
    case 'navigation.navigate':
      if (!isRecord(value.destination)) throw apiError('INVALID_ARGUMENT', 'Invalid navigation destination')
      return value.destination
    case 'settings.patch':
      return { value: value.value }
    case 'storage.get':
    case 'storage.remove':
      return { key: value.key }
    case 'storage.set':
      return { key: value.key, value: value.value }
    case 'ui.openCoreSurface':
      return { surface: value.surface, payload: value.payload }
    case 'network.fetch':
      return { input: value.input, init: value.init }
    case 'diagnostics.report':
      return { level: value.level, message: value.message, data: value.data }
    case 'monitor.getSnapshot':
    case 'settings.get':
      return undefined
  }
}

function isThemeApiMethod(value: string): value is ThemeApiMethod {
  return ['commands.execute', 'search.query', 'spaces.list', 'groups.list', 'items.list', 'monitor.getSnapshot', 'navigation.navigate', 'settings.get', 'settings.patch', 'storage.get', 'storage.set', 'storage.remove', 'ui.openCoreSurface', 'network.fetch', 'diagnostics.report'].includes(value)
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
