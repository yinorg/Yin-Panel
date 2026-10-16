import type { GlobalThemeOverrides } from 'naive-ui'
import { computed, ref, watch, watchEffect } from 'vue'
import { useOsTheme } from 'naive-ui'
import { useAppStore } from '../store'
import { useAuthStore } from '../store'
import { getCurrentTheme, getEffectiveTheme, getMyTheme, getPreviewTheme, getThemePackage } from '../api/theme'
import { parsePublicCodeFromPath } from '../utils/request/axios'
import { isBuiltinThemePackage, isYinThemePackage, resolveThemeSlots, resolveWallpaper, selectThemeScheme, type ThemePackage } from '../utils/theme'
import { suggestedForeground, wallpaperPainted, wallpaperTone } from '@/core/home/wallpaperTone'
import { prefetchThemeResource } from '@/theme/runtime/resourceCache'
import { isThemeSafeMode } from '../theme/recovery/safeMode'
import { publishHomeScheme } from './useColorScheme'

export const activeThemePackage = ref<ThemePackage | null>(null)
/**
 * Whether the initial theme-package load has settled — success *or* failure.
 * The home must not read "still loading" as "no home view": the theme package
 * arrives after the data, so an unresolved package briefly looked like a theme
 * with no home contribution and flashed the C3 fallback before the real theme
 * mounted. False until the first load resolves, then true for good.
 */
export const themePackageResolved = ref(false)
export const activeThemeSlots = ref<Record<string, string>>({})
export const activeThemePackageId = ref('')
export const activeThemeWallpaper = ref<ReturnType<typeof resolveWallpaper>>(null)
let themeLoad: Promise<void> | undefined
let loadedUserLoggedIn = false
const previewToken = new URLSearchParams(window.location.search).get('themePreview')
const previewMode = new URLSearchParams(window.location.search).get('themePreviewMode')
let appliedSlots: string[] = []
let fontStyle: HTMLStyleElement | null = null
let prefetchedThemeRevision = ''

/**
 * The installed revision each built-in package resolves to for this panel.
 *
 * A Space theme is stored as a *revision*, so two Spaces that both chose the same
 * built-in theme can still name two different revisions — whichever build was
 * current when each Space was configured. Mounting the named revision per Space
 * makes every Space switch tear the whole theme app down and rebuild it (measured
 * at ~4s on a loaded home) even though both copies are the same theme. A built-in
 * also ships with the panel, so its frozen older copy can never receive a fix.
 * Resolve every built-in to the installed revision for its id instead: Space
 * switches then hand the mounted theme a new snapshot, and only a genuinely
 * different theme restarts it.
 */
const builtinPackageRevision = new Map<string, Promise<ThemePackage | null>>()

function installedBuiltinPackage(pkg: ThemePackage): Promise<ThemePackage | null> {
  const id = pkg.manifest.id
  let pending = builtinPackageRevision.get(id)
  if (!pending) {
    pending = getThemePackage(id)
      .then(result => (result.code === 0 && result.data ? result.data : null))
      .catch(() => null)
    builtinPackageRevision.set(id, pending)
  }
  return pending
}

/**
 * Where the resolved palette is cached for the next page load.
 *
 * The boot layer in `index.html` runs before the bundle and cannot ask the theme
 * for its colours, so it reads this snapshot instead of guessing from the OS
 * preference — that is what removes the tone jump between the boot screen and the
 * home. A blocked or full `localStorage` only costs the exact match.
 */
const BOOT_SNAPSHOT_KEY = 'yinBootSnapshot'

function persistBootSnapshot(scheme: string, slots: Record<string, string>) {
  try {
    const canvas = slots.canvas || ''
    if (!canvas)
      return
    window.localStorage.setItem(BOOT_SNAPSHOT_KEY, JSON.stringify({
      scheme,
      canvas,
      text: slots.text || '',
      muted: slots.textMuted || '',
      primary: slots.primary || '',
    }))
  }
  catch {
    // Keep whatever snapshot is already stored.
  }
}

