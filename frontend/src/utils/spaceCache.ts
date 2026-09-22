import { ls, ss } from './storage/local'

export const HOME_CACHE_VERSION = 2
export const HOME_CACHE_MAX_AGE_MS = 7 * 24 * 60 * 60 * 1000

interface CacheEnvelope<T> {
  version: number
  updatedAt: number
  complete: boolean
  data: T
}

export interface SpaceCache {
  groups: any[]
  items: Record<string, any[]>
  searchConfig?: any
  updatedAt?: number
}

const spacesKey = (userId?: number) => `yin-panel-spaces-cache:${userId || 'anonymous'}`
const spaceKey = (spaceId: number, userId?: number) => `yin-panel-space-cache:${userId || 'anonymous'}:${spaceId}`

function storage(sessionOnly = false) {
  return sessionOnly ? ss : ls
}

function read<T>(key: string, sessionOnly = false): T | null {
  const cached = storage(sessionOnly).get(key) as CacheEnvelope<T> | null
  if (!cached || typeof cached !== 'object'
    || cached.version !== HOME_CACHE_VERSION
    || cached.complete !== true
    || typeof cached.updatedAt !== 'number'
    || Date.now() - cached.updatedAt > HOME_CACHE_MAX_AGE_MS) {
    storage(sessionOnly).remove(key)
    return null
  }
  return cached.data
}

function write<T>(key: string, data: T, sessionOnly = false) {
  storage(sessionOnly).set(key, {
    version: HOME_CACHE_VERSION,
    updatedAt: Date.now(),
    complete: true,
    data,
  } satisfies CacheEnvelope<T>)
}

export function readSpacesCache(userId?: number, sessionOnly = false): any[] | null {
  const cached = read<any[]>(spacesKey(userId), sessionOnly)
  return Array.isArray(cached) && cached.length ? cached : null
}

export function writeSpacesCache(spaces: any[], userId?: number, sessionOnly = false) {
  if (Array.isArray(spaces))
    write(spacesKey(userId), spaces, sessionOnly)
}

export function readSpaceCache(spaceId: number, userId?: number, sessionOnly = false): SpaceCache | null {
  const cached = read<SpaceCache>(spaceKey(spaceId, userId), sessionOnly)
  if (!cached || !Array.isArray(cached.groups) || !cached.groups.every(group => group && group.id !== undefined)
    || !cached.items || typeof cached.items !== 'object'
    || !cached.groups.every(group => Array.isArray(cached.items[String(group.id)])))
    return null
  return cached
}

export function createSpaceCache(): SpaceCache {
  return { groups: [], items: {} }
}

export function writeSpaceCache(spaceId: number, cache: SpaceCache, userId?: number, sessionOnly = false) {
  if (!Array.isArray(cache.groups)
    || !cache.groups.every(group => Array.isArray(cache.items[String(group.id)])))
    return
  write(spaceKey(spaceId, userId), { ...cache, updatedAt: Date.now() }, sessionOnly)
}

export function clearSpaceCache(spaceId: number, userId?: number, sessionOnly = false) {
  storage(sessionOnly).remove(spaceKey(spaceId, userId))
}
