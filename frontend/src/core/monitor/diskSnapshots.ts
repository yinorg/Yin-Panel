export async function loadDiskSnapshots<Disk>(
  paths: Iterable<string>,
  fetchDisk: (path: string) => Promise<{ code: number; data: Disk }>,
  previous: Record<string, Disk> = {},
): Promise<Record<string, Disk>> {
  const uniquePaths = [...new Set(paths)].filter(Boolean)
  const snapshots: Record<string, Disk> = {}
  for (const path of uniquePaths) {
    if (previous[path] !== undefined)
      snapshots[path] = previous[path]
  }
  await Promise.all(uniquePaths.map(async (path) => {
    try {
      const result = await fetchDisk(path)
      if (result.code === 0)
        snapshots[path] = result.data
    }
    catch {
      // Retain the last successful snapshot for paths that fail this poll.
    }
  }))
  return snapshots
}
