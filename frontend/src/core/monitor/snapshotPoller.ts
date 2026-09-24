export interface SnapshotPollerOptions<Snapshot> {
  fetchSnapshot: () => Promise<Snapshot>
  getInterval: () => Promise<number>
  onSnapshot: (snapshot: Snapshot) => void
  onFailure?: (error: unknown, failures: number) => void
  failureLimit?: number
}

export function createSnapshotPoller<Snapshot>(options: SnapshotPollerOptions<Snapshot>) {
  let timer: ReturnType<typeof setTimeout> | undefined
  let stopped = true
  let failures = 0
  let interval: Promise<number> | undefined

  async function poll() {
    if (stopped) return
    try {
      const snapshot = await options.fetchSnapshot()
      if (stopped) return
      failures = 0
      options.onSnapshot(snapshot)
    }
    catch (error) {
      if (stopped) return
      failures++
      options.onFailure?.(error, failures)
      if (failures >= (options.failureLimit ?? 3)) {
        stop()
        return
      }
    }
    if (stopped) return
    const delay = interval ? await interval : 10000
    if (!stopped) timer = setTimeout(poll, delay)
  }

  function start() {
    if (!stopped) return
    stopped = false
    failures = 0
    interval = options.getInterval().then(value => Math.max(250, value)).catch(() => 10000)
    void poll()
  }

  function stop() {
    stopped = true
    if (timer) clearTimeout(timer)
    timer = undefined
    interval = undefined
  }

  return { start, stop }
}
