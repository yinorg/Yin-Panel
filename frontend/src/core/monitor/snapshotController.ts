// @ts-expect-error Node's strip-types runner requires explicit source extensions.
import { loadDiskSnapshots } from './diskSnapshots.ts'
// @ts-expect-error Node's strip-types runner requires explicit source extensions.
import { createSnapshotPoller } from './snapshotPoller.ts'

interface DiskSnapshotMap<Disk> {
  DISK_INFO?: Record<string, Disk>
}

export interface MonitorSnapshotControllerOptions<Snapshot extends DiskSnapshotMap<Disk>, Disk> {
  fetchSnapshot: () => Promise<Snapshot>
  fetchDisk: (path: string) => Promise<{ code: number; data: Disk }>
  getInterval: () => Promise<number>
}

export interface MonitorSnapshotController<Snapshot> {
  fetchSnapshot: () => Promise<Snapshot>
  setDiskPaths: (paths: Iterable<string>) => void
  subscribe: (listener: (snapshot: Snapshot) => void) => () => void
  start: () => void
  stop: () => void
}

export function createMonitorSnapshotController<Snapshot extends DiskSnapshotMap<Disk>, Disk>(
  options: MonitorSnapshotControllerOptions<Snapshot, Disk>,
) {
  let diskPaths: string[] = []
  let diskPathGeneration = 0
  let diskCache: Record<string, Disk> = {}
  let snapshotRequest: Promise<Snapshot> | undefined
  const listeners = new Set<(snapshot: Snapshot) => void>()

  async function fetchSnapshot(): Promise<Snapshot> {
    if (snapshotRequest) return snapshotRequest

    const request = (async () => {
      const source = await options.fetchSnapshot()
      while (true) {
        const generation = diskPathGeneration
        const paths = diskPaths
        const disks = await loadDiskSnapshots(paths, options.fetchDisk, diskCache)
        if (generation !== diskPathGeneration) continue
        diskCache = disks
        return { ...source, DISK_INFO: disks } as Snapshot
      }
    })()
    const trackedRequest = request.finally(() => {
      if (snapshotRequest === trackedRequest)
        snapshotRequest = undefined
    })
    snapshotRequest = trackedRequest
    return trackedRequest
  }

  const poller = createSnapshotPoller({
    fetchSnapshot,
    getInterval: options.getInterval,
    onSnapshot(snapshot) {
      for (const listener of listeners)
        listener(snapshot)
    },
  })

  function setDiskPaths(paths: Iterable<string>) {
    const next = [...new Set(paths)].filter(Boolean)
    if (next.length === diskPaths.length && next.every((path, index) => path === diskPaths[index])) return
    diskPaths = next
    diskPathGeneration++
  }

  function subscribe(listener: (snapshot: Snapshot) => void) {
    listeners.add(listener)
    return () => { listeners.delete(listener) }
  }

  return {
    fetchSnapshot,
    setDiskPaths,
    subscribe,
    start: poller.start,
    stop: poller.stop,
  }
}
