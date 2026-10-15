import type { HomeLayoutInput } from './mutations'

/** The columns a layout save can change: which group an entry sits in, and its order. */
export interface LayoutEntry {
  id?: number
  itemIconGroupId?: number
  sort?: number
  createTime?: string
}

const sortOf = (entry: LayoutEntry) => Number(entry.sort) || 0
const idOf = (entry: LayoutEntry) => Number(entry.id) || 0
const createdOf = (entry: LayoutEntry) => {
  const parsed = entry.createTime ? Date.parse(entry.createTime) : Number.NaN
  return Number.isFinite(parsed) ? parsed : 0
}

/**
 * The order an item read comes back in: `sort, created_at, id`.
 *
 * Matching the endpoint exactly is what makes a local patch indistinguishable
 * from the re-read it replaces. It matters more than it looks: a save only sends
 * the rows it changed, so a group can carry a stale duplicate `sort` from an
 * earlier save, and then the tie-break is the only thing deciding the order.
 */
const compareItems = (a: LayoutEntry, b: LayoutEntry) =>
  sortOf(a) - sortOf(b) || createdOf(a) - createdOf(b) || idOf(a) - idOf(b)

/** The group list reads back in `sort, id`. */
const compareGroups = (a: LayoutEntry, b: LayoutEntry) => sortOf(a) - sortOf(b) || idOf(a) - idOf(b)

/**
 * Applies a saved layout to one set of group buckets, keyed by group id.
 *
 * This is the whole reason a layout save needs no re-read: the payload *is* the
 * resulting state, so the collection the theme is rendering from can take it
 * directly instead of asking the server to hand back what we just told it.
 *
 * Two properties keep the result equal to what a reload would have shown:
 *
 * - Only the groups the layout actually touched are re-sorted, and they are
 *   re-sorted in the server's own order. An untouched group keeps the order the
 *   server gave it rather than a local approximation of it.
 * - An item whose target group is not present here is dropped rather than placed
 *   somewhere invented. That group is simply not loaded (a collapsed or paged
 *   layout), and the next full load puts the item back where the server says.
 */
export function patchLayoutBuckets<Item extends LayoutEntry>(
  buckets: Record<string, Item[]>,
  layout: HomeLayoutInput,
): Record<string, Item[]> {
  const patchById = new Map(layout.items.map(entry => [entry.id, entry]))
  const touched = new Set<string>()
  const moved: Item[] = []
  for (const key of Object.keys(buckets)) {
    const list = buckets[key]
    if (!Array.isArray(list)) continue
    const kept: Item[] = []
    for (const item of list) {
      const patch = patchById.get(Number(item.id))
      if (!patch) {
        kept.push(item)
        continue
      }
      touched.add(key)
      moved.push({ ...item, itemIconGroupId: patch.groupId, sort: patch.sort })
    }
    buckets[key] = kept
  }
  for (const item of moved) {
    const key = String(item.itemIconGroupId)
    if (!Array.isArray(buckets[key])) continue
    buckets[key].push(item)
    touched.add(key)
  }
  for (const key of touched)
    buckets[key] = buckets[key].slice().sort(compareItems)
  return buckets
}

/**
 * Applies a saved layout's group order.
 *
 * Group order comes from the array the tree is built from, so this patches `sort`
 * and re-sorts in one step. Groups the layout did not name keep their order only
 * as far as the tie-break allows — which is exactly what the group list would
 * have read back as — and since every level's siblings share a parent, sorting
 * the flat list is enough for the tree to come out ordered.
 */
export function patchLayoutGroups<Group extends LayoutEntry>(
  groups: Group[],
  layout: HomeLayoutInput,
): Group[] {
  const sortById = new Map(layout.groups.map(entry => [entry.id, entry.sort]))
  return groups
    .map(group => sortById.has(Number(group.id)) ? { ...group, sort: sortById.get(Number(group.id)) } as Group : group)
    .sort(compareGroups)
}
