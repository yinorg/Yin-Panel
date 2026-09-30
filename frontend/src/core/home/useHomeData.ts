import { computed, onUnmounted, ref, type Ref } from 'vue'
import { getGroups, getItems, getSpaces, sortSpaces, spaceDisplayName, type Space } from '@/api/panel/space'
import { getEnableStatus } from '@/api/system/systemMonitor'
import { useAuthStore, usePanelState } from '@/store'
import { setTitle } from '@/utils/cmn'
import { clearSpaceCache, readSpaceCache, readSpacesCache, writeSpaceCache, writeSpacesCache } from '@/utils/spaceCache'
import { t } from '@/locales'
import { useMessage } from 'naive-ui'
import { createHomeBootstrap } from '@/core/home/bootstrap'
import { createHomeCollectionLoader, type HomeCollectionResult } from '@/core/home/collection'
import { createHomeCommandSearch } from '@/core/home/commandCenter'
import { buildHomeGroupTree } from '@/core/home/groupTree'
import { createHomeSearchService } from '@/core/home/search'
import { createHomeSpaceController } from '@/core/home/spaceController'
import type { ThemeCollectionStatus } from '@/theme/api/v1'

/** Mirrors the host's local group shape (a group tree node with its items). */
export interface HomeGroupNode extends Panel.ItemIconGroup {
  items?: Panel.ItemInfo[]
  depth?: number
}

export interface HomeMonitorStatus {
  enabled: boolean
  refreshInterval?: number
}

const HOME_REQUEST_TIMEOUT = 3000
const SIDE_SWITCH_ANIMATION_MS = 1000

/**
 * Data channel for the home page: spaces, the active space, the collection with
 * its cache, and the bootstrap that feeds both. This is Core's responsibility C1
 * — the theme never talks HTTP, it receives the projection of this state.
 *
 * Cross-concern effects are reported as callbacks instead of being written
 * directly, so this composable never depends on the command or modal layers:
 *
 * - invalidating the collection must drop the command layer's cached remote
 *   items (they belong to the space that just went away);
 * - the bootstrap reports monitor availability, which the host turns into the
 *   monitor snapshot controller.
 */
