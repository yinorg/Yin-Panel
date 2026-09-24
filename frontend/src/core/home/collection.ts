import type { SpaceCache } from '../../utils/spaceCache'

export interface HomeCollectionGroup {
  id?: number
}

export interface HomeCollectionItem {
  id?: number
}

export interface HomeCollectionResult<Group extends HomeCollectionGroup, Item extends HomeCollectionItem> {
  spaceId: number
  groups: Group[]
  itemsByGroup: Map<number, Item[]>
  cache: SpaceCache
  failedGroups: number
  stale: boolean
  error?: { code: string; message: string }
  cancelled: boolean
}

export interface HomeCollectionOptions<Group extends HomeCollectionGroup, Item extends HomeCollectionItem> {
  getGroups: (spaceId: number, signal: AbortSignal) => Promise<{ data?: Group[] }>
  getItems: (spaceId: number, groupId: number, page: number, pageSize: number, signal: AbortSignal) => Promise<{ data?: Item[] }>
  readCache: (spaceId: number) => SpaceCache | null
  writeCache: (spaceId: number, cache: SpaceCache) => void
  isOnline: () => boolean
  timeoutMs?: number
  concurrency?: number
  pageSize?: number
  onProgress?: (result: HomeCollectionResult<Group, Item>) => void
}

export function createHomeCollectionLoader<Group extends HomeCollectionGroup, Item extends HomeCollectionItem>(options: HomeCollectionOptions<Group, Item>) {
  let generation = 0
  let controller: AbortController | undefined

  function cancel() {
    generation++
    controller?.abort()
    controller = undefined
  }

  async function load(spaceId: number, forceRefresh = false): Promise<HomeCollectionResult<Group, Item>> {
    cancel()
    const currentGeneration = generation
    const requestController = new AbortController()
    controller = requestController
    const active = () => currentGeneration === generation && !requestController.signal.aborted
    const cached = options.readCache(spaceId)
    const cache: SpaceCache = cached || { groups: [], items: {} }
    const itemsByGroup = new Map<number, Item[]>()
    const result: HomeCollectionResult<Group, Item> = {
      spaceId,
      groups: [],
      itemsByGroup,
      cache,
      failedGroups: 0,
      stale: false,
      cancelled: false,
    }
    const publish = () => {
      if (active()) options.onProgress?.({ ...result, itemsByGroup: new Map(itemsByGroup) })
    }
    const empty = () => {
      if (!cached) result.error = { code: 'OFFLINE', message: 'Home data is unavailable offline' }
      result.stale = !!cached
      result.cancelled = !active()
      return result
    }

    let groups = !forceRefresh && cached ? cache.groups as Group[] : undefined
    if (!groups) {
      if (!options.isOnline()) return empty()
      try {
        const response = await withTimeout(signal => options.getGroups(spaceId, signal), options.timeoutMs ?? 3000, requestController.signal)
        if (!active()) return { ...result, cancelled: true }
        groups = response.data
        if (groups) cache.groups = groups
      }
      catch {
        if (!active()) return { ...result, cancelled: true }
        if (cache.groups.length) {
          groups = cache.groups as Group[]
          result.stale = true
        }
        else {
          result.error = { code: 'LOAD_FAILED', message: 'Could not load bookmark groups' }
          return result
        }
      }
    }
    if (!groups) return { ...result, cancelled: true }
    result.groups = groups
    const validGroups = groups.filter((group): group is Group & { id: number } => Number.isSafeInteger(Number(group.id)))
    const pending = validGroups.filter(group => forceRefresh || !Array.isArray(cache.items[String(group.id)]))
    for (const group of validGroups) {
      const cachedItems = cache.items[String(group.id)]
      if (Array.isArray(cachedItems)) itemsByGroup.set(Number(group.id), cachedItems as Item[])
    }
    publish()

    if (!options.isOnline() && pending.length) {
      result.stale = true
      options.writeCache(spaceId, cache)
      return result
    }

    let next = 0
    const worker = async () => {
      while (active() && next < pending.length) {
        const group = pending[next++]
        const groupId = Number(group.id)
        const previousItems = itemsByGroup.get(groupId)
        const loadedItems: Item[] = []
        const seenItemIds = new Set<number>()
        try {
          const pageSize = options.pageSize ?? 100
          let page = 1
          while (active()) {
            const response = await withTimeout(signal => options.getItems(spaceId, groupId, page, pageSize, signal), options.timeoutMs ?? 3000, requestController.signal)
            if (!active()) return
            const pageItems = response.data || []
            for (const item of pageItems) {
              const itemId = Number(item.id)
              if (!Number.isSafeInteger(itemId) || seenItemIds.has(itemId)) continue
              seenItemIds.add(itemId)
              loadedItems.push(item)
            }
            itemsByGroup.set(groupId, [...loadedItems])
            publish()
            if (pageItems.length < pageSize) {
              cache.items[String(groupId)] = loadedItems
              break
            }
            page++
          }
        }
        catch {
          if (!active()) return
          result.failedGroups++
          if (previousItems)
            itemsByGroup.set(groupId, previousItems)
          else if (loadedItems.length)
            itemsByGroup.set(groupId, loadedItems)
        }
        publish()
      }
    }
    await Promise.all(Array.from({ length: Math.min(options.concurrency ?? 2, pending.length) }, worker))
    if (!active()) return { ...result, cancelled: true }

    options.writeCache(spaceId, cache)
    result.stale ||= result.failedGroups > 0
    if (result.failedGroups)
      result.error = { code: 'PARTIAL_LOAD', message: 'Some bookmark items could not be loaded' }
    return result
  }

  return { load, cancel }
}

function withTimeout<T>(request: (signal: AbortSignal) => Promise<T>, timeoutMs: number, parentSignal: AbortSignal): Promise<T> {
  const controller = new AbortController()
  return new Promise((resolve, reject) => {
    let settled = false
    const finish = (callback: (value: any) => void, value: unknown) => {
      if (settled) return
      settled = true
      clearTimeout(timeout)
      parentSignal.removeEventListener('abort', abort)
      callback(value)
    }
    const abort = () => {
      controller.abort()
      finish(reject, Object.assign(new Error('Home request was cancelled'), { code: 'ABORTED' }))
    }
    const timeout = setTimeout(() => {
      controller.abort()
      finish(reject, new Error('Home request timed out'))
    }, timeoutMs)
    parentSignal.addEventListener('abort', abort, { once: true })
    if (parentSignal.aborted) abort()
    else request(controller.signal).then(
      value => finish(resolve, value),
      error => finish(reject, error),
    )
  })
}
