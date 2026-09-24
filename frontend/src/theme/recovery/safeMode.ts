export const THEME_SAFE_MODE_KEY = 'yin-theme-safe-mode'

export interface ThemeSafeModeStorage {
  getItem: (key: string) => string | null
  setItem: (key: string, value: string) => void
  removeItem: (key: string) => void
}

export function isThemeSafeMode(storage: ThemeSafeModeStorage = sessionStorage): boolean {
  return storage.getItem(THEME_SAFE_MODE_KEY) === '1'
}

export function enableThemeSafeMode(storage: ThemeSafeModeStorage = sessionStorage): void {
  storage.setItem(THEME_SAFE_MODE_KEY, '1')
}

export function disableThemeSafeMode(storage: ThemeSafeModeStorage = sessionStorage): void {
  storage.removeItem(THEME_SAFE_MODE_KEY)
}