function toNaiveDimension(value: string | undefined, fallback: string): string {
  if (!value) return fallback
  const normalized = value.trim()
  if (/^-?(?:\d+|\d*\.\d+)px$/.test(normalized)) return normalized
  const rem = normalized.match(/^-?(?:\d+|\d*\.\d+)rem$/)
  if (rem) {
    const rootSize = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16
    return `${Number.parseFloat(normalized) * rootSize}px`
  }
  return fallback
}

export async function refreshMyTheme(appStore: ReturnType<typeof useAppStore>) {
  const mine = await getMyTheme()
  if (mine.code === 0) {
    activeThemePackage.value = mine.data.package
    appStore.setTheme(mine.data.preference.mode)
  }
  return mine
}

async function refreshCurrentTheme() {
  try {
    const current = await getCurrentTheme()
    if (current.code === 0) activeThemePackage.value = current.data
  }
  catch {
    // Keep the last valid package if the theme endpoint is temporarily unavailable.
  }
}

/**
 * Re-resolve the theme for a space through the server-side precedence chain.
 * It is a no-op when the resolved revision matches what is already mounted, so
 * switching spaces never reloads an unchanged theme.
 *
 * A public link takes this path too: the visitor has no user preference to
 * resolve, but the space the code exposes does have a theme, and the server
 * resolves it from the Space alone. Skipping the call left every public link on
 * the system default no matter what the space had chosen.
 */
export async function reconcileSpaceTheme(spaceId?: number) {
  if (previewToken || isThemeSafeMode())
    return
  try {
    const effective = await getEffectiveTheme(spaceId)
    if (effective.code !== 0 || !effective.data.package)
      return
    let next = effective.data.package
    if (isBuiltinThemePackage(next.manifest.id)) {
      const installed = await installedBuiltinPackage(next)
      if (installed && installed.manifest.id === next.manifest.id)
        next = installed
    }
    if (next.revision !== activeThemePackage.value?.revision)
      activeThemePackage.value = next
  }
  catch {
    // Keep the mounted package when resolution is temporarily unavailable.
  }
}

function initializeTheme(appStore: ReturnType<typeof useAppStore>, loggedIn: boolean) {
  if (window.location.pathname === '/__yin/theme-recovery')
    return Promise.resolve()
  if (!themeLoad) {
    themeLoad = (async () => {
      try {
        if (isThemeSafeMode()) {
          const yin = await getThemePackage('org.yin.default')
          if (yin.code === 0) activeThemePackage.value = yin.data
        }
        else if (previewToken) {
          const preview = await getPreviewTheme(previewToken)
          if (preview.code === 0) activeThemePackage.value = preview.data
        }
        else await refreshCurrentTheme()
      }
      finally {
        // Settle even when the request failed: an unresolved flag would hide the
        // C3 fallback forever, which is exactly the state it exists to cover.
        themePackageResolved.value = true
      }
    })()
  }
  if (!previewToken && loggedIn && !loadedUserLoggedIn && !parsePublicCodeFromPath() && !isThemeSafeMode()) {
    loadedUserLoggedIn = true
    themeLoad = themeLoad.then(async () => {
      try {
        await refreshMyTheme(appStore)
      }
      catch {
        loadedUserLoggedIn = false
      }
    })
  }
  return themeLoad
}

/**
 * Warm the HTTP cache for the theme's own entry assets.
 *
 * They are revision-addressed and served `immutable` (see `AssetV2`), so requesting
 * them as soon as the package resolves overlaps the network with the grant round
 * trip that has to finish before the host would otherwise ask for them; the host's
 * own `fetch` then resolves from cache. Fire-and-forget: a failure here changes
 * nothing, because the host still loads and verifies every resource itself.
 */
