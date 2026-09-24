export interface HomeMutationResult {
  code: number
  msg?: string
}

export interface HomeMutationApi {
  createItem: (spaceId: number, input: Record<string, unknown>) => Promise<HomeMutationResult>
  updateItem: (spaceId: number, itemId: number, input: Record<string, unknown>) => Promise<HomeMutationResult>
  deleteItem: (spaceId: number, itemId: number) => Promise<HomeMutationResult>
  reorderItems: (spaceId: number, groupId: number, itemIds: number[]) => Promise<HomeMutationResult>
  createGroup: (spaceId: number, input: { title: string; icon: string; parentId: number | null }) => Promise<HomeMutationResult>
  updateGroup: (spaceId: number, groupId: number, input: { title: string; icon: string; parentId: number | null }) => Promise<HomeMutationResult>
  deleteGroup: (spaceId: number, groupId: number) => Promise<HomeMutationResult>
  reorderGroups: (spaceId: number, parentId: number | null, groupIds: number[]) => Promise<HomeMutationResult>
}

export function createHomeMutationService(options: {
  api: HomeMutationApi
  getActiveSpaceId: () => number | undefined
  canWrite: () => boolean
  invalidateSpace: (spaceId: number) => void
  refreshSpace: (spaceId: number) => Promise<unknown> | unknown
}) {
  function writableSpaceId(): number {
    const spaceId = options.getActiveSpaceId()
    if (!options.canWrite() || !spaceId)
      throw apiError('PERMISSION_DENIED', 'The active Space is read-only')
    return spaceId
  }

  async function commit(spaceId: number, operation: () => Promise<HomeMutationResult>) {
    const result = await operation()
    if (result.code !== 0)
      throw apiError('RUNTIME_UNAVAILABLE', result.msg || 'Core rejected the Theme command')
    options.invalidateSpace(spaceId)
    if (options.getActiveSpaceId() === spaceId)
      await options.refreshSpace(spaceId)
  }

  async function mutate(operation: (spaceId: number) => Promise<HomeMutationResult>, expectedSpaceId?: number) {
    const spaceId = writableSpaceId()
    if (expectedSpaceId !== undefined && expectedSpaceId !== spaceId)
      throw apiError('ABORTED', 'The active Space changed before the operation was confirmed')
    return commit(spaceId, () => operation(spaceId))
  }

  return {
    createItem: (input: Record<string, unknown>) => mutate(spaceId => options.api.createItem(spaceId, input)),
    updateItem: (itemId: number, input: Record<string, unknown>) => mutate(spaceId => options.api.updateItem(spaceId, itemId, input)),
    deleteItem: (itemId: number, expectedSpaceId?: number) => mutate(spaceId => options.api.deleteItem(spaceId, itemId), expectedSpaceId),
    reorderItems: (groupId: number, itemIds: number[]) => mutate(spaceId => options.api.reorderItems(spaceId, groupId, itemIds)),
    createGroup: (input: { title: string; icon: string; parentId: number | null }) => mutate(spaceId => options.api.createGroup(spaceId, input)),
    updateGroup: (groupId: number, input: { title: string; icon: string; parentId: number | null }) => mutate(spaceId => options.api.updateGroup(spaceId, groupId, input)),
    deleteGroup: (groupId: number, expectedSpaceId?: number) => mutate(spaceId => options.api.deleteGroup(spaceId, groupId), expectedSpaceId),
    reorderGroups: (parentId: number | null, groupIds: number[]) => mutate(spaceId => options.api.reorderGroups(spaceId, parentId, groupIds)),
  }
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
