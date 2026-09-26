export const THEME_API_VERSION = '1.0.0' as const
export const THEME_API_DEFAULT_PAGE_SIZE = 50
export const THEME_API_MAX_PAGE_SIZE = 200

export type ThemePermission =
  | 'spaces.read'
  | 'groups.read'
  | 'items.read'
  | 'items.write'
  | 'groups.write'
  | 'monitor.read'
  | 'preferences.read'
  | 'preferences.write'
  | 'theme.storage'
  | 'network.fetch'
  | 'diagnostics.report'
  | 'media.remote'

export type ThemeCommand =
  | 'space.select'
  | 'space.toggleSide'
  | 'item.open'
  | 'item.create'
  | 'item.update'
  | 'item.delete'
  | 'items.reorder'
  | 'group.create'
  | 'group.update'
  | 'group.delete'
  | 'groups.reorder'
  | 'search.submit'
  | 'data.refresh'
  | 'editor.open'
  | 'commandCenter.open'
  | 'ui.openCoreSurface'

export type ThemeCollectionStatus = 'idle' | 'loading' | 'ready' | 'empty' | 'error' | 'stale'

export interface ThemeEnvironment {
  apiVersion: typeof THEME_API_VERSION
  coreVersion: string
  language: string
  colorScheme: 'light' | 'dark'
  reducedMotion: boolean
  online: boolean
  viewport: { width: number; height: number }
  assets?: Readonly<Record<string, string>>
}

export interface ThemeSpace {
  id: string
  name: string
  side?: 'yin' | 'yang'
  pairedSpaceId?: string
  capabilities: readonly string[]
}

export interface ThemeGroup {
  id: string
  spaceId: string
  parentId?: string
  title: string
  icon?: string
  itemIds: readonly string[]
}

export interface ThemeItem {
  id: string
  groupId: string
  title: string
  description?: string
  icon?: unknown
  sort: number
  capabilities: readonly string[]
}

export interface ThemeHomeSnapshot {
  version: number
  status: ThemeCollectionStatus
  error?: { code: string; message: string }
  spaces: readonly ThemeSpace[]
  activeSpaceId?: string
  activeSpaceSide?: 'yin' | 'yang'
  activeSpacePairedId?: string
  activeSpaceCapabilities?: readonly string[]
  groups: readonly ThemeGroup[]
  items: readonly ThemeItem[]
}

export interface ThemeSearchPage {
  query: string
  items: readonly ThemeItem[]
  total: number
  nextCursor?: string
}

export interface ThemePage<T> {
  items: readonly T[]
  total: number
  nextCursor?: string
}

export interface ThemePageOptions {
  limit?: number
  cursor?: string
}

export interface ThemeMonitorSnapshot {
  capturedAt: string
  cpu?: { coreCount: number; model: string; usagePercent: number }
  memory?: { total: number; used: number; free: number; usedPercent: number }
  network?: readonly { name: string; bytesSent: number; bytesRecv: number }[]
}

export type ThemeEventName =
  | 'state.updated'
  | 'environment.changed'
  | 'session.changed'
  | 'space.changed'
  | 'groups.changed'
  | 'items.changed'
  | 'preferences.changed'
  | 'monitor.updated'
  | 'navigation.changed'
  | 'theme.disposing'

export interface ThemeEventEnvelope<T = unknown> {
  contextVersion: number
  sequence: number
  payload: T
}

export interface ThemeApiError extends Error {
  code:
    | 'PERMISSION_DENIED'
    | 'NOT_FOUND'
    | 'INVALID_ARGUMENT'
    | 'OFFLINE'
    | 'CONFLICT'
    | 'RATE_LIMITED'
    | 'ABORTED'
    | 'TIMEOUT'
    | 'UNSUPPORTED_CAPABILITY'
    | 'RUNTIME_UNAVAILABLE'
}