function prefetchThemeEntrypoints(pkg: ThemePackage | null) {
  if (!pkg)
    return
  // The package ref is reassigned by each resolver (`current`, then `mine`,
  // then the per-space `effective`), so dedupe on the revision the assets carry.
  const revision = pkg.revision || ''
  if (revision && revision === prefetchedThemeRevision)
    return
  prefetchedThemeRevision = revision
  const urls = new Set<string>()
  const collect = (path?: string) => {
    if (!path)
      return
    const resource = pkg.manifest.resources?.find(item => item.path === path)
    if (resource?.url)
      urls.add(new URL(resource.url, window.location.origin).href)
  }
  collect(pkg.manifest.entrypoints?.script)
  for (const style of pkg.manifest.entrypoints?.styles || [])
    collect(style)
  for (const url of urls)
    prefetchThemeResource(url)
}

export function useTheme() {
  const appStore = useAppStore()
  const authStore = useAuthStore()
  const OsTheme = useOsTheme()

  initializeTheme(appStore, authStore.loggedIn)
  watch(() => authStore.loggedIn, (loggedIn) => {
    if (previewToken) return
    if (loggedIn) {
      initializeTheme(appStore, loggedIn)
    }
    else {
      loadedUserLoggedIn = false
      appStore.setTheme('auto')
      void refreshCurrentTheme()
    }
  })

  const selectedScheme = computed(() => {
    const schemes = activeThemePackage.value?.manifest.schemes || []
    const mode = previewToken && (previewMode === 'light' || previewMode === 'dark') ? previewMode : appStore.theme
    // `auto` follows the artwork, not the operating system.
    //
    // The home is painted on the wallpaper, so the theme's whole token set has to
    // agree with what is behind it: a light wallpaper with a dark OS theme used to
    // put the dark tokens on a light background, and every text, accent, card fill
    // and border fought the surface under it. Only the few rules that happened to
    // read the derived tone looked right, which is exactly the mixed page that
    // exposed this. With no artwork behind the page there is nothing to follow, so
    // the OS theme still decides.
    const ambient = wallpaperPainted.value && wallpaperTone.value
      ? wallpaperTone.value
      : OsTheme.value === 'dark' ? 'dark' : 'light'
    return selectThemeScheme(schemes, mode, ambient)
  })

  /**
   * The on-wallpaper text pair every element without a background of its own
   * reads.
   *
   * Their compatibility defaults are a fixed white, which is only right over
   * dark artwork, so the derived tone replaces them whenever artwork is painted.
   * Together with the scheme following the same tone, this is what makes the
   * page coherent: a light wallpaper gives dark on-wallpaper text and pale
   * surface fills, a dark one gives light text and dark fills, and the two never
   * disagree.
   */
  watch([wallpaperPainted, wallpaperTone], ([painted, tone]) => {
    const root = document.documentElement
    if (!painted || !tone) {
      root.style.removeProperty('--yin-onWallpaper')
      root.style.removeProperty('--yin-onWallpaperMuted')
      return
    }
    const foreground = suggestedForeground(tone)
    root.style.setProperty('--yin-onWallpaper', foreground.onWallpaper)
    root.style.setProperty('--yin-onWallpaperMuted', foreground.onWallpaperMuted)
  }, { immediate: true })

  const isDark = computed(() => {
    return selectedScheme.value === 'dark'
  })

  const themeOverrides = computed<GlobalThemeOverrides>(() => {
    const slots = activeThemeSlots.value
    // The pre-theme build rendered Naive components with their own defaults.
    // Injecting the theme package's Naive overrides changes control heights,
    // borders and text colours (the login form drifted by several pixels), so
    // the built-in default theme in light mode keeps Naive's own tokens.
    if (!isDark.value && !previewToken && isYinThemePackage(activeThemePackageId.value))
      return {}
    const controlHeight = toNaiveDimension(slots.controlHeight, '36px')
    return {
      common: {
        bodyColor: slots.canvas,
        cardColor: slots.surface,
        modalColor: slots.surfaceElevated,
        popoverColor: slots.surfaceElevated,
        textColor1: slots.text,
        textColor2: slots.textMuted,
        borderColor: slots.border,
        primaryColor: slots.primary,
        primaryColorHover: slots.primary,
        primaryColorPressed: slots.primary,
        primaryColorSuppl: slots.primary,
        successColor: slots.success,
        warningColor: slots.warning,
        errorColor: slots.danger,
        fontFamily: slots.fontBody || 'var(--yin-fontBody)',
        fontSize: slots.fontBodySize || 'var(--yin-fontBodySize)',
        borderRadius: slots.radiusControl || 'var(--yin-radiusControl)',
        heightMedium: controlHeight,
        boxShadow1: slots.shadowCard || 'var(--yin-shadowCard)',
        boxShadow2: slots.shadowPopup || 'var(--yin-shadowPopup)',
      },
      Button: { heightMedium: 'var(--yin-component-button-height)', borderRadiusMedium: 'var(--yin-component-button-radius)' },
      Input: { heightMedium: 'var(--yin-component-input-height)', borderRadius: 'var(--yin-component-input-radius)' },
      Card: { borderRadius: 'var(--yin-component-card-radius)', boxShadow: 'var(--yin-component-card-shadow)' },
      Modal: { borderRadius: 'var(--yin-component-dialog-radius)' },
      Dialog: { borderRadius: 'var(--yin-component-dialog-radius)' },
      Popover: { borderRadius: 'var(--yin-component-menu-radius)' },
      Dropdown: { borderRadius: 'var(--yin-component-menu-radius)' },
      Tooltip: { borderRadius: 'var(--yin-component-tooltip-radius)' },
      Select: {
        peers: {
          InternalSelection: { borderRadius: 'var(--yin-component-input-radius)' },
          InternalSelectMenu: {
            color: 'var(--yin-surfaceElevated)',
            borderRadius: 'var(--yin-component-menu-radius)',
            optionHeightMedium: controlHeight,
            optionTextColor: 'var(--yin-text)',
            optionTextColorActive: 'var(--yin-primary)',
            optionTextColorPressed: 'var(--yin-onPrimary)',
            optionTextColorDisabled: 'var(--yin-textMuted)',
            optionColorActive: 'color-mix(in srgb, var(--yin-primary) 14%, var(--yin-surfaceElevated))',
            optionColorActivePending: 'color-mix(in srgb, var(--yin-primary) 20%, var(--yin-surfaceElevated))',
            optionColorPending: 'color-mix(in srgb, var(--yin-primary) 8%, var(--yin-surfaceElevated))',
            optionCheckColor: 'var(--yin-primary)',
          },
        },
      },
      Menu: { borderRadius: 'var(--yin-component-menu-radius)' },
    }
  })

  // The application-level Naive provider lives above the router and cannot read
  // the package state this composable owns, so the resolved pair is published to
  // the lightweight colour module that provider does read. This is what keeps the
  // login page's provider free of the theme runtime while still giving the home
  // the package's own colours, control metrics and wallpaper-following scheme.
  watchEffect(() => {
    publishHomeScheme(selectedScheme.value === 'dark' ? 'dark' : 'light', themeOverrides.value)
  })

  watchEffect(() => {
    const pkg = activeThemePackage.value
    if (!pkg) return
    try {
      // Resolve into locals first: reading the refs back after writing them would
      // make this effect depend on its own output and re-run forever.
      const slots = resolveThemeSlots(pkg, selectedScheme.value)
      const wallpaper = resolveWallpaper(pkg, selectedScheme.value)
      activeThemePackageId.value = pkg.manifest.id
      activeThemeSlots.value = slots
      activeThemeWallpaper.value = wallpaper
      persistBootSnapshot(selectedScheme.value, slots)
    }
    catch {
      activeThemeSlots.value = {}
      activeThemeWallpaper.value = null
    }
  })

  watch(activeThemeSlots, (slots) => {
    const root = document.documentElement
    for (const slot of appliedSlots) root.style.removeProperty(`--yin-${slot}`)
    for (const [slot, value] of Object.entries(slots))
      root.style.setProperty(`--yin-${slot}`, value)
    appliedSlots = Object.keys(slots)
    // The elevated surface is a pre-theme variable, not part of the token set, so
    // it kept the Core's light default whatever the theme chose. Everything that
    // reads it — the group panel, the menus, the icon tiles — stayed white in a
    // dark scheme. Deriving it from the theme's own surface puts them back on the
    // scheme their text is drawn in.
    const surface = slots.surface || slots.canvas
    if (surface) {
      root.style.setProperty('--yin-surfaceElevated', surface)
      root.style.setProperty('--yin-searchBoxSurface', surface)
    }
    const surfaceOpacity = Number(slots['component-card-surface-opacity'] || 1)
    const surfaceBlur = Number.parseFloat(slots['component-card-surface-blur'] || '0')
    const surfaceGlow = Number.parseFloat(slots['component-card-surface-glow'] || '0')
    const surfaceMode = slots['component-card-surface-mode'] || (surfaceOpacity < 0.9 && surfaceBlur > 0 ? 'glass' : surfaceBlur > 0 ? 'frosted' : surfaceGlow > 0 ? 'gradient' : 'solid')
    const densityScale = Number(slots['density-scale'] || 1)
    const density = slots['density-scale'] && Number.isNaN(densityScale) ? slots['density-scale'] : densityScale <= 0.9 ? 'compact' : densityScale >= 1.2 ? 'spacious' : densityScale > 1.04 ? 'comfortable' : 'standard'
    const texture = slots['background-texture'] || (Number(slots['background-grid-opacity'] || 0) > 0 ? 'grid' : Number(slots['background-dots-opacity'] || 0) > 0 ? 'dots' : Number(slots['background-noise-opacity'] || 0) > 0 ? 'noise' : 'none')
    const validSurface = (mode?: string) => ['solid', 'transparent', 'glass', 'frosted', 'gradient'].includes(mode || '') ? mode : 'solid'
    root.dataset.yinSurface = validSurface(surfaceMode)
    root.dataset.yinSearchSurface = validSurface(slots['component-search-box-surface-mode'] || surfaceMode)
    root.dataset.yinSidebarSurface = validSurface(slots['component-sidebar-surface-mode'] || surfaceMode)
    root.dataset.yinDialogSurface = validSurface(slots['component-dialog-surface-mode'] || surfaceMode)
    root.dataset.yinMenuSurface = validSurface(slots['component-menu-surface-mode'] || surfaceMode)
    root.dataset.yinButtonSurface = validSurface(slots['component-button-surface-mode'] || surfaceMode)
    root.dataset.yinInputSurface = validSurface(slots['component-input-surface-mode'] || surfaceMode)
    root.dataset.yinTooltipSurface = validSurface(slots['component-tooltip-surface-mode'] || surfaceMode)
    root.dataset.yinDensity = ['compact', 'standard', 'comfortable', 'spacious'].includes(density) ? density : 'standard'
    root.dataset.yinTexture = ['none', 'grid', 'dots', 'noise'].includes(texture) ? texture : 'none'
  }, { immediate: true })

  watch(activeThemePackage, (pkg) => {
    fontStyle?.remove()
    fontStyle = document.createElement('style')
    fontStyle.id = 'yin-theme-fonts'
    for (const font of pkg?.manifest.fonts || []) {
      const url = pkg?.manifest.resources?.find(resource => resource.path === font.path)?.url
      if (!url) continue
      fontStyle.textContent += `@font-face { font-family: ${JSON.stringify(font.family)}; src: url(${JSON.stringify(url)}) format('woff2'); font-weight: ${font.weight}; font-style: ${font.style}; font-display: swap; }`
    }
    document.head.appendChild(fontStyle)
    prefetchThemeEntrypoints(pkg)
  }, { immediate: true })

  return { isDark, themeOverrides }
}
