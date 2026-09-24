import { isThemeApiRequest, type ThemeApiRequest, type ThemePermission } from './v1'
import { requiredCommandPermission } from './permissions'
import { executeThemeApiOperation, isThemeCommand, type ThemeApiOperationHandlers } from './operation'

export type ThemeApiMethod = ThemeApiRequest['method']

export type ThemeApiHandlers = ThemeApiOperationHandlers

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
  getContextVersion: () => number
  getPermissions: () => ReadonlySet<ThemePermission>
  handlers: ThemeApiHandlers
  maxInFlight?: number
  timeoutMs?: number
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
    const operation = executeThemeApiOperation(options.handlers, request)
    void operation.then(
      () => { inFlight -= 1 },
      () => { inFlight -= 1 },
    )
    try {
      const result = await Promise.race([
        operation,
        new Promise<never>((_, reject) => {
          timer = setTimeout(() => reject(Object.assign(new Error('Theme API request timed out'), { code: 'TIMEOUT' })), timeoutMs)
        }),
      ])
      if (isContextScopedRead(request) && request.contextVersion !== options.getContextVersion())
        return failure(request, 'ABORTED', 'Theme context changed while the request was in progress')
      if (permission && !options.getPermissions().has(permission))
        return failure(request, 'PERMISSION_DENIED', `Theme permission was revoked: ${permission}`)
      return { protocol: 'yin-theme-api', version: 1, requestId: request.requestId, contextVersion: request.contextVersion, ok: true, result }
    }
    catch (error) {
      const code = error && typeof error === 'object' && 'code' in error && typeof error.code === 'string' ? error.code : 'RUNTIME_UNAVAILABLE'
      const message = error instanceof Error ? error.message : 'Theme API request failed'
      return failure(request, code, message)
    }
    finally {
      if (timer) clearTimeout(timer)
    }
  }
}

function isContextScopedRead(request: ThemeApiRequest): boolean {
  return request.method === 'search.query'
    || request.method === 'spaces.list'
    || request.method === 'groups.list'
    || request.method === 'items.list'
    || request.method === 'monitor.getSnapshot'
    || request.method === 'settings.get'
    || request.method === 'storage.get'
}

function requiredPermission(request: ThemeApiRequest): ThemePermission | undefined {
  if (request.method === 'settings.get') return 'preferences.read'
  if (request.method === 'settings.patch') return 'preferences.write'
  if (request.method === 'commands.execute') {
    const command = request.payload?.command
    if (typeof command !== 'string' || !isThemeCommand(command)) return 'items.read'
    return requiredCommandPermission(command)
  }
  if (request.method === 'search.query') return 'items.read'
  if (request.method === 'spaces.list') return 'spaces.read'
  if (request.method === 'groups.list') return 'groups.read'
  if (request.method === 'items.list') return 'items.read'
  if (request.method === 'monitor.getSnapshot') return 'monitor.read'
  if (request.method === 'storage.get' || request.method === 'storage.set' || request.method === 'storage.remove')
    return 'theme.storage'
  return undefined
}
