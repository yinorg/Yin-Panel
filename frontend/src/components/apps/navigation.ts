import type { InjectionKey } from 'vue'

/**
 * Switch the settings modal to another app.
 *
 * Provided by `AppStarter`, which owns the modal's current app: a page that would
 * otherwise duplicate a sibling page's UI (the appearance page and the wallpaper
 * centre both pick a wallpaper) points at that page instead of re-implementing it.
 */
export const openSettingsAppKey: InjectionKey<(componentName: string) => void> = Symbol('openSettingsApp')
