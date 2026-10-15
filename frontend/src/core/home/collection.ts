import type { SpaceCache } from '../../utils/spaceCache'

export interface HomeCollectionGroup {
  id?: number
  /**
   * Items the group holds, as the group list reports it. Groups with no items are
   * left out of the counts rather than reported as zero, so an absent count is a
   * zero; only a list that reports no counts at all means the total is unknown.
   */
  itemCount?: number
}

export interface HomeCollectionItem {
  id?: number
  /** The group an item belongs to; lets one Space-wide page replace the fan-out. */
  itemIconGroupId?: number
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
  /**
   * One page of the whole Space, in `sort, created_at, id` order. When supplied,
   * the loader pages the Space rather than asking once per group: a Space of
   * ordinary size then costs `ceil(items / pageSize)` round trips instead of
   * `ceil(groups / concurrency)` of them, which is the whole difference on a slow
   * link. It only takes that shape while it is the cheaper one, and falls back to
   * the per-group walk otherwise. Omitting the option keeps the walk only.
   */
  getSpaceItems?: (spaceId: number, page: number, pageSize: number, signal: AbortSignal) => Promise<{ data?: Item[] }>
  readCache: (spaceId: number) => SpaceCache | null
  writeCache: (spaceId: number, cache: SpaceCache) => void
  isOnline: () => boolean
  timeoutMs?: number
  concurrency?: number
  pageSize?: number
  spacePageSize?: number
  retries?: number
  onProgress?: (result: HomeCollectionResult<Group, Item>) => void
}

/**
 * Item requests in flight per load.
 *
 * The deployment is HTTP/2 + HTTP/3, where streams multiplex over one connection
 * and the browser's six-connections-per-origin ceiling does not apply, so every
 * group of a Space can be asked for at once. Over HTTP/1.1 the browser applies
 * that ceiling itself, which makes this value inert rather than wrong.
 */
export const HOME_ITEM_CONCURRENCY = 10

/**
 * Extra attempts per group before it counts as failed.
 *
 * A group that loses a packet used to stay missing for the rest of the visit —
 * `failedGroups` only marks the result stale, nothing retried it. One retry turns
 * that into a slow group instead of an absent one.
 */
export const HOME_ITEM_RETRIES = 1

/** Pause before a retry, long enough to let a congested link recover. */
const HOME_ITEM_RETRY_DELAY_MS = 200

/** The page size of a Space-wide read; the endpoint caps `pageSize` at 200. */
const HOME_SPACE_PAGE_SIZE = 200

