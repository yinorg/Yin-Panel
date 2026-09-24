export interface HomeBootstrapResult<TMonitor, TSpaces> {
  cancelled: boolean
  monitor?: TMonitor
  spaces?: TSpaces
  spacesFailed?: boolean
}

interface HomeBootstrapOptions<TMonitor, TSpaces> {
  getMonitor: (signal: AbortSignal) => Promise<TMonitor>
  getSpaces: (signal: AbortSignal) => Promise<TSpaces>
  refreshConfig: (signal: AbortSignal) => Promise<unknown>
  timeoutMs?: number
}

export function createHomeBootstrap<TMonitor, TSpaces>(options: HomeBootstrapOptions<TMonitor, TSpaces>) {
  const timeoutMs = options.timeoutMs ?? 3000
  let generation = 0
  let controller: AbortController | undefined

  function cancel() {
    generation++
    controller?.abort()
    controller = undefined
  }

  async function load(): Promise<HomeBootstrapResult<TMonitor, TSpaces>> {
    cancel()
    const currentGeneration = generation
    const requestController = new AbortController()
    controller = requestController
    const run = async <T>(request: (signal: AbortSignal) => Promise<T>) => {
      let timer: ReturnType<typeof setTimeout> | undefined
      let abortListener: (() => void) | undefined
      let requestPromise: Promise<T>
      try {
        requestPromise = Promise.resolve(request(requestController.signal))
      }
      catch (error) {
        requestPromise = Promise.reject(error)
      }
      try {
        return await Promise.race([
          requestPromise,
          new Promise<T>((_, reject) => {
            timer = setTimeout(() => reject(new Error('Home request timed out')), timeoutMs)
          }),
          new Promise<T>((_, reject) => {
            abortListener = () => reject(new DOMException('Home request cancelled', 'AbortError'))
            requestController.signal.addEventListener('abort', abortListener, { once: true })
          }),
        ])
      }
      finally {
        if (timer) clearTimeout(timer)
        if (abortListener) requestController.signal.removeEventListener('abort', abortListener)
      }
    }
    const [monitor, , spaces] = await Promise.allSettled([
      run(options.getMonitor),
      run(options.refreshConfig),
      run(options.getSpaces),
    ])
    if (currentGeneration !== generation || requestController.signal.aborted) return { cancelled: true }
    controller = undefined
    return {
      cancelled: false,
      monitor: monitor.status === 'fulfilled' ? monitor.value : undefined,
      spaces: spaces.status === 'fulfilled' ? spaces.value : undefined,
      spacesFailed: spaces.status === 'rejected',
    }
  }

  return { load, cancel }
}
