import type { ThemeCollectionStatus, ThemeGroup, ThemeHomeSnapshot, ThemeItem, ThemeSpace } from '../../theme/api/v1'

interface SpaceSource {
  id: number
  name: string
  side?: 'yin' | 'yang'
  pairedSpaceId?: number
  canSelect?: boolean
}

interface GroupSource {
  id: number
  parentId?: number | null
  title?: string
  icon?: string
  items?: readonly ItemSource[]
}

interface ItemSource {
  id: number
  title: string
  description?: string
  icon?: { itemType: number; src?: string; fileName?: string; text?: string; backgroundColor?: string } | null
  sort?: number
}

export function createThemeHomeSnapshot(input: {
  version: number
  status: ThemeCollectionStatus
  spaces: readonly SpaceSource[]
  activeSpaceId?: number
  activeSpaceSide?: 'yin' | 'yang'
  activeSpacePairedId?: number
  activeSpaceCapabilities?: readonly string[]
  groups: readonly GroupSource[]
  canWrite: boolean
  error?: { code: string; message: string }
}): ThemeHomeSnapshot {
  const spaces: ThemeSpace[] = input.spaces.map(space => ({
    id: String(space.id),
    name: space.name,
    side: space.side,
    pairedSpaceId: space.pairedSpaceId === undefined ? undefined : String(space.pairedSpaceId),
    capabilities: space.canSelect === false ? [] : ['space.select'],
  }))
  const groups: ThemeGroup[] = []
  const items: ThemeItem[] = []
  for (const group of input.groups) {
    const groupId = String(group.id)
    groups.push({
      id: groupId,
      spaceId: input.activeSpaceId === undefined ? '' : String(input.activeSpaceId),
      parentId: group.parentId == null ? undefined : String(group.parentId),
      title: group.title || '',
      icon: group.icon,
      itemIds: (group.items || []).map(item => String(item.id)),
    })
    for (const item of group.items || []) {
      items.push({
        id: String(item.id),
        groupId,
        title: item.title,
        description: item.description,
        icon: item.icon ? {
          itemType: item.icon.itemType,
          src: item.icon.src,
          fileName: item.icon.fileName,
          text: item.icon.text,
          backgroundColor: item.icon.backgroundColor,
        } : undefined,
        sort: item.sort ?? 0,
        capabilities: input.canWrite ? ['item.open', 'item.update', 'item.delete'] : ['item.open'],
      })
    }
  }
  return {
    version: input.version,
    status: input.status,
    error: input.error,
    spaces,
    activeSpaceId: input.activeSpaceId === undefined ? undefined : String(input.activeSpaceId),
    activeSpaceSide: input.activeSpaceSide,
    activeSpacePairedId: input.activeSpacePairedId === undefined ? undefined : String(input.activeSpacePairedId),
    activeSpaceCapabilities: input.activeSpaceCapabilities || [],
    groups,
    items,
  }
}
