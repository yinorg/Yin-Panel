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
  let generation = 0
  let inFlight: Promise<Snapshot> | undefined

  async function poll(run: number) {
    if (stopped || run !== generation) return
    try {
      if (!inFlight) {
        const request = Promise.resolve().then(options.fetchSnapshot)
        const trackedRequest = request.finally(() => {
          if (inFlight === trackedRequest)
            inFlight = undefined
        })
        inFlight = trackedRequest
      }
      const snapshot = await inFlight
      if (stopped || run !== generation) return
      failures = 0
      options.onSnapshot(snapshot)
    }
    catch (error) {
      if (stopped || run !== generation) return
      failures++
      options.onFailure?.(error, failures)
      if (failures >= (options.failureLimit ?? 3)) {
        stop()
        return
      }
    }
    if (stopped || run !== generation) return
    const delay = interval ? await interval : 10000
    if (!stopped && run === generation) timer = setTimeout(() => void poll(run), delay)
  }

  function start() {
    if (!stopped) return
    stopped = false
    failures = 0
    const run = ++generation
    interval = Promise.resolve().then(options.getInterval).then(value => Math.max(250, value)).catch(() => 10000)
    void poll(run)
  }

  function stop() {
    generation++
    stopped = true
    if (timer) clearTimeout(timer)
    timer = undefined
    interval = undefined
  }

  return { start, stop }
}