export interface ThemeAPI {
  environment: { get: () => ThemeEnvironment }
  state: { getSnapshot: () => ThemeHomeSnapshot }
  events: { subscribe: <T = unknown>(name: ThemeEventName, listener: (event: ThemeEventEnvelope<T>) => void) => () => void }
  commands: { execute: <T = unknown>(command: ThemeCommand, payload?: Record<string, unknown>) => Promise<T> }
  search: { query: (query: string, options?: { limit?: number; cursor?: string }) => Promise<ThemeSearchPage> }
  spaces: { list: (options?: ThemePageOptions) => Promise<ThemePage<ThemeSpace>> }
  groups: { list: (options?: ThemePageOptions) => Promise<ThemePage<ThemeGroup>> }
  items: { list: (options?: ThemePageOptions & { groupId?: string }) => Promise<ThemePage<ThemeItem>> }
  monitor: { getSnapshot: () => Promise<ThemeMonitorSnapshot> }
  navigation: {
    get: () => { spaceId?: string; view: string }
    navigate: (destination: { view: string; spaceId?: string }) => Promise<void>
  }
  assets: { resolve: (path: string) => string }
  settings: { get: () => Promise<Record<string, unknown>>; patch: (value: Record<string, unknown>) => Promise<void> }
  storage: {
    get: (key: string) => Promise<unknown>
    set: (key: string, value: unknown) => Promise<void>
    remove: (key: string) => Promise<void>
  }
  ui: { openCoreSurface: (surface: string, payload?: Record<string, unknown>) => Promise<void> }
  network: { fetch: (input: string, init?: { method?: string; headers?: Record<string, string>; body?: string }) => Promise<{ status: number; headers: Record<string, string>; body: unknown }> }
  diagnostics: { report: (entry: { level: 'info' | 'warn' | 'error'; message: string; data?: Record<string, unknown> }) => Promise<void> }
}

export interface ThemeMountedView {
  update?: (snapshot: ThemeHomeSnapshot) => void | Promise<void>
  unmount: () => void | Promise<void>
}

export type ThemeViewFactory = (
  element: HTMLElement,
  api: ThemeAPI,
  snapshot: ThemeHomeSnapshot,
) => ThemeMountedView | Promise<ThemeMountedView>

export type ThemeComponentFactory = (
  element: HTMLElement,
  api: ThemeAPI,
  snapshot: ThemeHomeSnapshot,
) => ThemeMountedView | Promise<ThemeMountedView>

export interface ThemeDefinition {
  views?: Partial<Record<'home' | 'public-home' | 'theme-settings' | 'theme-page', ThemeViewFactory>>
  regions?: Record<string, ThemeViewFactory>
  components?: Record<string, ThemeComponentFactory>
  dispose?: () => void | Promise<void>
}

export interface ThemeModule {
  apiVersion: typeof THEME_API_VERSION
  setup: (api: ThemeAPI) => ThemeDefinition | Promise<ThemeDefinition>
}

export interface ThemeApiRequest {
  protocol: 'yin-theme-api'
  version: 1
  requestId: string
  contextVersion: number
  method: 'commands.execute' | 'search.query' | 'spaces.list' | 'groups.list' | 'items.list' | 'monitor.getSnapshot' | 'navigation.navigate' | 'settings.get' | 'settings.patch' | 'storage.get' | 'storage.set' | 'storage.remove' | 'ui.openCoreSurface' | 'network.fetch' | 'diagnostics.report'
  payload?: Record<string, unknown>
}

export interface ThemeApiClientHost {
  request: (request: ThemeApiRequest) => Promise<unknown>
  getSnapshot: () => ThemeHomeSnapshot
  getEnvironment: () => ThemeEnvironment
  getNavigation?: () => { view: string; spaceId?: string }
}

