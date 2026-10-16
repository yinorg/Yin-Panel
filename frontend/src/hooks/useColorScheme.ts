import type { GlobalThemeOverrides } from 'naive-ui'
import { computed, ref, shallowRef, watch } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'
import { useAppStore } from '../store'

/**
 * The colour half of the application's appearance, kept apart from the theme
 * *package*.
 *
 * `useTheme` resolves the installed package, its Naive slots and the palette the
 * wallpaper derives, which pulls in the whole theme runtime with it. The
 * application-level Naive provider sits above the router, so it has to decide
 * before — and without — any of that: on the Core-only routes (login, OAuth
 * callback, recovery, errors) there is no theme package at all, and those pages
 * paint with the store's own light/dark choice and Naive's defaults.
 *
 * The themed shell resolves what that provider should actually show and
 * publishes it here (see `publishHomeScheme`), so a single provider keeps
 * serving both kinds of route without dragging the home runtime onto the login
 * page.
 */
const homeScheme = ref<'dark' | 'light' | null>(null)
const homeOverrides = shallowRef<GlobalThemeOverrides>({})

/**
 * Publish the scheme and Naive overrides the themed shell resolved.
 *
 * Called by `useTheme` while the shell is mounted; the values outlive the shell
 * deliberately, so leaving the home for a Core-only route keeps the appearance
 * the visitor was just looking at instead of snapping back to the OS default.
 */
export function publishHomeScheme(scheme: 'dark' | 'light', overrides: GlobalThemeOverrides) {
  homeScheme.value = scheme
  homeOverrides.value = overrides
}

export function useColorScheme() {
  const appStore = useAppStore()
  const OsTheme = useOsTheme()

  const params = new URLSearchParams(window.location.search)
  const previewMode = params.has('themePreview') && (params.get('themePreviewMode') === 'light' || params.get('themePreviewMode') === 'dark')
    ? params.get('themePreviewMode') as 'light' | 'dark'
    : null

  /**
   * What the Core's own surfaces follow when no theme is mounted. `auto` follows
   * the operating system; the home overrides this with the wallpaper's own tone
   * once a package resolves, because a themed page is painted on that artwork
   * rather than on a bare canvas.
   */
  const preferredScheme = computed<'dark' | 'light'>(() => {
    if (previewMode)
      return previewMode
    if (appStore.theme === 'dark') return 'dark'
    if (appStore.theme === 'light') return 'light'
    return OsTheme.value === 'dark' ? 'dark' : 'light'
  })

  const isDark = computed(() => (homeScheme.value ?? preferredScheme.value) === 'dark')
  const theme = computed(() => (isDark.value ? darkTheme : undefined))
  const themeOverrides = computed(() => homeOverrides.value)

  watch(isDark, (dark) => {
    if (typeof document === 'undefined') return
    if (dark) document.documentElement.classList.add('dark')
    else document.documentElement.classList.remove('dark')
  }, { immediate: true })

  return { isDark, theme, themeOverrides }
}
