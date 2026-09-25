export interface ThemeSettingsStorage {
  getItem: (key: string) => string | null
  setItem: (key: string, value: string) => void
}

export interface ThemePersistenceContext {
  userId: () => string | number | undefined | null
  packageId: () => string | undefined | null
  revision: () => string | undefined | null
  validateSettings?: (settings: Record<string, unknown>) => Promise<void>
}

export interface ThemePersistenceStorage extends ThemeSettingsStorage {
  removeItem: (key: string) => void
}

export function createThemePersistence(storage: ThemePersistenceStorage, context: ThemePersistenceContext, maxBytes = 65_536) {
  const identity = () => ({
    userId: context.userId() || 'anonymous',
    packageId: context.packageId() || 'unknown',
  })

  function settingsStore() {
    const { userId, packageId } = identity()
    const revision = context.revision() || 'uninstalled'
    const key = `yin-theme-settings:${userId}:${packageId}:${revision}`
    return {
      get(): Record<string, unknown> {
        const encoded = storage.getItem(key)
        if (!encoded) return {}
        try {
          const value: unknown = JSON.parse(encoded)
          return isRecord(value) ? value : {}
        }
        catch {
          return {}
        }
      },
      async patch(value: Record<string, unknown>): Promise<Record<string, unknown>> {
        const next = { ...this.get(), ...value }
        const encoded = encodeJson(next, 'Theme settings must be JSON serializable')
        if (byteLength(encoded) > maxBytes)
          throw apiError('INVALID_ARGUMENT', `Theme settings are limited to ${maxBytes} bytes`)
        await context.validateSettings?.(next)
        storage.setItem(key, encoded)
        return next
      },
    }
  }

  function storageKey(key: string) {
    const { userId, packageId } = identity()
    return `yin-theme-storage:${userId}:${packageId}:${key}`
  }

  return {
    getSettings: () => settingsStore().get(),
    patchSettings: (value: Record<string, unknown>) => settingsStore().patch(value),
    getStorage(key: string): unknown {
      const encoded = storage.getItem(storageKey(key))
      if (encoded === null) return undefined
      try {
        return JSON.parse(encoded)
      }
      catch {
        return undefined
      }
    },
    setStorage(key: string, value: unknown): void {
      const encoded = encodeJson(value, 'Theme storage values must be JSON serializable')
      if (byteLength(encoded) > maxBytes)
        throw apiError('INVALID_ARGUMENT', `Theme storage values are limited to ${maxBytes} bytes`)
      storage.setItem(storageKey(key), encoded)
    },
    removeStorage: (key: string) => storage.removeItem(storageKey(key)),
  }
}

function encodeJson(value: unknown, errorMessage: string): string {
  try {
    const encoded = JSON.stringify(value)
    if (encoded !== undefined) return encoded
  }
  catch { /* Convert serialization errors to the Theme API error shape. */ }
  throw apiError('INVALID_ARGUMENT', errorMessage)
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).byteLength
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