export function createHomeCollectionLoader<Group extends HomeCollectionGroup, Item extends HomeCollectionItem>(options: HomeCollectionOptions<Group, Item>) {
  let generation = 0
  let controller: AbortController | undefined
  /**
   * Spaces already known to be too large for a Space-wide page.
   *
   * Without this the fallback would pay for the probe again on every load. Kept
   * per loader rather than per module so two panels in one session never share it.
   */
  const oversizedSpaces = new Set<number>()

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
    // One publish rebuilds the theme's whole snapshot (625 groups and 1000 items
    // on a loaded Space) and re-renders it, so a Space-wide read that published
    // per page paid that cost once per page. Coalesce them instead: progress
    // still reaches the reader, but a few times a second at most, and the final
    // state is carried by the returned result either way.
    const PUBLISH_MIN_INTERVAL_MS = 120
    let lastPublishAt = 0
    let publishTimer: ReturnType<typeof setTimeout> | null = null
    const publishNow = () => {
      if (publishTimer) {
        clearTimeout(publishTimer)
        publishTimer = null
      }
      lastPublishAt = Date.now()
      if (active()) options.onProgress?.({ ...result, itemsByGroup: new Map(itemsByGroup) })
    }
    const publish = () => {
      if (!active()) return
      const wait = PUBLISH_MIN_INTERVAL_MS - (Date.now() - lastPublishAt)
      if (wait <= 0) {
        publishNow()
        return
      }
      if (!publishTimer) publishTimer = setTimeout(publishNow, wait)
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

    if (await loadWholeSpace()) return result

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
            const response = await fetchGroupPage(groupId, page, pageSize)
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

    /** One group page, retried once so a lost packet costs a delay and not a group. */
    const fetchGroupPage = async (groupId: number, page: number, pageSize: number) => {
      const attempts = Math.max(0, options.retries ?? HOME_ITEM_RETRIES) + 1
      let lastError: unknown
      for (let attempt = 0; attempt < attempts; attempt++) {
        if (!active()) throw new Error('Home request was cancelled')
        if (attempt) await delay(HOME_ITEM_RETRY_DELAY_MS)
        try {
          return await withTimeout(signal => options.getItems(spaceId, groupId, page, pageSize, signal), options.timeoutMs ?? 3000, requestController.signal)
        }
        catch (error) {
          lastError = error
          // A cancelled or superseded load must not be retried into life.
          if (!active()) throw error
        }
      }
      throw lastError
    }

    /**
     * Replaces the per-group fan-out with whole-Space pages.
     *
     * The group list carries each group's item count, so how many pages the Space
     * needs is known before the first request. That matters because one Space-wide
     * page costs the same round trip as one group page: when the Space would need
     * at least as many pages as it has groups, the walk is the cheaper shape and
     * no probe is spent finding that out. Without the counts (an older backend)
     * the loader keeps its single probe and hands back to the walk on a full page.
     *
     * Returns true when it produced the items, false when the caller should walk
     * the groups instead. Never throws: this is an optimisation, so anything that
     * goes wrong here just hands the work back to the walk.
     */
    async function loadWholeSpace(): Promise<boolean> {
      if (!options.getSpaceItems || pending.length < 2 || !active()) return false
      if (oversizedSpaces.has(spaceId)) return false
      const pageSize = options.spacePageSize ?? HOME_SPACE_PAGE_SIZE
      const counted = pending.some(group => Number.isFinite(Number(group.itemCount)))
      const itemTotal = pending.reduce((total, group) => total + (Number(group.itemCount) || 0), 0)
      const pages = counted ? Math.max(1, Math.ceil(itemTotal / pageSize)) : 1
      // Paging the Space only saves anything while it needs fewer round trips than
      // the per-group walk it replaces.
      if (pages >= pending.length) return false

      const collected: Item[] = []
      for (let page = 1; page <= pages; page++) {
        let chunk: Item[]
        try {
          const response = await withTimeout(
            signal => options.getSpaceItems!(spaceId, page, pageSize, signal),
            options.timeoutMs ?? 3000,
            requestController.signal,
          )
          if (!active()) return false
          chunk = response.data || []
        }
        catch {
          return false
        }
        collected.push(...chunk)
        // A short page is the end of the Space.
        if (chunk.length < pageSize) return bucketWholeSpace(collected)
      }
      // A full last page means the counts were short — the Space has grown since
      // the group list was read. Hand the work back rather than save a layout that
      // is missing whatever the budget cut off.
      if (counted) return false
      oversizedSpaces.add(spaceId)
      return false
    }

    /**
     * Buckets whole-Space pages by group and writes the cache.
     *
     * The pages arrive in `sort, created_at, id` order, the same order each
     * per-group page uses, so bucketing preserves every group's own order.
     */
    function bucketWholeSpace(page: Item[]): boolean {
      const pendingIds = new Set(pending.map(group => Number(group.id)))
      const buckets = new Map<number, Item[]>()
      const seen = new Map<number, Set<number>>()
      for (const item of page) {
        const groupId = Number(item.itemIconGroupId)
        const itemId = Number(item.id)
        if (!Number.isSafeInteger(groupId) || !Number.isSafeInteger(itemId) || !pendingIds.has(groupId)) continue
        let bucket = buckets.get(groupId)
        if (!bucket) {
          bucket = []
          buckets.set(groupId, bucket)
          seen.set(groupId, new Set())
        }
        if (seen.get(groupId)!.has(itemId)) continue
        seen.get(groupId)!.add(itemId)
        bucket.push(item)
      }
      for (const group of pending) {
        const groupId = Number(group.id)
        // A group the pages did not mention is empty, and an empty array is what
        // tells the next load not to ask for it again.
        const bucket = buckets.get(groupId) || []
        itemsByGroup.set(groupId, bucket)
        cache.items[String(groupId)] = bucket
      }
      publish()
      options.writeCache(spaceId, cache)
      result.stale ||= result.failedGroups > 0
      return true
    }

    await Promise.all(Array.from({ length: Math.min(options.concurrency ?? HOME_ITEM_CONCURRENCY, pending.length) }, worker))
    if (!active()) return { ...result, cancelled: true }

    options.writeCache(spaceId, cache)
    result.stale ||= result.failedGroups > 0
    if (result.failedGroups)
      result.error = { code: 'PARTIAL_LOAD', message: 'Some bookmark items could not be loaded' }
    return result
  }

  return { load, cancel }
}

function delay(ms: number) {
  return new Promise(resolve => setTimeout(resolve, ms))
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
