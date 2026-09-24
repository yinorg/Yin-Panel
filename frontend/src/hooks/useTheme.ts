import type { GlobalThemeOverrides } from 'naive-ui'
import { computed, ref, watch, watchEffect } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'
import { useAppStore } from '../store'
import { useAuthStore } from '../store'
import { getCurrentTheme, getMyTheme, getPreviewTheme } from '../api/theme'
import { parsePublicCodeFromPath } from '../utils/request/axios'
import { resolveThemeSlots, resolveWallpaper, selectThemeScheme, type ThemePackage } from '../utils/theme'

export const activeThemePackage = ref<ThemePackage | null>(null)
export const activeThemeSlots = ref<Record<string, string>>({})
export const activeThemePackageId = ref('')
export const activeThemeWallpaper = ref<ReturnType<typeof resolveWallpaper>>(null)
let themeLoad: Promise<void> | undefined
let loadedUserToken = ''
const previewToken = new URLSearchParams(window.location.search).get('themePreview')
const previewMode = new URLSearchParams(window.location.search).get('themePreviewMode')
let appliedSlots: string[] = []
let fontStyle: HTMLStyleElement | null = null

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

function initializeTheme(appStore: ReturnType<typeof useAppStore>, token: string) {
  if (!themeLoad) {
    themeLoad = (async () => {
      if (previewToken) {
        const preview = await getPreviewTheme(previewToken)
        if (preview.code === 0) activeThemePackage.value = preview.data
      }
      else await refreshCurrentTheme()
    })()
  }
  if (!previewToken && token && token !== loadedUserToken && !parsePublicCodeFromPath()) {
    loadedUserToken = token
    themeLoad = themeLoad.then(async () => {
      try {
        await refreshMyTheme(appStore)
      }
      catch {
        loadedUserToken = ''
      }
    })
  }
  return themeLoad
}

export function useTheme() {
  const appStore = useAppStore()
  const authStore = useAuthStore()
  const OsTheme = useOsTheme()

  initializeTheme(appStore, authStore.token || '')
  watch(() => authStore.token, (token) => {
    if (previewToken) return
    if (token) {
      initializeTheme(appStore, token)
    }
    else {
      loadedUserToken = ''
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
        heightMedium: slots.controlHeight || 'var(--yin-controlHeight)',
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
      Select: { peers: { InternalSelection: { borderRadius: 'var(--yin-component-input-radius)' } } },
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
