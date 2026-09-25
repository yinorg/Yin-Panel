export interface HomeMutationResult {
  code: number
  msg?: string
  data?: unknown
}

export interface HomeItemMutationCommands {
  saveItem: (input: Record<string, unknown>, options: { itemId?: number; iconFile?: File; expectedSpaceId: number }) => Promise<HomeMutationResult>
  uploadItemIcon: (file: File, expectedSpaceId: number) => Promise<HomeMutationResult>
}

export interface HomeMutationApi {
  createItem: (spaceId: number, input: Record<string, unknown>) => Promise<HomeMutationResult>
  createItemWithIcon?: (spaceId: number, input: Record<string, unknown>, file: File) => Promise<HomeMutationResult>
  uploadItemIcon?: (file: File) => Promise<HomeMutationResult>
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
    return result
  }

  async function mutateResult(operation: (spaceId: number) => Promise<HomeMutationResult>, expectedSpaceId?: number) {
    const spaceId = writableSpaceId()
    if (expectedSpaceId !== undefined && expectedSpaceId !== spaceId)
      throw apiError('ABORTED', 'The active Space changed before the operation was confirmed')
    return commit(spaceId, () => operation(spaceId))
  }

  async function mutate(operation: (spaceId: number) => Promise<HomeMutationResult>, expectedSpaceId?: number) {
    await mutateResult(operation, expectedSpaceId)
  }

  return {
    uploadItemIcon: async (file: File, expectedSpaceId: number) => {
      const spaceId = writableSpaceId()
      if (expectedSpaceId !== spaceId)
        throw apiError('ABORTED', 'The active Space changed before the icon upload')
      if (!options.api.uploadItemIcon)
        throw apiError('UNSUPPORTED_CAPABILITY', 'Uploading item icons is unavailable')
      const result = await options.api.uploadItemIcon(file)
      if (result.code !== 0)
        throw apiError('RUNTIME_UNAVAILABLE', result.msg || 'Core rejected the item icon upload')
      if (options.getActiveSpaceId() !== spaceId)
        throw apiError('ABORTED', 'The active Space changed while the icon was uploading')
      return result
    },
    saveItem: (input: Record<string, unknown>, saveOptions: { itemId?: number; iconFile?: File; expectedSpaceId: number }) => mutateResult((spaceId) => {
      if (saveOptions.itemId !== undefined)
        return options.api.updateItem(spaceId, saveOptions.itemId, input)
      if (saveOptions.iconFile) {
        if (!options.api.createItemWithIcon)
          throw apiError('UNSUPPORTED_CAPABILITY', 'Creating items with an icon file is unavailable')
        return options.api.createItemWithIcon(spaceId, input, saveOptions.iconFile)
      }
      return options.api.createItem(spaceId, input)
    }, saveOptions.expectedSpaceId),
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