export function createThemeApiClient(host: ThemeApiClientHost) {
  let snapshot = host.getSnapshot()
  let environment = host.getEnvironment()
  let lastEventSequence = 0
  const listeners = new Map<string, Set<(event: ThemeEventEnvelope) => void>>()

  function createRequestId() {
    const webCrypto = typeof globalThis !== 'undefined' ? globalThis.crypto : undefined
    if (typeof webCrypto?.randomUUID === 'function') return webCrypto.randomUUID()
    if (typeof webCrypto?.getRandomValues === 'function') {
      const bytes = new Uint8Array(16)
      webCrypto.getRandomValues(bytes)
      return [...bytes].map(value => value.toString(16).padStart(2, '0')).join('')
    }
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`
  }

  function request(method: ThemeApiRequest['method'], payload?: Record<string, unknown>) {
    return host.request({
      protocol: 'yin-theme-api',
      version: 1,
      requestId: createRequestId(),
      contextVersion: snapshot.version,
      method,
      payload,
    })
  }

  function clone<T>(value: T): T {
    return structuredClone(value)
  }

  return {
    api: {
      environment: { get: () => clone(environment) },
      state: { getSnapshot: () => clone(snapshot) },
      events: {
        subscribe: <T = unknown>(name: ThemeEventName, listener: (event: ThemeEventEnvelope<T>) => void) => {
          const handlers = listeners.get(name) || new Set()
          const callback = listener as (event: ThemeEventEnvelope) => void
          handlers.add(callback)
          listeners.set(name, handlers)
          return () => { handlers.delete(callback) }
        },
      },
      commands: { execute: <T>(command: ThemeCommand, payload: Record<string, unknown> = {}) => request('commands.execute', { command, arguments: payload }) as Promise<T> },
      search: { query: (query: string, options: { limit?: number; cursor?: string } = {}) => request('search.query', { query, ...options }) as Promise<ThemeSearchPage> },
      spaces: { list: (options: ThemePageOptions = {}) => request('spaces.list', { ...options }) as Promise<ThemePage<ThemeSpace>> },
      groups: { list: (options: ThemePageOptions = {}) => request('groups.list', { ...options }) as Promise<ThemePage<ThemeGroup>> },
      items: { list: (options: ThemePageOptions & { groupId?: string } = {}) => request('items.list', { ...options }) as Promise<ThemePage<ThemeItem>> },
      monitor: { getSnapshot: () => request('monitor.getSnapshot') as Promise<ThemeMonitorSnapshot> },
      navigation: {
        get: () => host.getNavigation ? host.getNavigation() : { view: 'home', spaceId: snapshot.activeSpaceId },
        navigate: (destination: { view: string; spaceId?: string }) => request('navigation.navigate', destination).then(() => undefined),
      },
      assets: {
        resolve: (path: string) => {
          const url = environment.assets?.[path]
          if (!url) throw new Error('Theme asset is not declared')
          return url
        },
      },
      settings: {
        get: () => request('settings.get') as Promise<Record<string, unknown>>,
        patch: (value: Record<string, unknown>) => request('settings.patch', { value }).then(() => undefined),
      },
      storage: {
        get: (key: string) => request('storage.get', { key }),
        set: (key: string, value: unknown) => request('storage.set', { key, value }).then(() => undefined),
        remove: (key: string) => request('storage.remove', { key }).then(() => undefined),
      },
      ui: { openCoreSurface: (surface: string, payload: Record<string, unknown> = {}) => request('ui.openCoreSurface', { surface, payload }).then(() => undefined) },
      network: { fetch: (input: string, init = {}) => request('network.fetch', { input, init }) as Promise<{ status: number; headers: Record<string, string>; body: unknown }> },
      diagnostics: { report: (entry: { level: 'info' | 'warn' | 'error'; message: string; data?: Record<string, unknown> }) => request('diagnostics.report', entry).then(() => undefined) },
    } satisfies ThemeAPI,
    updateSnapshot(next: ThemeHomeSnapshot) {
      snapshot = clone(next)
    },
    updateEnvironment(next: ThemeEnvironment) {
      environment = clone(next)
    },
    emit(name: ThemeEventName, event: ThemeEventEnvelope) {
      if (!['state.updated', 'environment.changed', 'session.changed', 'space.changed', 'groups.changed', 'items.changed', 'preferences.changed', 'monitor.updated', 'navigation.changed', 'theme.disposing'].includes(name)
        || !Number.isSafeInteger(event.contextVersion)
        || event.contextVersion !== snapshot.version
        || !Number.isSafeInteger(event.sequence)
        || event.sequence <= lastEventSequence)
        return false
      lastEventSequence = event.sequence
      for (const listener of listeners.get(name) || []) {
        try { listener(clone(event)) }
        catch { /* Theme listeners are isolated from the host runtime. */ }
      }
      return true
    },
    dispose() {
      listeners.clear()
    },
  }
}

export function isThemeApiRequest(value: unknown): value is ThemeApiRequest {
  if (!value || typeof value !== 'object') return false
  const request = value as Partial<ThemeApiRequest>
  return request.protocol === 'yin-theme-api'
    && request.version === 1
    && typeof request.requestId === 'string'
    && request.requestId.length >= 8
    && request.requestId.length <= 80
    && Number.isSafeInteger(request.contextVersion)
    && request.contextVersion! >= 0
    && typeof request.method === 'string'
    && ['commands.execute', 'search.query', 'spaces.list', 'groups.list', 'items.list', 'monitor.getSnapshot', 'navigation.navigate', 'settings.get', 'settings.patch', 'storage.get', 'storage.set', 'storage.remove', 'ui.openCoreSurface', 'network.fetch', 'diagnostics.report'].includes(request.method)
    && (request.payload === undefined || isJsonRecord(request.payload))
}

export function isJsonRecord(value: unknown): value is Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return false
  try {
    return JSON.stringify(value).length <= 1_048_576
  }
  catch {
    return false
  }
}

export function isThemeEventName(value: unknown): value is ThemeEventName {
  return typeof value === 'string' && [
    'state.updated',
    'environment.changed',
    'session.changed',
    'space.changed',
    'groups.changed',
    'items.changed',
    'preferences.changed',
    'monitor.updated',
    'navigation.changed',
    'theme.disposing',
  ].includes(value)
}
