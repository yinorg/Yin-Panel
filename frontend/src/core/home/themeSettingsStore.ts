export interface ThemeSettingsStorage {
  getItem: (key: string) => string | null
  setItem: (key: string, value: string) => void
}

export function createThemeSettingsStore(storage: ThemeSettingsStorage, namespace: string, maxBytes = 65_536) {
  const key = `yin-theme-settings:${namespace}`

  function get(): Record<string, unknown> {
    const encoded = storage.getItem(key)
    if (!encoded) return {}
    try {
      const value: unknown = JSON.parse(encoded)
      return isRecord(value) ? value : {}
    }
    catch {
      return {}
    }
  }

  function patch(value: Record<string, unknown>): Record<string, unknown> {
    const next = { ...get(), ...value }
    let encoded: string
    try {
      encoded = JSON.stringify(next)
    }
    catch {
      throw apiError('INVALID_ARGUMENT', 'Theme settings must be JSON serializable')
    }
    if (new TextEncoder().encode(encoded).byteLength > maxBytes)
      throw apiError('INVALID_ARGUMENT', `Theme settings are limited to ${maxBytes} bytes`)
    storage.setItem(key, encoded)
    return next
  }

  return { get, patch }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value)
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
