import { computed, getCurrentInstance, onMounted, onUnmounted, reactive, ref, watch } from 'vue'
import { parsePublicCodeFromPath } from '@/utils/request/axios'
import { useAuthStore, usePanelState } from '@/store'
import { PanelStateNetworkModeEnum } from '@/enums'
import { resolvePanelValue } from '@/utils/theme'
import { PanelPanelConfigStyleEnum } from '@/enums'
import { activeThemeWallpaper, reconcileSpaceTheme } from '@/hooks/useTheme'
import { spaceDisplayName } from '@/api/panel/space'
import { useHomeEnvironment } from '@/core/home/useHomeEnvironment'
import { useHomePublicAccess } from '@/core/home/useHomePublicAccess'
import { useHomeData } from '@/core/home/useHomeData'
import { useHomeModals } from '@/core/home/useHomeModals'
import { useThemeRuntime } from '@/core/home/useThemeRuntime'
import { useThemeMonitor } from '@/core/home/useThemeMonitor'
import { useHomeCommands } from '@/core/home/useHomeCommands'
import { setThemeMonitorBridge } from '@/core/home/monitorBridge'
// Registers the `<yin-system-monitor>` custom element so a theme can embed the
// Core's own monitor component. See `theme/bridge/systemMonitor.ts`.
import '@/theme/bridge/systemMonitor'

/**
 * The home page's view model.
 *
 * `views/home/index.vue` used to own all of this directly, which made the host the
 * largest file in the frontend and mixed wiring with markup. Everything that is not
 * markup now lives here: the seven `core/home` composables are called in their
 * dependency order, the host-level derived values and lifecycle hooks sit beside
 * them, and the result is handed to `HomeChrome.vue` as a few cohesive groups.
 *
 * The groups are reactive objects rather than a flat return because the template
 * has to reach every one of these values, and `<script setup>` would otherwise force
 * the host to destructure ~100 names — which is exactly the size problem this
 * addresses. Grouping by concern keeps it a view model instead of a grab bag.
 *
 * Ordering note: the composables are mutually dependent (public access needs the
 * data loader, the theme gates need `publicAccessReady`, the monitor needs
 * `themeRuntimeActive`, the runtime snapshot needs the monitor's reserved height).
 * Cycles are broken by passing lazy getters, so the call order below stays acyclic.
 */