export function useHomeData(input: {
  publicCode: string
  publicAccessReady: Ref<boolean>
  isOnline: Ref<boolean>
  onCollectionInvalidated: () => void
  onMonitorStatus?: (status: HomeMonitorStatus) => void
}) {
  const panelState = usePanelState()
  const authStore = useAuthStore()
  const ms = useMessage()

  const spaces = ref<Space[]>([])
  const activeSpace = ref<Space | null>(null)
  const spaceSelectorOptions = computed(() => spaces.value
    .filter(space => space.id !== activeSpace.value?.id)
    .map(space => ({ label: spaceDisplayName(space, spaces.value, authStore.userInfo?.id), key: space.id })))

  const cacheUpdatedAt = ref<number | null>(null)
  const hasValidCachedHome = ref(false)
  const homeReady = ref(false)
  const homeCollectionStatus = ref<ThemeCollectionStatus>('idle')
  const homeCollectionError = ref<{ code: string, message: string }>()
  const themeSnapshotVersion = ref(0)
  const sideSwitching = ref(false)
  let homeTransitionGeneration = 0
  let sideSwitchTimer: ReturnType<typeof setTimeout> | undefined

  const items = ref<HomeGroupNode[]>([])
  const collectionSpaceId = ref<number | null>(null)

  // A public link stores its cache per session: it must not survive into a later
  // signed-in visit under the same browser profile.
  const sessionOnlyCache = !!input.publicCode
  function getCachedSpace(spaceId: number) { return readSpaceCache(spaceId, authStore.userInfo?.id, sessionOnlyCache) }
  function saveCachedSpace(spaceId: number, cache: any) { writeSpaceCache(spaceId, cache, authStore.userInfo?.id, sessionOnlyCache) }
  function clearCachedSpace(spaceId: number) { clearSpaceCache(spaceId, authStore.userInfo?.id, sessionOnlyCache) }

  const offlineUnavailable = computed(() => homeReady.value && !input.isOnline.value && !hasValidCachedHome.value)
  const canEditActiveSpace = computed(() => !input.publicCode && activeSpace.value?.canEdit !== false)
  const canWrite = computed(() => input.isOnline.value && !offlineUnavailable.value && canEditActiveSpace.value)

  function applyGroups(data: HomeGroupNode[], itemsByGroup = new Map<number, Panel.ItemInfo[]>(), spaceId = activeSpace.value?.id) {
    collectionSpaceId.value = spaceId ?? null
    const { groups: flattened } = buildHomeGroupTree(data, itemsByGroup)
    items.value = flattened as HomeGroupNode[]
    themeSnapshotVersion.value++
  }

  function applyCollectionProgress(result: HomeCollectionResult<HomeGroupNode, Panel.ItemInfo>) {
    if (activeSpace.value?.id !== result.spaceId) return
    applyGroups(result.groups, result.itemsByGroup, result.spaceId)
  }

  const homeCollectionLoader = createHomeCollectionLoader<HomeGroupNode, Panel.ItemInfo>({
    getGroups: (spaceId, signal) => getGroups<HomeGroupNode[]>(spaceId, signal),
    getItems: (spaceId, groupId, page, pageSize, signal) => getItems<Panel.ItemInfo[]>(spaceId, groupId, page, pageSize, signal),
    readCache: getCachedSpace,
    writeCache: saveCachedSpace,
    isOnline: () => input.isOnline.value,
    timeoutMs: HOME_REQUEST_TIMEOUT,
    onProgress: applyCollectionProgress,
  })

  const homeBootstrap = createHomeBootstrap({
    // The monitor band is Core chrome for a signed-in session. Its enabling flag
    // and its data endpoint are both JWT-only, so a public link can never render
    // the band; probing anyway only earns a 1005 on every open.
    getMonitor: signal => input.publicCode
      ? Promise.resolve({ code: 0, msg: '', data: { enabled: false } })
      : getEnableStatus<{ enabled: boolean, refresh_interval?: number }>(signal),
    getSpaces: signal => getSpaces<Space[]>(signal),
    refreshConfig: signal => panelState.updatePanelConfigByCloud(signal),
    timeoutMs: HOME_REQUEST_TIMEOUT,
  })

  const homeSpaceController = createHomeSpaceController<Space>({
    getSpaces: signal => getSpaces<Space[]>(signal),
    sortSpaces: value => sortSpaces(value, authStore.userInfo?.id),
    getCurrentSpaces: () => spaces.value,
    readCachedSpaces: () => readSpacesCache(authStore.userInfo?.id, sessionOnlyCache) as Space[] | null,
    writeCachedSpaces: value => writeSpacesCache(value, authStore.userInfo?.id, sessionOnlyCache),
    getActiveSpaceId: () => activeSpace.value?.id,
    setSpaces: value => { spaces.value = value },
    setActiveSpace: value => { activeSpace.value = value },
    loadSpace: spaceId => getListForSpace(spaceId),
    canWrite: () => canWrite.value,
  })

  const homeSearch = createHomeSearchService<Panel.ItemInfo>({
    fetchPage: (spaceId, page, pageSize, signal) => getItems<Panel.ItemInfo[]>(spaceId, undefined, page, pageSize, signal),
    isOnline: () => input.isOnline.value,
  })

  const homeCommandSearch = createHomeCommandSearch<Panel.ItemInfo>({
    getSpaceId: () => activeSpace.value?.id,
    search: (spaceId, query) => homeSearch.query(spaceId, query),
  })

  async function getList(forceRefresh = false) {
    if (forceRefresh && !canWrite.value) return
    const transitionGeneration = homeTransitionGeneration
    themeSnapshotVersion.value++
    homeCollectionStatus.value = 'loading'
    homeCollectionError.value = undefined
    if (!activeSpace.value) {
      items.value = []
      collectionSpaceId.value = null
      homeCollectionStatus.value = 'empty'
      return
    }

    const targetSpaceId = activeSpace.value.id
    if (collectionSpaceId.value !== targetSpaceId) {
      items.value = []
      collectionSpaceId.value = targetSpaceId
    }
    if (forceRefresh) homeSearch.invalidate(targetSpaceId)
    const result = await homeCollectionLoader.load(targetSpaceId, forceRefresh)
    if (result.cancelled || transitionGeneration !== homeTransitionGeneration || activeSpace.value?.id !== result.spaceId) return
    applyCollectionProgress(result)
    cacheUpdatedAt.value = result.cache.updatedAt || Date.now()
    hasValidCachedHome.value = !!getCachedSpace(result.spaceId)
    const itemCount = Array.from(result.itemsByGroup.values()).reduce((count, groupItems) => count + groupItems.length, 0)
    homeCollectionStatus.value = result.error?.code === 'LOAD_FAILED' || (result.failedGroups > 0 && !itemCount)
      ? 'error'
      : result.stale ? 'stale' : itemCount ? 'ready' : 'empty'
    homeCollectionError.value = result.error
  }

  async function getListForSpace(spaceId: number) {
    if (activeSpace.value?.id !== spaceId) return
    await getList()
  }

  function selectSpace(key: string | number) {
    homeTransitionGeneration++
    homeCollectionLoader.cancel()
    homeSearch.invalidate()
    homeCommandSearch.invalidate()
    input.onCollectionInvalidated()
    homeSpaceController.select(Number(key))
  }

  function togglePanelSide() {
    if (!activeSpace.value?.pairedSpaceId) return
    homeTransitionGeneration++
    const yin = activeSpace.value
    const target = yin.side === 'yang'
      ? spaces.value.find(space => space.id === yin.pairId)
      : spaces.value.find(space => space.id === yin.pairedSpaceId)
        || { ...yin, id: yin.pairedSpaceId!, name: `${yin.name}-B`, side: 'yang' as const, pairId: yin.id, pairedSpaceId: yin.id }
    if (!target) return
    homeCollectionLoader.cancel()
    homeSearch.invalidate()
    homeCommandSearch.invalidate()
    input.onCollectionInvalidated()
    activeSpace.value = target
    getList()
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    sideSwitching.value = true
    if (sideSwitchTimer) clearTimeout(sideSwitchTimer)
    sideSwitchTimer = setTimeout(() => {
      sideSwitching.value = false
      sideSwitchTimer = undefined
    }, SIDE_SWITCH_ANIMATION_MS)
  }

  async function loadHomeData(refresh = false) {
    const bootstrapGeneration = homeTransitionGeneration
    if (!refresh) homeReady.value = false
    if (panelState.panelConfig.logoText)
      setTitle(panelState.panelConfig.logoText)

    const cachedSpace = homeSpaceController.restoreCached()
    if (cachedSpace) {
      const cache = getCachedSpace(cachedSpace.id)
      hasValidCachedHome.value = !!cache
      cacheUpdatedAt.value = cache?.updatedAt || null
    }
    homeReady.value = true
    performance.mark('home-ready')

    if (!input.isOnline.value) return
    const result = await homeBootstrap.load()
    if (result.cancelled) return
    if (result.monitor?.code === 0)
      input.onMonitorStatus?.({ enabled: result.monitor.data.enabled, refreshInterval: result.monitor.data.refresh_interval })

    if (result.spaces?.data?.length) {
      const userChangedSpace = bootstrapGeneration !== homeTransitionGeneration
      const previousSpaceId = activeSpace.value?.id
      spaces.value = sortSpaces(result.spaces.data, authStore.userInfo?.id)
      writeSpacesCache(spaces.value, authStore.userInfo?.id, sessionOnlyCache)
      if (!userChangedSpace)
        activeSpace.value = spaces.value.find(space => space.id === previousSpaceId) || spaces.value[0]
      else if (activeSpace.value)
        activeSpace.value = spaces.value.find(space => space.id === activeSpace.value?.id) || activeSpace.value
      void getList()
    }
    else if (result.spacesFailed) {
      // Losing the space list is treated as lost connectivity.
      input.isOnline.value = false
    }
  }

  function reloadSpaces(selectLatest = false) {
    void homeSpaceController.refresh(selectLatest)
  }
  function handleSpacesChanged() {
    reloadSpaces()
  }

  async function refreshCurrentSpace() {
    if (!canWrite.value) return
    const authStorage = localStorage.getItem('authStorage')
    localStorage.clear()
    if (authStorage !== null) localStorage.setItem('authStorage', authStorage)
    sessionStorage.clear()
    if ('serviceWorker' in navigator) {
      const registrations = await navigator.serviceWorker.getRegistrations()
      await Promise.all(registrations.map(registration => registration.unregister()))
    }
    if (typeof caches !== 'undefined') {
      const cacheNames = await caches.keys()
      await Promise.all(cacheNames.map(cacheName => caches.delete(cacheName)))
    }
    ms.success(t('panelHome.refreshCacheSuccess'))
    await new Promise(resolve => setTimeout(resolve, 500))
    window.location.reload()
  }

  /** Warms the server-side match cache the command center reuses. */
  function warmSearchCache(keyword?: string) {
    const query = keyword?.trim() || ''
    const spaceId = activeSpace.value?.id
    if (query === '' || !spaceId || !panelState.panelConfig.searchBoxSearchIcon) return
    void homeSearch.query(spaceId, query).catch(() => {})
  }

  onUnmounted(() => {
    if (sideSwitchTimer) clearTimeout(sideSwitchTimer)
    homeCollectionLoader.cancel()
    homeBootstrap.cancel()
    homeSpaceController.cancel()
    homeSearch.cancel()
  })

  return {
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
    canEditActiveSpace,
    canWrite,
    sideSwitching,
    homeCollectionLoader,
    homeBootstrap,
    homeSpaceController,
    homeSearch,
    homeCommandSearch,
    getList,
    loadHomeData,
    selectSpace,
    togglePanelSide,
    reloadSpaces,
    handleSpacesChanged,
    refreshCurrentSpace,
    warmSearchCache,
    getCachedSpace,
    saveCachedSpace,
    clearCachedSpace,
    sessionOnlyCache,
  }
}
