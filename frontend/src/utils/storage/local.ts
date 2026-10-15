import { deCrypto, enCrypto } from '../crypto'

interface StorageData<T = any> {
  data: T
  expire: number | null
}

/**
 * How many decoded entries stay in memory.
 *
 * The Space cache is a CryptoJS AES payload of roughly a megabyte on a loaded
 * home, so decoding it costs hundreds of milliseconds and a Space switch reads it
 * again. Keeping the last few decoded values avoids paying for the same decrypt
 * and parse on every switch. The `storage` event covers the only staleness this
 * can introduce — a write from another tab.
 */
const DECODED_LIMIT = 8

export function createLocalStorage(options?: { expire?: number | null; crypto?: boolean }) {
  const DEFAULT_CACHE_TIME = 60 * 60 * 24 * 7

  const { expire, crypto } = Object.assign(
    {
      expire: DEFAULT_CACHE_TIME,
      crypto: true,
    },
    options,
  )

  const decoded = new Map<string, StorageData>()
  const remember = (key: string, value: StorageData) => {
    decoded.delete(key)
    decoded.set(key, value)
    while (decoded.size > DECODED_LIMIT)
      decoded.delete(decoded.keys().next().value as string)
  }
  if (typeof window !== 'undefined' && typeof window.addEventListener === 'function') {
    // Fires in the *other* tabs only, which is exactly the writer this memory
    // cannot see; this tab's own writes go through `set`, `remove` and `clear`.
    window.addEventListener('storage', (event) => {
      if (event.key === null)
        decoded.clear()
      else
        decoded.delete(event.key)
    })
  }

  function set<T = any>(key: string, data: T) {
    const storageData: StorageData<T> = {
      data,
      expire: expire !== null ? new Date().getTime() + expire * 1000 : null,
    }

    const json = crypto ? enCrypto(storageData) : JSON.stringify(storageData)
    window.localStorage.setItem(key, json)
    remember(key, storageData)
  }

  function get(key: string) {
    const remembered = decoded.get(key)
    if (remembered) {
      if (remembered.expire === null || remembered.expire >= Date.now())
        return remembered.data
      decoded.delete(key)
    }
    const json = window.localStorage.getItem(key)
    if (json) {
      let storageData: StorageData | null = null

      try {
        storageData = crypto ? deCrypto(json) : JSON.parse(json)
      }
      catch {
        // Prevent failure
      }

      if (storageData) {
        const { data, expire } = storageData
        if (expire === null || expire >= Date.now()) {
          remember(key, storageData)
          return data
        }
      }

      remove(key)
      return null
    }
  }

  function remove(key: string) {
    window.localStorage.removeItem(key)
    decoded.delete(key)
  }

  function clear() {
    window.localStorage.clear()
    decoded.clear()
  }

  return {
    set,
    get,
    remove,
    clear,
  }
}

export const ls = createLocalStorage()

export const ss = createLocalStorage({ expire: null, crypto: false })