export function useHomeShell() {
  const panelState = usePanelState()
  const authStore = useAuthStore()
  const publicCode = parsePublicCodeFromPath()
  const previewTheme = new URLSearchParams(window.location.search).has('themePreview')

  const useThemeDefaults = computed(() => previewTheme || !!panelState.panelConfig.useThemeDefaults)
  const useThemeColors = computed(() => useThemeDefaults.value || panelState.panelConfig.wallpaperMode === 'theme')
  // Mirrors the resolution order in WallpaperLayer.vue: the layer renders unless the
  // user explicitly disabled wallpapers, so content above it must know whether
  // artwork is actually painting behind it.
  const wallpaperActive = computed(() => {
    if (previewTheme) return true
    const config = panelState.panelConfig
    if (config.wallpaperMode === 'none') return false
    return config.wallpaperMode === 'custom'
      || config.wallpaperMode === 'theme'
      || !!config.backgroundImageSrc
      || !!activeThemeWallpaper.value
  })
  const panelIconTextColor = computed(() => resolvePanelValue('var(--yin-text)', panelState.panelConfig.iconTextColor, useThemeDefaults.value))
  const directoryLayout = computed(() => panelState.panelConfig.homeLayout === 'directory')

  // The data composable must drop the command layer's cached remote items when the
  // collection is invalidated, but the command layer is created further down. A
  // mutable holder keeps the dependency lazy instead of forward-referencing it.
  let invalidateRemoteCommandItems: () => void = () => {}

  const {
    isOnline,
    pwaReady,
    runtimeEnvironment,
    retryWhenOnline,
  } = useHomeEnvironment({
    onViewportChanged: () => measureThemeMonitorReservation(),
    reloadHomeData: forceRefresh => void loadHomeData(forceRefresh),
  })

  // Core-owned layout edit signal. The value is carried to the theme through the
  // environment, and the theme toggles its own edit mode when the token changes,
  // so the two never need a shared editing flag across the sandbox boundary.
  const layoutEditToken = ref(0)
  const themeRuntimeEnvironment = computed(() => ({ ...runtimeEnvironment.value, editToken: layoutEditToken.value }))

  // Wired before the theme gates below because they read `publicAccessReady` to
  // decide whether a public link may hand the theme read scopes.
  const {
    publicAccessCode,
    publicAccessReady,
    publicAccessChecking,
    resolvePublicAccessMode,
    unlockPublicAccess,
  } = useHomePublicAccess({
    publicCode,
    // Lazy: `activeSpace` and `loadHomeData` belong to the data composable, which is
    // created after this one because it reads `publicAccessReady` from here.
    getActiveSpace: () => activeSpace.value,
    loadHomeData: () => loadHomeData(),
  })

  const {
    spaces,
    activeSpace,
    spaceSelectorOptions,
    items,
    homeReady,
    homeCollectionStatus,
    homeCollectionError,
    themeSnapshotVersion,
    cacheUpdatedAt,
    hasValidCachedHome,
    offlineUnavailable,
    canWrite,
    sideSwitching,
    homeSearch,
    homeCommandSearch,
    loadHomeData,
    getList,
    reloadSpaces,
    selectSpace,
    togglePanelSide,
    handleSpacesChanged,
    refreshCurrentSpace,
    warmSearchCache,
    clearCachedSpace,
  } = useHomeData({
    publicCode,
    publicAccessReady,
    isOnline,
    onCollectionInvalidated: () => invalidateRemoteCommandItems(),
    onMonitorStatus: status => applyMonitorStatus(status),
  })

  const {
    editItemInfoShow,
    editItemInfoData,
    windowShow,
    windowSrc,
    windowTitle,
    windowIframeIsLoad,
    currentAddItenIconGroupId,
    settingModalShow,
    createSpaceVisible,
    spaceName,
    creatingSpace,
    groupCreateVisible,
    groupName,
    creatingGroup,
    homeMutations,
    openPage,
    getItemOpenUrl,
    handWindowIframeIdLoad,
    handleEditItem,
    handleAddItem,
    submitCreateGroup,
    submitCreateSpace,
    handleChangeNetwork,
  } = useHomeModals({
    activeSpace,
    canWrite,
    reloadSpaces,
    refreshCollection: forceRefresh => getList(forceRefresh),
    invalidateSpaceCache: clearCachedSpace,
  })

  /** Handle to the mounted theme host. It is a function ref because the element is
   *  bound from `HomeChrome`, and the Core cannot reach inside the sandbox to scroll
   *  it itself. */
  const themeHostRef = ref<{ scrollToTop: () => void; hasView?: (name: string) => boolean; mountView?: (name: string, element: HTMLElement) => Promise<{ unmount: () => void | Promise<void> }>; getScopeClass?: () => string }>()
  function setThemeHostElement(element: Element | { $el?: unknown } | null) {
    themeHostRef.value = element && 'scrollToTop' in element ? element as typeof themeHostRef.value : undefined
  }
  function scrollToTop() {
    themeHostRef.value?.scrollToTop()
  }

  // The theme runtime mounts asynchronously; `ready` is emitted once the host has
  // mounted it. Surface routes need this signal because the runtime handle is a
  // plain variable, so `hasView` going false→true is not reactive on its own.
  const themeRuntimeReady = ref(false)
  function handleThemeRuntimeReady() { themeRuntimeReady.value = true }

  // A theme can contribute views beyond the home (theme-settings, theme-page).
  // The Core renders one on its own route (`/theme/:view`, mounted by
  // `ThemeSurface.vue`) and keeps its own page as the fallback when the theme
  // does not provide the view.
  function themeHasSurface(name: string) {
    return themeHostRef.value?.hasView?.(name) === true
  }
  async function mountThemeSurface(name: string, element: HTMLElement) {
    const view = await themeHostRef.value?.mountView?.(name, element)
    if (!view) throw new Error(`Theme does not provide the ${name} surface`)
    return view
  }
  /** The light-DOM scope class of the mounted theme, so a surface route can make
   *  the theme's own styles and tokens apply to its page. Empty when not light-mounted. */
  function themeScopeClass() {
    return themeHostRef.value?.getScopeClass?.() ?? ''
  }

  const {
    themeRuntimePackage,
    themeExecutionMode,
    themeMountMode,
    themeRequiredPermissions,
    themeRuntimeActive,
    themeRuntimeNeedsConsent,
    homeFallbackVisible,
    homeFallbackReason,
    themeRuntimePermissions,
    themeCanWriteGroups,
    themeRuntimeSlots,
    themeRuntimeSnapshot,
    themeRuntimeGrant,
    trustedRuntimeAvailable,
    themeRuntimeGrantSaving,
    themeRuntimeConsentVisible,
    selectedThemeExecutionMode,
    trustedRuntimeAcknowledged,
    themeRuntimeFailureMessage,
    themeSafeMode,
    themePersistence,
    createThemeRuntimeError,
    confirmThemeDelete,
    grantThemeRuntimePermissions,
    openThemeRuntimeConsent,
    handleThemeRuntimeFailure,
  } = useThemeRuntime({
    publicCode,
    publicAccessReady,
    previewTheme,
    homeReady,
    themeSnapshotVersion,
    homeCollectionStatus,
    homeCollectionError,
    spaces,
    activeSpace,
    items,
    canWrite,
    environment: themeRuntimeEnvironment,
    // The theme opens "new window" bookmarks inside the tap, so the snapshot has to
    // carry the resolved address. Handing over the Core's own resolver is what keeps
    // the two from picking different URLs for the same item.
    getItemOpenUrl,
    getMonitorReservedHeight: () => themeMonitorReservedHeight.value,
    getSearchConfiguration: () => ({
      engines: themeSearchEngineConfiguration.value.engines.map(engine => ({ id: engine.id, title: engine.title, iconSrc: engine.iconSrc, url: engine.url })),
      currentSearchEngine: { ...themeSearchEngineConfiguration.value.currentSearchEngine },
    }),
  })

  // Re-resolve the theme when the active space changes, passing its id so the
  // server can apply a space theme. The resolver returns the same revision
  // today, so this never reloads the mounted theme. Watching the theme-active
  // flag too, because the theme can become active after the space is known.
  watch([() => activeSpace.value?.id, themeRuntimeActive], ([spaceId, active]) => {
    if (active && spaceId)
      void reconcileSpaceTheme(spaceId)
  })

  const {
    monitorEnabled,
    monitorSnapshotController,
    applyMonitorStatus,
    setLayerElement: setMonitorLayerElement,
    reservedHeight: themeMonitorReservedHeight,
    layerTop: themeMonitorTop,
    measureReservation: measureThemeMonitorReservation,
    reportSearchBottom,
    getMonitorSnapshot,
  } = useThemeMonitor({ isThemeActive: () => themeRuntimeActive.value })

  const {
    commandCenterVisible,
    commandCenterQuery,
    commandCenterSelectedIndex,
    remoteCommandItems,
    commandCenterSearchEngine,
    themeSearchEngineConfiguration,
    filteredCommandDefinitions,
    commandCenterItems,
    closeCommandCenter,
    moveCommandSelection,
    selectCommandItem,
    submitCommandCenterSearch,
    executeCommand,
    executeCommandItem,
    updateCommandCenterQuery,
    handleGlobalKeydown,
    executeThemeRequest,
  } = useHomeCommands({
    publicCode,
    publicAccessReady,
    items,
    activeSpace,
    spaces,
    canWrite,
    selectSpace,
    togglePanelSide,
    refreshCollection: forceRefresh => getList(forceRefresh),
    warmSearchCache,
    homeSearch,
    homeCommandSearch,
    openPage,
    getItemOpenUrl,
    handleEditItem,
    handleAddItem,
    handleChangeNetwork,
    homeMutations,
    groupCreateVisible,
    createSpaceVisible,
    settingModalShow,
    editItemInfoShow,
    windowShow,
    themeCanWriteGroups,
    themePersistence,
    createThemeRuntimeError,
    confirmThemeDelete,
    reportSearchBottom,
    getMonitorSnapshot,
    scrollToTop,
    hasThemeSurface: themeHasSurface,
  })

  invalidateRemoteCommandItems = () => { remoteCommandItems.value = [] }

  /** Opening a bookmark from the fallback resolves the record here and goes through
   *  exactly the same open path as the full home (LAN/WAN and mobile aware), so a
   *  degraded view never changes which URL a bookmark resolves to. */
  function openFallbackItem(itemId?: number | string) {
    if (itemId === undefined) return
    const record = items.value.flatMap(group => group.items || []).find(item => String(item.id) === String(itemId))
    if (record) openPage(record.openMethod, getItemOpenUrl(record), record.title)
  }

  const activeSpaceLabel = computed(() => activeSpace.value
    ? spaceDisplayName(activeSpace.value, spaces.value, authStore.userInfo?.id)
    : '')
  // A public link has no space to select: it shows the one space it points at.
  // `authStore.loggedIn` alone is not a safe test, because a visitor who is (or
  // was) signed in keeps that flag while opening a public link, so the bar would
  // render for exactly that visitor; `!publicCode` is what actually gates it.
  const showSpaceBar = computed(() => homeReady.value && spaces.value.length > 0 && authStore.loggedIn && !publicCode)

  /**
   * The fixed utility stack on the right edge (refresh, back to top, LAN/WAN and
   * system settings).
   *
   * It lives in the Core rather than in the theme, which is the one place it can
   * work: the theme runs in a frame sized to its full content height so the Core
   * can scroll it, and inside that frame `position: fixed` resolves against the
   * content box, not the viewport — a theme-drawn stack would sit at the bottom of
   * the page and only appear once you scrolled there. It is viewport chrome, the
   * same class of thing as the space bar and the monitor band, and every button
   * calls a Core capability directly instead of a round trip through the sandbox.
   */
  const showFloatingBar = computed(() => homeReady.value && (authStore.loggedIn || !!publicCode))
  // System settings are an operator surface, so a public link gets the other
  // three buttons but not this one.
  const showSystemSettingsButton = computed(() => authStore.loggedIn && !publicCode)
  const networkMode = computed<'lan' | 'wan'>(() => panelState.networkMode === PanelStateNetworkModeEnum.lan ? 'lan' : 'wan')
  const showNetworkSwitch = computed(() => panelState.panelConfig.netModeChangeButtonShow === true)
  function toggleNetworkMode() {
    handleChangeNetwork(networkMode.value === 'lan' ? PanelStateNetworkModeEnum.wan : PanelStateNetworkModeEnum.lan)
  }

  watch(directoryLayout, (enabled) => {
    document.documentElement.dataset.yinLayout = enabled ? 'directory' : 'standard'
    if (enabled)
      items.value = []
  }, { immediate: true })

  onMounted(() => {
    window.addEventListener('keydown', handleGlobalKeydown)
    // The environment composable owns its own listeners; only the visualViewport
    // resize needs an extra hook here, which it does not observe.
    window.visualViewport?.addEventListener('resize', measureThemeMonitorReservation)
    if (publicCode && !publicAccessReady.value) {
      void resolvePublicAccessMode().then(() => {
        if (publicAccessReady.value)
          loadHomeData()
      })
      return
    }
    loadHomeData()
  })

  onUnmounted(() => {
    delete document.documentElement.dataset.yinLayout
    window.removeEventListener('keydown', handleGlobalKeydown)
    window.visualViewport?.removeEventListener('resize', measureThemeMonitorReservation)
  })

  /** Root shell classes and the theme frame. */
  // Monitor-layer placement and brand text are derived here so the template never
  // reaches into `panelState` itself.
  const monitorVisible = computed(() => monitorEnabled.value && panelState.panelConfig.systemMonitorShow)
  const monitorIsInfo = computed(() => panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info)
  const contentTopVh = computed(() => panelState.panelConfig.marginTop ?? 10)
  const monitorShowTitle = computed(() => panelState.panelConfig.systemMonitorShowTitle)
  const brandText = computed(() => panelState.panelConfig.logoText || 'Yin-Panel')

  // Publish the Core-owned monitor data for the `<yin-system-monitor>` component
  // bridge, so a theme can embed the monitor itself and get the same rendering.
  // Cleared with the shell so the element never holds a stale controller.
  const appContext = getCurrentInstance()?.appContext
  if (appContext) {
    watch([monitorShowTitle, panelIconTextColor], () => {
      setThemeMonitorBridge({
        controller: monitorSnapshotController,
        showTitle: monitorShowTitle.value,
        iconTextColor: panelIconTextColor.value,
        appContext,
      })
    }, { immediate: true })
    onUnmounted(() => setThemeMonitorBridge(undefined))
  }

  const theme = reactive({
    useThemeColors,
    wallpaperActive,
    themeRuntimeActive,
    themeRuntimePackage,
    themeExecutionMode,
    themeMountMode,
    themeRuntimeGrant,
    themeRuntimeSnapshot,
    themeRuntimeEnvironment,
    themeRuntimePermissions,
    themeRuntimeSlots,
    executeThemeRequest,
    handleThemeRuntimeFailure,
    setThemeHostElement,
    hasThemeSurface: themeHasSurface,
    mountThemeSurface,
    themeScopeClass,
    themeRuntimeReady,
    handleThemeRuntimeReady,
    monitorEnabled,
    monitorVisible,
    monitorIsInfo,
    monitorShowTitle,
    setMonitorLayerElement,
    monitorSnapshotController,
    panelIconTextColor,
    themeMonitorTop,
    contentTopVh,
    brandText,
    homeFallbackVisible,
    homeFallbackReason,
    themeSafeMode,
  })

  /** Space switcher, connectivity status and the fallback's data. */
  const home = reactive({
    canWrite,
    requestLayoutEdit: () => { layoutEditToken.value++ },
    setPublicAccessCode,
    homeReady,
    spaces,
    activeSpace,
    activeSpaceLabel,
    showSpaceBar,
    spaceSelectorOptions,
    selectSpace,
    items,
    publicCode,
    publicAccessReady,
    publicAccessChecking,
    publicAccessCode,
    unlockPublicAccess,
    isOnline,
    pwaReady,
    hasValidCachedHome,
    cacheUpdatedAt,
    offlineUnavailable,
    retryWhenOnline,
    refreshCurrentSpace,
    openFallbackItem,
    sideSwitching,
    directoryLayout,
    showFloatingBar,
    showSystemSettingsButton,
    scrollToTop,
    networkMode,
    showNetworkSwitch,
    toggleNetworkMode,
  })

  // Writes go through explicit setters: `HomeChrome` receives these groups as props,
  // and two-way binding directly onto a prop would mutate it (`vue/no-mutating-props`).
  // Setters keep the flow one-directional while still being terse at the call site.
  function setPublicAccessCode(value: string) { publicAccessCode.value = value }

  /** Every dialog the home owns, plus the theme consent prompt. */
  const dialogs = reactive({
    setSpaceName: (value: string) => { spaceName.value = value },
    setGroupName: (value: string) => { groupName.value = value },
    setSelectedThemeExecutionMode: (value: 'sandbox' | 'trusted') => { selectedThemeExecutionMode.value = value },
    setTrustedRuntimeAcknowledged: (value: boolean) => { trustedRuntimeAcknowledged.value = value },
    setWindowShow: (value: boolean) => { windowShow.value = value },
    setEditItemInfoShow: (value: boolean) => { editItemInfoShow.value = value },
    setCreateSpaceVisible: (value: boolean) => { createSpaceVisible.value = value },
    setGroupCreateVisible: (value: boolean) => { groupCreateVisible.value = value },
    setSettingModalShow: (value: boolean) => { settingModalShow.value = value },
    setThemeRuntimeConsentVisible: (value: boolean) => { themeRuntimeConsentVisible.value = value },
    settingModalShow,
    handleSpacesChanged,
    editItemInfoShow,
    editItemInfoData,
    currentAddItenIconGroupId,
    homeMutations,
    windowShow,
    windowSrc,
    windowTitle,
    windowIframeIsLoad,
    handWindowIframeIdLoad,
    createSpaceVisible,
    spaceName,
    creatingSpace,
    submitCreateSpace,
    groupCreateVisible,
    groupName,
    creatingGroup,
    submitCreateGroup,
    themeRuntimeNeedsConsent,
    themeRuntimeConsentVisible,
    selectedThemeExecutionMode,
    trustedRuntimeAvailable,
    trustedRuntimeAcknowledged,
    themeRequiredPermissions,
    themeRuntimeGrantSaving,
    themeRuntimeFailureMessage,
    grantThemeRuntimePermissions,
    openThemeRuntimeConsent,
  })

  /** The command centre surface. */
  const commands = reactive({
    commandCenterVisible,
    commandCenterQuery,
    commandCenterItems,
    filteredCommandDefinitions,
    commandCenterSelectedIndex,
    commandCenterSearchEngine,
    updateCommandCenterQuery,
    moveCommandSelection,
    selectCommandItem,
    submitCommandCenterSearch,
    executeCommandItem,
    executeCommand,
    closeCommandCenter,
  })

  return { theme, home, dialogs, commands }
}
