export interface HomeGroupNode<Item = unknown> {
  id?: number
  parentId?: number | null
  items?: Item[]
  depth?: number
}

export interface HomeGroupTree<Group extends HomeGroupNode> {
  groups: Group[]
  childrenById: Map<number, Group[]>
}

export function buildHomeGroupTree<Group extends HomeGroupNode<Item>, Item>(
  source: Group[],
  itemsByGroup = new Map<number, Item[]>(),
): HomeGroupTree<Group> {
  const byId = new Map<number, Group>()
  for (const group of source) {
    const id = Number(group.id)
    if (!Number.isSafeInteger(id)) continue
    const node = { ...group, items: itemsByGroup.get(id) || [] } as Group
    byId.set(id, node)
  }

  const groups: Group[] = []
  const childrenById = new Map<number, Group[]>()
  const visited = new Set<number>()
  const roots: Group[] = []
  const append = (node: Group, depth: number) => {
    const id = Number(node.id)
    if (visited.has(id)) return
    visited.add(id)
    node.depth = depth
    groups.push(node)
    for (const child of childrenById.get(id) || [])
      append(child, depth + 1)
  }

  for (const node of byId.values()) {
    const parentId = Number(node.parentId) || 0
    if (!parentId || parentId === Number(node.id) || !byId.has(parentId)) {
      node.parentId = null
      roots.push(node)
      continue
    }
    const children = childrenById.get(parentId) || []
    children.push(node)
    childrenById.set(parentId, children)
  }

  for (const root of roots)
    append(root, 0)

  for (const node of byId.values()) {
    if (!visited.has(Number(node.id))) {
      node.parentId = null
      roots.push(node)
      append(node, 0)
    }
  }

  return { groups, childrenById }
}

export function getHomeGroupRoots<Group extends HomeGroupNode>(groups: Group[]): Group[] {
  return groups.filter(group => !group.parentId)
}

export function resolveActiveDirectoryId<Group extends HomeGroupNode>(groups: Group[], currentId: number | null): number | null {
  const roots = getHomeGroupRoots(groups)
  return roots.some(group => Number(group.id) === currentId)
    ? currentId
    : Number(roots[0]?.id) || null
}

export function getDirectoryGroups<Group extends HomeGroupNode>(groups: Group[], selectedId: number | null): Group[] {
  if (!selectedId) return groups
  const visible = new Set<number>([selectedId])
  let changed = true
  while (changed) {
    changed = false
    for (const group of groups) {
      const id = Number(group.id)
      if (group.parentId && visible.has(Number(group.parentId)) && !visible.has(id)) {
        visible.add(id)
        changed = true
      }
    }
  }
  return groups.filter(group => visible.has(Number(group.id)))
}

export function getInitiallyCollapsedGroups<Group extends HomeGroupNode<Item>, Item>(
  groups: Group[],
  enabled: boolean,
  itemThreshold = 40,
): Set<number> {
  if (!enabled) return new Set()
  const parentIds = new Set(groups.map(group => Number(group.parentId)).filter(Boolean))
  return new Set(groups
    .filter(group => parentIds.has(Number(group.id)) || (group.items?.length || 0) > itemThreshold)
    .map(group => Number(group.id)))
}

export function isHomeGroupHidden<Group extends HomeGroupNode>(
  groups: Group[],
  index: number,
  collapsed: ReadonlySet<number>,
): boolean {
  const byId = new Map(groups.map(group => [Number(group.id), group]))
  const group = groups[index]
  let parent = group?.parentId ? byId.get(Number(group.parentId)) : undefined
  const visited = new Set<number>()
  while (parent) {
    const id = Number(parent.id)
    if (collapsed.has(id)) return true
    if (visited.has(id)) return false
    visited.add(id)
    parent = parent.parentId ? byId.get(Number(parent.parentId)) : undefined
  }
  return false
}
