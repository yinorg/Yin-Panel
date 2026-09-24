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

export interface ThemeDefinition {
  views?: Partial<Record<'home' | 'public-home' | 'theme-settings' | 'theme-page', ThemeViewFactory>>
  regions?: Record<string, ThemeViewFactory>
  components?: Record<string, unknown>
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
  method: 'commands.execute' | 'search.query' | 'spaces.list' | 'groups.list' | 'items.list' | 'monitor.getSnapshot' | 'navigation.navigate' | 'settings.get' | 'settings.patch' | 'storage.get' | 'storage.set' | 'storage.remove'
  payload?: Record<string, unknown>
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
    && ['commands.execute', 'search.query', 'spaces.list', 'groups.list', 'items.list', 'monitor.getSnapshot', 'navigation.navigate', 'settings.get', 'settings.patch', 'storage.get', 'storage.set', 'storage.remove'].includes(request.method)
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
