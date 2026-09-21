import { ls } from './storage/local'

export interface SpaceCache {
  groups?: any[]
  items: Record<string, any[]>
  searchConfig?: any
}

const spacesKey = (userId?: number) => `yin-panel-spaces-cache:${userId || 'anonymous'}`

export function readSpacesCache(userId?: number): any[] {
  const cached = ls.get(spacesKey(userId))
  return Array.isArray(cached) ? cached : []
}

export function writeSpacesCache(spaces: any[], userId?: number) {
  ls.set(spacesKey(userId), spaces)
}

function key(spaceId: number, userId?: number) {
  return `yin-panel-space-cache:${userId || 'anonymous'}:${spaceId}`
}

export function readSpaceCache(spaceId: number, userId?: number): SpaceCache {
  const cached = ls.get(key(spaceId, userId)) as Partial<SpaceCache> | null
  return cached && typeof cached === 'object' ? { items: {}, ...cached } : { items: {} }
}

export function writeSpaceCache(spaceId: number, cache: SpaceCache, userId?: number) {
  ls.set(key(spaceId, userId), cache)
}

export function clearSpaceCache(spaceId: number, userId?: number) {
  ls.remove(key(spaceId, userId))
}
