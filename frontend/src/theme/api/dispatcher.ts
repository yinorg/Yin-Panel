import { isThemeApiRequest, type ThemeApiRequest, type ThemeCommand, type ThemePermission } from './v1'

export type ThemeApiMethod = ThemeApiRequest['method']

export interface ThemeApiHandlers {
  executeCommand(command: ThemeCommand, payload: Record<string, unknown>): Promise<unknown> | unknown
  navigate(destination: { view: string; spaceId?: string }): Promise<unknown> | unknown
  getSettings(): Promise<Record<string, unknown>> | Record<string, unknown>
  patchSettings(value: Record<string, unknown>): Promise<unknown> | unknown
  getStorage(key: string): Promise<unknown> | unknown
  setStorage(key: string, value: unknown): Promise<unknown> | unknown
  removeStorage(key: string): Promise<unknown> | unknown
}

export interface ThemeApiResponse {
  protocol: 'yin-theme-api'
  version: 1
  requestId: string
  contextVersion: number
  ok: boolean
  result?: unknown
  error?: { code: string; message: string }
}

export interface ThemeApiDispatcherOptions {
  getContextVersion(): number
  getPermissions(): ReadonlySet<ThemePermission>
  handlers: ThemeApiHandlers
  maxInFlight?: number
  timeoutMs?: number
}

const commandPermission: Partial<Record<ThemeCommand, ThemePermission>> = {
  'space.select': 'spaces.read',
  'space.toggleSide': 'spaces.read',
  'item.open': 'items.read',
  'item.create': 'items.write',
  'item.update': 'items.write',
  'item.delete': 'items.write',
  'items.reorder': 'items.write',
  'group.create': 'groups.write',
  'group.update': 'groups.write',
  'group.delete': 'groups.write',
  'groups.reorder': 'groups.write',
  'search.submit': 'items.read',
  'data.refresh': 'groups.read',
  'editor.open': 'items.write',
}

function failure(request: Pick<ThemeApiRequest, 'requestId' | 'contextVersion'>, code: string, message: string): ThemeApiResponse {
  return { protocol: 'yin-theme-api', version: 1, requestId: request.requestId, contextVersion: request.contextVersion, ok: false, error: { code, message } }
}

export function createThemeApiDispatcher(options: ThemeApiDispatcherOptions) {
  let inFlight = 0
  const maxInFlight = options.maxInFlight ?? 32
  const timeoutMs = options.timeoutMs ?? 15_000

  return async function dispatch(value: unknown): Promise<ThemeApiResponse | null> {
    if (!isThemeApiRequest(value)) return null
    const request = value
    if (request.contextVersion !== options.getContextVersion())
      return failure(request, 'ABORTED', 'Theme context is no longer current')
    if (inFlight >= maxInFlight)
      return failure(request, 'RATE_LIMITED', 'Too many theme requests are in progress')

    const permission = requiredPermission(request)
    if (permission && !options.getPermissions().has(permission))
      return failure(request, 'PERMISSION_DENIED', `Theme permission required: ${permission}`)

    inFlight += 1
    let timer: ReturnType<typeof setTimeout> | undefined
    try {
      const result = await Promise.race([
        runHandler(request, options.handlers),
        new Promise<never>((_, reject) => {
          timer = setTimeout(() => reject(Object.assign(new Error('Theme API request timed out'), { code: 'TIMEOUT' })), timeoutMs)
        }),
      ])
      if (request.contextVersion !== options.getContextVersion())
        return failure(request, 'ABORTED', 'Theme context changed while the request was running')
      return { protocol: 'yin-theme-api', version: 1, requestId: request.requestId, contextVersion: request.contextVersion, ok: true, result }
    }
    catch (error) {
      const code = error && typeof error === 'object' && 'code' in error && typeof error.code === 'string' ? error.code : 'RUNTIME_UNAVAILABLE'
      const message = error instanceof Error ? error.message : 'Theme API request failed'
      return failure(request, code, message)
    }
    finally {
      if (timer) clearTimeout(timer)
      inFlight -= 1
    }
  }
}

function requiredPermission(request: ThemeApiRequest): ThemePermission | undefined {
  if (request.method === 'commands.execute') {
    const command = request.payload?.command
    if (typeof command !== 'string') return 'items.read'
    return commandPermission[command as ThemeCommand]
  }
  if (request.method === 'storage.get' || request.method === 'storage.set' || request.method === 'storage.remove')
    return 'theme.storage'
  return undefined
}

async function runHandler(request: ThemeApiRequest, handlers: ThemeApiHandlers): Promise<unknown> {
  const payload = request.payload || {}
  switch (request.method) {
    case 'commands.execute': {
      const command = payload.command
      if (typeof command !== 'string' || !isThemeCommand(command)) throw Object.assign(new Error('Unknown theme command'), { code: 'INVALID_ARGUMENT' })
      return handlers.executeCommand(command, payload.arguments && isRecord(payload.arguments) ? payload.arguments : {})
    }
    case 'navigation.navigate': {
      if (typeof payload.view !== 'string' || payload.view.length > 100) throw Object.assign(new Error('Invalid navigation destination'), { code: 'INVALID_ARGUMENT' })
      const spaceId = typeof payload.spaceId === 'string' ? payload.spaceId : undefined
      return handlers.navigate({ view: payload.view, spaceId })
    }
    case 'settings.get': return handlers.getSettings()
    case 'settings.patch':
      if (!isRecord(payload.value)) throw Object.assign(new Error('Settings patch must be an object'), { code: 'INVALID_ARGUMENT' })
      return handlers.patchSettings(payload.value)
    case 'storage.get':
    case 'storage.remove':
    case 'storage.set': {
      const key = payload.key
      if (typeof key !== 'string' || key.length < 1 || key.length > 160) throw Object.assign(new Error('Invalid theme storage key'), { code: 'INVALID_ARGUMENT' })
      if (request.method === 'storage.get') return handlers.getStorage(key)
      if (request.method === 'storage.remove') return handlers.removeStorage(key)
      return handlers.setStorage(key, payload.value)
    }
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function isThemeCommand(value: string): value is ThemeCommand {
  return ['space.select', 'space.toggleSide', 'item.open', 'item.create', 'item.update', 'item.delete', 'items.reorder', 'group.create', 'group.update', 'group.delete', 'groups.reorder', 'search.submit', 'data.refresh', 'editor.open', 'commandCenter.open', 'ui.openCoreSurface'].includes(value)
}
