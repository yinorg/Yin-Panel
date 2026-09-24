export interface HomeSpace {
  id: number
}

export interface HomeSpaceControllerOptions<Space extends HomeSpace> {
  getSpaces: (signal: AbortSignal) => Promise<{ data?: Space[] }>
  sortSpaces: (spaces: Space[]) => Space[]
  getCurrentSpaces: () => Space[]
  readCachedSpaces: () => Space[] | null
  writeCachedSpaces: (spaces: Space[]) => void
  getActiveSpaceId: () => number | undefined
  setSpaces: (spaces: Space[]) => void
  setActiveSpace: (space: Space | null) => void
  loadSpace: (spaceId: number) => Promise<unknown> | unknown
  canWrite: () => boolean
}

export function createHomeSpaceController<Space extends HomeSpace>(options: HomeSpaceControllerOptions<Space>) {
  let generation = 0
  let controller: AbortController | undefined

  function cancel() {
    generation++
    controller?.abort()
    controller = undefined
  }

  function select(spaceId: number): boolean {
    const spaces = options.getCurrentSpaces()
    const selected = spaces?.find(space => space.id === spaceId)
    if (!selected) return false
    options.setActiveSpace(selected)
    void options.loadSpace(selected.id)
    return true
  }

  function restoreCached(): Space | null {
    const cached = options.readCachedSpaces()
    if (!cached?.length) return null
    const spaces = options.sortSpaces(cached)
    const selected = spaces[0] || null
    options.setSpaces(spaces)
    options.setActiveSpace(selected)
    if (selected) void options.loadSpace(selected.id)
    return selected
  }

  async function refresh(selectLatest = false): Promise<boolean> {
    if (!options.canWrite()) return false
    cancel()
    const requestGeneration = generation
    const requestController = new AbortController()
    controller = requestController
    try {
      const response = await options.getSpaces(requestController.signal)
      if (requestGeneration !== generation || requestController.signal.aborted) return false
      const source = response.data || []
      const spaces = options.sortSpaces(source)
      const requestedId = selectLatest ? source.at(-1)?.id : options.getActiveSpaceId()
      const active = spaces.find(space => space.id === requestedId) || spaces[0] || null
      options.setSpaces(spaces)
      options.writeCachedSpaces(spaces)
      options.setActiveSpace(active)
      if (active) await options.loadSpace(active.id)
      return requestGeneration === generation && !requestController.signal.aborted
    }
    catch {
      return false
    }
    finally {
      if (controller === requestController) controller = undefined
    }
  }

  return { cancel, select, restoreCached, refresh }
}
