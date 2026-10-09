import { computed, onMounted, onUnmounted, ref } from 'vue'
import { t } from '@/locales'
import type { ThemeEnvironment } from '@/theme/api/v1'

/**
 * Browser/runtime environment the theme home needs: connectivity, viewport,
 * language, colour scheme and reduced-motion, plus the PWA readiness badge.
 *
 * It owns its own listeners so the host does not have to remember to tear them
 * down, and it reports the two changes that other concerns must react to through
 * callbacks rather than reaching into their state:
 *
 * - the viewport changed → the Core monitor band is positioned in viewport units
 *   and has to be re-measured;
 * - connectivity returned → the collection should reload.
 */
export function useHomeEnvironment(input: {
  onViewportChanged: () => void
  reloadHomeData: (forceRefresh?: boolean) => void
}) {
  const isOnline = ref(typeof navigator === 'undefined' ? true : navigator.onLine)
  const runtimeViewport = ref({ width: window.innerWidth, height: window.innerHeight })
  const runtimeLanguage = ref(document.documentElement.lang || navigator.language)
  const runtimeColorScheme = ref<'light' | 'dark'>(document.documentElement.classList.contains('dark') ? 'dark' : 'light')
  const reducedMotionMedia = window.matchMedia('(prefers-reduced-motion: reduce)')
  const colorSchemeMedia = window.matchMedia('(prefers-color-scheme: dark)')
  const runtimeReducedMotion = ref(reducedMotionMedia.matches)
  const pwaReady = ref(false)
  let environmentObserver: MutationObserver | undefined

  function syncThemeEnvironment() {
    runtimeViewport.value = { width: window.innerWidth, height: window.innerHeight }
    runtimeLanguage.value = document.documentElement.lang || navigator.language
    runtimeColorScheme.value = document.documentElement.classList.contains('dark') ? 'dark' : 'light'
    runtimeReducedMotion.value = reducedMotionMedia.matches
    input.onViewportChanged()
  }

  function handleOnline() {
    isOnline.value = true
    input.reloadHomeData(true)
  }

  function handleOffline() {
    isOnline.value = false
  }

  function retryWhenOnline() {
    window.location.reload()
  }

  async function updatePwaReady() {
    if (!('serviceWorker' in navigator) || !navigator.serviceWorker.controller) return
    const registration = await navigator.serviceWorker.ready
    pwaReady.value = registration.active?.state === 'activated' && !!navigator.serviceWorker.controller
  }

  /** Handed to the sandbox/trusted theme transport. The transport stamps the
   *  API version itself, which is why it is omitted here. */
  const runtimeEnvironment = computed<Omit<ThemeEnvironment, 'apiVersion'>>(() => ({
    coreVersion: import.meta.env.VITE_APP_VERSION || '0.0.0',
    language: runtimeLanguage.value,
    colorScheme: runtimeColorScheme.value,
    reducedMotion: runtimeReducedMotion.value,
    online: isOnline.value,
    // Runtime boundaries only accept plain structured-clone data. The viewport
    // source can be reactive, so copy its scalar fields before entering either
    // the sandbox or trusted transport.
    viewport: {
      width: Number(runtimeViewport.value.width),
      height: Number(runtimeViewport.value.height),
    },
    // The theme holds no locale bundle, so the Core supplies the labels it
    // renders. Keys are stable identifiers; the theme keeps its own fallback so
    // a Core older than a key still renders correctly.
    labels: {
      'search.placeholder': t('deskModule.searchBox.inputPlaceholder'),
      'item.edit': t('iconItem.edit'),
      'item.delete': t('common.delete'),
      'item.moveUp': t('iconItem.moveUp'),
      'item.moveDown': t('iconItem.moveDown'),
      'group.addItem': t('themeHome.addItem'),
      'group.edit': t('themeHome.editGroup'),
      'group.delete': t('themeHome.deleteGroup'),
      'group.moveUp': t('themeHome.moveGroupUp'),
      'group.moveDown': t('themeHome.moveGroupDown'),
      'actions.commands': t('themeHome.commands'),
      'actions.addGroup': t('themeHome.addGroup'),
      'actions.style': t('themeHome.style'),
      'dialog.editGroup': t('themeHome.editGroup'),
      'dialog.addGroup': t('themeHome.addGroup'),
      'dialog.save': t('themeHome.saveGroup'),
      'dialog.create': t('themeHome.createGroup'),
      'dialog.groupName': t('themeHome.groupName'),
      'dialog.groupIcon': t('themeHome.groupIcon'),
      'dialog.cancel': t('common.cancel'),
      'dialog.nameRequired': t('themeHome.groupNameRequired'),
      'collection.loading': t('themeHome.loadingItems'),
      'collection.loadFailed': t('themeHome.loadItemsFailed'),
      'collection.noMatch': t('themeHome.noMatchingItems'),
      'collection.empty': t('themeHome.noItems'),
      'actions.edit': t('themeHome.edit'),
      'actions.save': t('themeHome.save'),
      'actions.cancel': t('common.cancel'),
      'status.layoutSaved': t('themeHome.layoutSaved'),
      'status.layoutSaveFailed': t('themeHome.layoutSaveFailed'),
      // The Core app pages a theme can render, and the app hub that lists them.
      'actions.apps': t('appLauncher.title'),
      'actions.back': t('themeRecovery.home'),
      'apps.title': t('appLauncher.title'),
      'apps.userInfo': t('apps.userInfo.appName'),
      'apps.style': t('apps.baseSettings.appName'),
      'apps.spaceManage': t('spaceManage.title'),
      'apps.users': t('adminSettingUsers.appName'),
      'apps.about': t('apps.about.appName'),
    },
  }))

  onMounted(() => {
    window.addEventListener('online', handleOnline)
    window.addEventListener('offline', handleOffline)
    window.addEventListener('resize', syncThemeEnvironment)
    colorSchemeMedia.addEventListener('change', syncThemeEnvironment)
    reducedMotionMedia.addEventListener('change', syncThemeEnvironment)
    environmentObserver = new MutationObserver(syncThemeEnvironment)
    environmentObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'lang'] })
    syncThemeEnvironment()
    void updatePwaReady()
    navigator.serviceWorker?.addEventListener('controllerchange', updatePwaReady)
  })

  onUnmounted(() => {
    window.removeEventListener('online', handleOnline)
    window.removeEventListener('offline', handleOffline)
    window.removeEventListener('resize', syncThemeEnvironment)
    colorSchemeMedia.removeEventListener('change', syncThemeEnvironment)
    reducedMotionMedia.removeEventListener('change', syncThemeEnvironment)
    environmentObserver?.disconnect()
    navigator.serviceWorker?.removeEventListener('controllerchange', updatePwaReady)
  })

  return {
    isOnline,
    runtimeViewport,
    runtimeLanguage,
    runtimeColorScheme,
    runtimeReducedMotion,
    pwaReady,
    runtimeEnvironment,
    syncThemeEnvironment,
    handleOnline,
    handleOffline,
    retryWhenOnline,
  }
}
