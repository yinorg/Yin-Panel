import type { GlobalThemeOverrides } from 'naive-ui'
import { computed, ref, watch, watchEffect } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'
import { useAppStore } from '../store'
import { useAuthStore } from '../store'
import { getCurrentTheme, getEffectiveTheme, getMyTheme, getPreviewTheme, getThemePackage } from '../api/theme'
import { parsePublicCodeFromPath } from '../utils/request/axios'
import { resolveThemeSlots, resolveWallpaper, selectThemeScheme, type ThemePackage } from '../utils/theme'
import { isThemeSafeMode } from '../theme/recovery/safeMode'

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
 * switching spaces never reloads an unchanged theme — and today no space theme
 * is set, so it always matches. This is the reservation that lets a space theme
 * take effect later without touching the call sites again.
 */
export async function reconcileSpaceTheme(spaceId?: number) {
  if (previewToken || isThemeSafeMode() || parsePublicCodeFromPath())
    return
  try {
    const effective = await getEffectiveTheme(spaceId)
    if (effective.code !== 0 || !effective.data.package)
      return
    if (effective.data.package.revision !== activeThemePackage.value?.revision)
      activeThemePackage.value = effective.data.package
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
    return selectThemeScheme(schemes, mode, OsTheme.value === 'dark' ? 'dark' : 'light')
  })

  const isDark = computed(() => {
    return selectedScheme.value === 'dark'
  })

  const theme = computed(() => {
    return isDark.value ? darkTheme : undefined
  })

  const themeOverrides = computed<GlobalThemeOverrides>(() => {
    const slots = activeThemeSlots.value
    // The pre-theme build rendered Naive components with their own defaults.
    // Injecting the theme package's Naive overrides changes control heights,
    // borders and text colours (the login form drifted by several pixels), so
    // the built-in default theme in light mode keeps Naive's own tokens.
    if (!isDark.value && !previewToken && activeThemePackageId.value === 'org.yin.default')
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

  watchEffect(() => {
    const pkg = activeThemePackage.value
    if (!pkg) return
    try {
      activeThemePackageId.value = pkg.manifest.id
      activeThemeSlots.value = resolveThemeSlots(pkg, selectedScheme.value)
      activeThemeWallpaper.value = resolveWallpaper(pkg, selectedScheme.value)
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
  }, { immediate: true })

  watch(
    () => isDark.value,
    (dark) => {
      if (dark)
        document.documentElement.classList.add('dark')
      else
        document.documentElement.classList.remove('dark')
    },
    { immediate: true },
  )

  return { theme, themeOverrides }
}
