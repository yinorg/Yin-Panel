export interface SearchableHomeItem {
  id?: number
  title?: string
  url?: string
  description?: string
  itemIconGroupId?: number
}

export interface HomeSearchOptions<Item extends SearchableHomeItem> {
  fetchPage: (spaceId: number, page: number, pageSize: number, signal: AbortSignal) => Promise<{ data?: Item[] }>
  isOnline: () => boolean
  pageSize?: number
}

export function createHomeSearchService<Item extends SearchableHomeItem>(options: HomeSearchOptions<Item>) {
  const itemsBySpace = new Map<number, Item[]>()
  const pendingBySpace = new Map<number, { controller: AbortController; promise: Promise<Item[]> }>()

  function cancel() {
    for (const pending of pendingBySpace.values()) pending.controller.abort()
    pendingBySpace.clear()
  }

  function invalidate(spaceId?: number) {
    if (spaceId === undefined) itemsBySpace.clear()
    else itemsBySpace.delete(spaceId)
    cancel()
  }

  async function getAllItems(spaceId: number): Promise<Item[]> {
    const cached = itemsBySpace.get(spaceId)
    if (cached) return cached
    const pending = pendingBySpace.get(spaceId)
    if (pending) return pending.promise
    if (!options.isOnline()) throw Object.assign(new Error('Search is unavailable offline until all items are cached'), { code: 'OFFLINE' })

    const controller = new AbortController()
    const pageSize = options.pageSize ?? 200
    const promise = (async () => {
      const allItems: Item[] = []
      const seen = new Set<number>()
      let page = 1
      while (true) {
        let data: Item[] | undefined
        try {
          ({ data } = await options.fetchPage(spaceId, page, pageSize, controller.signal))
        }
        catch (error) {
          if (controller.signal.aborted)
            throw Object.assign(new Error('Search request was superseded'), { code: 'ABORTED' })
          throw error
        }
        if (controller.signal.aborted)
          throw Object.assign(new Error('Search request was superseded'), { code: 'ABORTED' })
        const rows = data || []
        for (const item of rows) {
          const id = Number(item.id)
          if (!Number.isSafeInteger(id) || seen.has(id)) continue
          seen.add(id)
          allItems.push(item)
        }
        if (rows.length < pageSize) break
        page++
      }
      if (controller.signal.aborted)
        throw Object.assign(new Error('Search request was superseded'), { code: 'ABORTED' })
      itemsBySpace.set(spaceId, allItems)
      return allItems
    })()
    pendingBySpace.set(spaceId, { controller, promise })
    try { return await promise }
    finally {
      if (pendingBySpace.get(spaceId)?.promise === promise) pendingBySpace.delete(spaceId)
    }
  }

  async function query(spaceId: number, keyword: string): Promise<Item[]> {
    const normalized = keyword.trim().toLocaleLowerCase()
    if (!normalized) return []
    const allItems = await getAllItems(spaceId)
    return allItems.filter(item => [item.title, item.url, item.description]
      .some(value => value?.toLocaleLowerCase().includes(normalized)))
  }

  return { query, invalidate, cancel }
}
