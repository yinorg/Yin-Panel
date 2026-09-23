import type { GlobalThemeOverrides } from 'naive-ui'
import { computed, ref, watch, watchEffect } from 'vue'
import { darkTheme, useOsTheme } from 'naive-ui'
import { useAppStore } from '../store'
import { useAuthStore } from '../store'
import { getCurrentTheme, getMyTheme } from '../api/theme'
import { parsePublicCodeFromPath } from '../utils/request/axios'
import { resolveThemeSlots, selectThemeScheme, type ThemePackage } from '../utils/theme'

export const activeThemePackage = ref<ThemePackage | null>(null)
export const activeThemeSlots = ref<Record<string, string>>({})
export const activeThemePackageId = ref('')
let themeLoad: Promise<void> | undefined
let loadedUserToken = ''

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
      await refreshCurrentTheme()
    })()
  }
  if (token && token !== loadedUserToken && !parsePublicCodeFromPath()) {
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
    return selectThemeScheme(schemes, appStore.theme, OsTheme.value === 'dark' ? 'dark' : 'light')
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
      },
    }
  })

  watchEffect(() => {
    const pkg = activeThemePackage.value
    if (!pkg) return
    try {
      activeThemePackageId.value = pkg.manifest.id
      activeThemeSlots.value = resolveThemeSlots(pkg, selectedScheme.value)
    }
    catch {
      activeThemeSlots.value = {}
    }
  })

  watch(activeThemeSlots, (slots) => {
    const root = document.documentElement
    for (const [slot, value] of Object.entries(slots))
      root.style.setProperty(`--yin-${slot}`, value)
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
