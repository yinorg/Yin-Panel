<script setup lang="ts">
import { VueDraggable } from 'vue-draggable-plus'
import { NBackTop, NButton, NButtonGroup, NCard, NCheckbox, NDropdown, NInput, NModal, NRadio, NRadioGroup, NSkeleton, NSpin, NSpace, useDialog, useMessage } from 'naive-ui'
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { createGroup, createItem, createItemWithIcon, createSpace, deleteGroup, deleteItem as deleteSpaceItem, getGroups, getItems, getSpaces, sortGroups, sortItems, sortSpaces, spaceDisplayName, updateGroup, updateItem, type Space } from '../../api/panel/space'
import { uploadImage } from '@/api/panel/file'
import Clock from '../../components/deskModule/Clock/index.vue'
import SearchBox from '../../components/deskModule/SearchBox/index.vue'
import { replaceOrAppendKeywordToUrl, searchEngineList, type SearchEngine } from '../../components/deskModule/SearchBox/engines'
import SvgIcon from '../../components/common/SvgIcon/index.vue'
import AppIcon from './components/AppIcon/index.vue'
import CommandCenter from './components/CommandCenter/index.vue'
import WallpaperLayer from './components/WallpaperLayer.vue'
import { setTitle } from '@/utils/cmn'
import { parsePublicCodeFromPath } from '@/utils/request/axios'
import { usePanelState, useAuthStore } from '@/store'
import { PanelPanelConfigStyleEnum, PanelStateNetworkModeEnum } from '@/enums'
import { t } from '@/locales'
import { getDiskStateByPath, getEnableStatus, getSnapshot } from '@/api/system/systemMonitor'
import { clearSpaceCache, readSpaceCache, readSpacesCache, writeSpaceCache, writeSpacesCache } from '@/utils/spaceCache'
import { resolvePanelValue } from '@/utils/theme'
import { activeThemePackage, activeThemeSlots } from '@/hooks/useTheme'
import { getThemeRuntimeGrant, setThemeRuntimeGrant } from '@/api/theme'
import { createHomeThemeHandlers } from '@/core/home/themeHandlers'
import { executeHomeThemeRequest } from '@/core/home/themeRequest'
import { createThemePersistence } from '@/core/home/themePersistence'
import { createThemeSettingsSchemaValidator } from '@/core/home/themeSettingsSchema'
import { createHomeCollectionLoader, type HomeCollectionResult } from '@/core/home/collection'
import { createHomeBootstrap } from '@/core/home/bootstrap'
import { createHomeSpaceController } from '@/core/home/spaceController'
import { createHomeMutationService } from '@/core/home/mutations'
import { createHomeSearchService } from '@/core/home/search'
import { createHomeCommandSearch, filterHomeCommandItems, filterHomeCommands, findHomeCommandItem, getInitialHomeCommandSelection, isHomeCommandWrite, moveHomeCommandSelection, parseHomeCommand } from '@/core/home/commandCenter'
import { buildHomeGroupTree, getDirectoryGroups, getHomeGroupRoots, getInitiallyCollapsedGroups, isHomeGroupHidden, resolveActiveDirectoryId } from '@/core/home/groupTree'
import { createIconifyResourceResolver } from '@/core/home/iconifyResource'
import { resolveItemOpenUrl } from '@/core/items/openPolicy'
import { normalizeMonitorSnapshot } from '@/core/monitor/themeSnapshot'
import { createMonitorSnapshotController } from '@/core/monitor/snapshotController'
import { createThemeHomeSnapshot } from '@/core/home/themeSnapshot'
import type { ThemeCollectionStatus, ThemePermission } from '@/theme/api/v1'
import type { CoreMonitorSnapshot } from '@/core/monitor/themeSnapshot'
import ThemeHost from '@/theme/runtime/ThemeHost.vue'
import { isThemeSafeMode } from '@/theme/recovery/safeMode'

const SystemMonitor = defineAsyncComponent(() => import('../../components/deskModule/SystemMonitor/index.vue'))
const AppStarter = defineAsyncComponent(() => import('./components/AppStarter/index.vue'))
const EditItem = defineAsyncComponent(() => import('./components/EditItem/index.vue'))

interface ItemGroup extends Panel.ItemIconGroup {
  sortStatus?: boolean
  hoverStatus: boolean
  items?: Panel.ItemInfo[]
  depth?: number
}

const ms = useMessage()
const dialog = useDialog()
const panelState = usePanelState()
const authStore = useAuthStore()
const route = useRoute()
const router = useRouter()
const publicCode = parsePublicCodeFromPath()
const previewTheme = new URLSearchParams(window.location.search).has('themePreview')
const useThemeDefaults = computed(() => previewTheme || !!panelState.panelConfig.useThemeDefaults)
const useThemeColors = computed(() => useThemeDefaults.value || panelState.panelConfig.wallpaperMode === 'theme')
const panelIconTextColor = computed(() => resolvePanelValue('var(--yin-text)', panelState.panelConfig.iconTextColor, useThemeDefaults.value))
const directoryLayout = computed(() => panelState.panelConfig.homeLayout === 'directory')
const directoryRoots = computed(() => getHomeGroupRoots(items.value))
const activeDirectoryId = ref<number | null>(null)
const directoryItems = computed(() => {
  if (!directoryLayout.value) return filterItems.value
  if (filterItems.value.length !== items.value.length) return filterItems.value
  return getDirectoryGroups(items.value, resolveActiveDirectoryId(items.value, activeDirectoryId.value))
})

const scrollContainerRef = ref<HTMLElement | undefined>(undefined)

const editItemInfoShow = ref<boolean>(false)
const editItemInfoData = ref<Panel.ItemInfo | null>(null)
const windowShow = ref<boolean>(false)
const windowSrc = ref<string>('')
const windowTitle = ref<string>('')

const windowIframeIsLoad = ref<boolean>(false)

const dropdownMenuX = ref(0)
const dropdownMenuY = ref(0)
const dropdownShow = ref(false)
const currentRightSelectItem = ref<Panel.ItemInfo | null>(null)
const currentAddItenIconGroupId = ref<number | undefined>()

const settingModalShow = ref(false)
const spaces = ref<Space[]>([])
const activeSpace = ref<Space | null>(null)
const spaceSelectorOptions = computed(() => spaces.value
  .filter(space => space.id !== activeSpace.value?.id)
  .map(space => ({ label: spaceDisplayName(space, spaces.value, authStore.userInfo?.id), key: space.id })))
const createSpaceVisible = ref(false)
const spaceName = ref('')
const creatingSpace = ref(false)
const monitorEnabled = ref(false)
const monitorResultRefreshInterval = ref<number | undefined>()
const monitorSnapshotController = createMonitorSnapshotController<CoreMonitorSnapshot, SystemMonitor.DiskInfo>({
  fetchSnapshot: async () => {
    const result = await getSnapshot<CoreMonitorSnapshot>()
    if (result.code !== 0) throw createThemeRuntimeError('UNSUPPORTED_CAPABILITY', 'Monitor data is unavailable')
    return result.data
  },
  fetchDisk: path => getDiskStateByPath<SystemMonitor.DiskInfo>(path),
  getInterval: async () => Math.max(250, (monitorResultRefreshInterval.value || 10) * 1000),
})
const themeMonitorLayerRef = ref<HTMLElement>()
const themeMonitorReservedHeight = ref(0)
const themeHostRef = ref<{ scrollToTop: () => void }>()
let themeMonitorResizeObserver: ResizeObserver | undefined
let themeMonitorMeasureFrame = 0
function measureThemeMonitorReservation() {
  if (themeMonitorMeasureFrame) cancelAnimationFrame(themeMonitorMeasureFrame)
  themeMonitorMeasureFrame = requestAnimationFrame(() => {
    themeMonitorMeasureFrame = 0
    const element = themeMonitorLayerRef.value
    // The layer is absolutely positioned inside the scrolling shell, so its
    // offset box is stable across scrolls while a viewport-relative rect is not.
    const nextHeight = themeRuntimeActive.value && monitorEnabled.value && panelState.panelConfig.systemMonitorShow && element?.isConnected
      ? Math.min(window.innerHeight, Math.max(0, Math.ceil(element.offsetTop + element.offsetHeight)))
      : 0
    if (themeMonitorReservedHeight.value !== nextHeight)
      themeMonitorReservedHeight.value = nextHeight
  })
}
const isOnline = ref(typeof navigator === 'undefined' ? true : navigator.onLine)
const runtimeViewport = ref({ width: window.innerWidth, height: window.innerHeight })
const runtimeLanguage = ref(document.documentElement.lang || navigator.language)
const runtimeColorScheme = ref(document.documentElement.classList.contains('dark') ? 'dark' as const : 'light' as const)
const reducedMotionMedia = window.matchMedia('(prefers-reduced-motion: reduce)')
const colorSchemeMedia = window.matchMedia('(prefers-color-scheme: dark)')
const runtimeReducedMotion = ref(reducedMotionMedia.matches)
let runtimeEnvironmentObserver: MutationObserver | undefined
const pwaReady = ref(false)
const cacheUpdatedAt = ref<number | null>(null)
const hasValidCachedHome = ref(false)
const homeReady = ref(false)
const homeCollectionStatus = ref<ThemeCollectionStatus>('idle')
const homeCollectionError = ref<{ code: string; message: string }>()
const themeSnapshotVersion = ref(0)
const sideSwitching = ref(false)
let homeTransitionGeneration = 0
let sideSwitchTimer: ReturnType<typeof setTimeout> | undefined
const commandCenterVisible = ref(false)
const commandCenterQuery = ref('')
const commandCenterSelectedIndex = ref(-1)
const remoteCommandItems = ref<Panel.ItemInfo[]>([])
const commandCenterSearchEngine = ref<SearchEngine>(searchEngineList[0])
function getThemeSearchEngineId(engine: SearchEngine) {
  const builtInIndex = searchEngineList.findIndex(candidate => candidate.url === engine.url)
  return builtInIndex >= 0 ? `built-in-${builtInIndex}` : 'active-custom'
}

const themeSearchEngineConfiguration = computed(() => {
  const engines = [...searchEngineList]
  if (!engines.some(engine => getThemeSearchEngineId(engine) === getThemeSearchEngineId(commandCenterSearchEngine.value)))
    engines.push(commandCenterSearchEngine.value)
  return {
    engines: engines.map(engine => ({
      id: getThemeSearchEngineId(engine),
      title: engine.title,
      iconSrc: engine.iconSrc,
      url: engine.url,
    })),
    currentSearchEngine: {
      id: getThemeSearchEngineId(commandCenterSearchEngine.value),
      title: commandCenterSearchEngine.value.title,
      iconSrc: commandCenterSearchEngine.value.iconSrc,
      url: commandCenterSearchEngine.value.url,
    },
  }
})
const groupCreateVisible = ref(false)
const groupName = ref('')
const creatingGroup = ref(false)

const items = ref<ItemGroup[]>([])
const filterItems = ref<ItemGroup[]>([])
const collectionSpaceId = ref<number | null>(null)
const loadedGroups = new Set<number>()
let homeSearchGeneration = 0
const HOME_REQUEST_TIMEOUT = 3000
const homeCollectionLoader = createHomeCollectionLoader<ItemGroup, Panel.ItemInfo>({
  getGroups: (spaceId, signal) => getGroups<ItemGroup[]>(spaceId, signal),
  getItems: (spaceId, groupId, page, pageSize, signal) => getItems<Panel.ItemInfo[]>(spaceId, groupId, page, pageSize, signal),
  readCache: getCachedSpace,
  writeCache: saveCachedSpace,
  isOnline: () => isOnline.value,
  timeoutMs: HOME_REQUEST_TIMEOUT,
  onProgress: applyCollectionProgress,
})
const homeBootstrap = createHomeBootstrap({
  getMonitor: signal => getEnableStatus<{ enabled: boolean; refresh_interval?: number }>(signal),
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
  isOnline: () => isOnline.value,
})
const homeCommandSearch = createHomeCommandSearch<Panel.ItemInfo>({
  getSpaceId: () => activeSpace.value?.id,
  search: (spaceId, query) => homeSearch.query(spaceId, query),
})

const collapsedGroups = ref<Set<number>>(new Set())
const themeRuntimeGrant = ref<{ revision: string; executionMode: 'sandbox' | 'trusted'; available: boolean; granted: boolean; permissions: ThemePermission[] }>({ revision: '', executionMode: 'sandbox', available: false, granted: false, permissions: [] })
const trustedRuntimeAvailable = ref(false)
const themeRuntimeGrantLoading = ref(false)
const themeRuntimeGrantSaving = ref(false)
const themeRuntimeConsentVisible = ref(false)
const selectedThemeExecutionMode = ref<'sandbox' | 'trusted'>('sandbox')
const trustedRuntimeAcknowledged = ref(false)
const themeRuntimeFailed = ref(false)
const themeRuntimeFailureMessage = ref('')
const themeSafeMode = ref(isThemeSafeMode())
let themeGrantRequestGeneration = 0
let trustedPolicyPollTimer: ReturnType<typeof setInterval> | undefined
let trustedThemeRestoreTimer: ReturnType<typeof setTimeout> | undefined
const themeRuntimePackage = computed(() => activeThemePackage.value)
const trustedRouteRevision = computed(() => route.name === 'trustedThemeHome' ? String(route.params.revision || '') : '')
const themeExecutionMode = computed<'sandbox' | 'trusted'>(() => trustedRouteRevision.value ? 'trusted' : 'sandbox')
const themeHomeContribution = computed(() => {
  const manifest = themeRuntimePackage.value?.manifest
  return !!manifest?.entrypoints?.script && !!manifest.contributes?.views?.includes('home') && !!manifest.runtime?.supportedModes?.includes(themeExecutionMode.value)
})
const themeRequiredPermissions = computed(() => (themeRuntimePackage.value?.manifest.permissions?.required || []).map(permission => permission.name as ThemePermission))
const themeGrantMatches = computed(() => {
  const revision = themeRuntimePackage.value?.revision || ''
  return !!revision && themeRuntimeGrant.value.revision === revision && themeRuntimeGrant.value.executionMode === themeExecutionMode.value && themeRuntimeGrant.value.granted && themeRequiredPermissions.value.every(permission => themeRuntimeGrant.value.permissions.includes(permission))
})
const themeRuntimeActive = computed(() => homeReady.value && !themeSafeMode.value && themeHomeContribution.value && themeGrantMatches.value && !themeRuntimeFailed.value && !publicCode)
const themeRuntimeNeedsConsent = computed(() => homeReady.value && !themeSafeMode.value && themeHomeContribution.value && !!authStore.token && !publicCode && !themeRuntimeGrantLoading.value && !themeGrantMatches.value)
watch([themeRuntimeActive, monitorEnabled, () => panelState.panelConfig.systemMonitorShow, themeMonitorLayerRef], async ([runtimeActive, enabled, visible]) => {
  themeMonitorResizeObserver?.disconnect()
  themeMonitorResizeObserver = undefined
  await nextTick()
  if (!runtimeActive || !enabled || !visible) {
    measureThemeMonitorReservation()
    return
  }
  const element = themeMonitorLayerRef.value
  if (!element) {
    measureThemeMonitorReservation()
    return
  }
  if (typeof ResizeObserver !== 'undefined') {
    themeMonitorResizeObserver = new ResizeObserver(measureThemeMonitorReservation)
    themeMonitorResizeObserver.observe(element)
  }
  measureThemeMonitorReservation()
}, { flush: 'post', immediate: true })
const iconifyResourceResolver = createIconifyResourceResolver()
const resolvedThemeIconResources = ref<Record<string, string>>({})
watch([themeRuntimeActive, () => [...new Set(items.value.flatMap(group => (group.items || [])
  .filter(item => item.icon?.itemType === 3 && typeof item.icon.text === 'string')
  .map(item => item.icon?.text || '')))].sort().join('|')], ([runtimeActive, iconIdentifiers]) => {
  const generation = iconifyResourceResolver.beginGeneration()
  if (!runtimeActive) {
    resolvedThemeIconResources.value = {}
    return
  }
  const identifiers = iconIdentifiers ? iconIdentifiers.split('|') : []
  void Promise.all(identifiers.map(async identifier => {
    const resource = await iconifyResourceResolver.resolve(identifier, generation)
    return resource ? [identifier, resource] as const : undefined
  })).then((resolved) => {
    if (!iconifyResourceResolver.isCurrentGeneration(generation)) return
    resolvedThemeIconResources.value = Object.fromEntries(resolved.filter((entry): entry is readonly [string, string] => !!entry))
  })
}, { immediate: true })
onUnmounted(() => iconifyResourceResolver.cancel())
const themeRuntimeSnapshot = computed(() => createThemeHomeSnapshot({
  version: themeSnapshotVersion.value,
  status: homeCollectionStatus.value,
  error: homeCollectionError.value,
  spaces: spaces.value.map(space => ({ ...space, name: spaceDisplayName(space, spaces.value, authStore.userInfo?.id) })),
  activeSpaceId: activeSpace.value?.id,
  activeSpaceSide: activeSpace.value?.side,
  activeSpacePairedId: activeSpace.value?.pairedSpaceId,
  activeSpaceCanEdit: activeSpace.value?.canEdit === true,
  activeSpaceCapabilities: activeSpace.value?.pairedSpaceId ? ['space.toggleSide'] : [],
  groups: items.value.filter(group => Number.isSafeInteger(Number(group.id))).map(group => ({
    ...group,
    id: Number(group.id),
    items: (group.items || []).filter(item => Number.isSafeInteger(Number(item.id))).map(item => ({
      ...item,
      id: Number(item.id),
      icon: item.icon ? { ...item.icon, resolvedSrc: item.icon.text ? resolvedThemeIconResources.value[item.icon.text] : undefined } : item.icon,
    })),
  })),
  canWrite: canWrite.value && themeRuntimePermissions.value.includes('items.write'),
  canWriteGroups: themeCanWriteGroups.value,
  presentation: panelState.panelConfig,
  monitorReservedHeight: themeMonitorReservedHeight.value,
  searchConfiguration: themeSearchEngineConfiguration.value,
}))
const themeRuntimeEnvironment = computed(() => ({
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
}))
const themeRuntimePermissions = computed(() => themeGrantMatches.value ? themeRuntimeGrant.value.permissions : [])
const themeCanWriteGroups = computed(() => canWrite.value && activeSpace.value?.canEdit === true && themeRuntimePackage.value?.manifest.id === 'org.yin.default' && themeRuntimePermissions.value.includes('groups.write'))
const themeRuntimeSlots = computed(() => activeThemeSlots.value)

watch([() => themeRuntimePackage.value?.revision, () => authStore.token, trustedRouteRevision], async ([revision, token, trustedRoute]) => {
  if (trustedThemeRestoreTimer) clearTimeout(trustedThemeRestoreTimer)
  trustedThemeRestoreTimer = undefined
  const requestGeneration = ++themeGrantRequestGeneration
  themeRuntimeFailed.value = false
  themeRuntimeFailureMessage.value = ''
  const executionMode = trustedRoute ? 'trusted' : 'sandbox'
  themeRuntimeGrant.value = { revision: revision || '', executionMode, available: false, granted: false, permissions: [] }
  trustedRuntimeAvailable.value = false
  themeRuntimeGrantLoading.value = false
  if (trustedRoute && (!token || publicCode || previewTheme)) {
    window.location.replace('/')
    return
  }
  if (trustedRoute && (!revision || trustedRoute !== revision || !themeHomeContribution.value)) {
    trustedThemeRestoreTimer = setTimeout(() => {
      if (trustedRouteRevision.value && (themeRuntimePackage.value?.revision !== trustedRouteRevision.value || !themeHomeContribution.value))
        window.location.replace('/')
    }, 10000)
    return
  }
  if (!revision || !token || !themeHomeContribution.value || publicCode || previewTheme) {
    if (trustedRoute) window.location.replace('/')
    return
  }
  themeRuntimeGrantLoading.value = true
  try {
    const result = await getThemeRuntimeGrant(revision, executionMode)
    if (requestGeneration === themeGrantRequestGeneration && result.code === 0 && result.data.revision === revision) {
      themeRuntimeGrant.value = { revision, executionMode, available: result.data.available, granted: result.data.granted, permissions: result.data.permissions as ThemePermission[] }
      if (executionMode === 'sandbox') {
        try {
          const trustedResult = await getThemeRuntimeGrant(revision, 'trusted')
          if (requestGeneration === themeGrantRequestGeneration && trustedResult.code === 0)
            trustedRuntimeAvailable.value = trustedResult.data.available
        }
        catch { trustedRuntimeAvailable.value = false }
      }
      else if (!result.data.granted || !result.data.available) {
        window.location.replace('/')
      }
    }
  }
  catch {
    if (requestGeneration === themeGrantRequestGeneration) {
      themeRuntimeGrant.value = { revision, executionMode, available: false, granted: false, permissions: [] }
      if (trustedRoute) window.location.replace('/')
    }
  }
  finally {
    if (requestGeneration === themeGrantRequestGeneration)
      themeRuntimeGrantLoading.value = false
  }
}, { immediate: true })
watch([trustedRouteRevision, () => authStore.token], ([revision, token]) => {
  if (trustedPolicyPollTimer) clearInterval(trustedPolicyPollTimer)
  trustedPolicyPollTimer = undefined
  if (!revision || !token) return
  trustedPolicyPollTimer = setInterval(async () => {
    try {
      const result = await getThemeRuntimeGrant(revision, 'trusted')
      if (result.code !== 0 || result.data.revision !== revision || !result.data.available || !result.data.granted)
        window.location.replace('/')
    }
    catch {
      window.location.replace('/')
    }
  }, 5000)
}, { immediate: true })
watch(directoryLayout, (enabled) => {
  document.documentElement.dataset.yinLayout = enabled ? 'directory' : 'standard'
  if (enabled)
    collapsedGroups.value = new Set()
}, { immediate: true })
const publicAccessCode = ref('')
const publicAccessReady = ref(!publicCode || !!sessionStorage.getItem(`yin-panel-public-access:${publicCode}`))

const sessionOnlyCache = !!publicCode
function getCachedSpace(spaceId: number) { return readSpaceCache(spaceId, authStore.userInfo?.id, sessionOnlyCache) }
function saveCachedSpace(spaceId: number, cache: any) { writeSpaceCache(spaceId, cache, authStore.userInfo?.id, sessionOnlyCache) }
function clearCachedSpace(spaceId: number) { clearSpaceCache(spaceId, authStore.userInfo?.id, sessionOnlyCache) }
const offlineUnavailable = computed(() => homeReady.value && !isOnline.value && !hasValidCachedHome.value)
const canWrite = computed(() => isOnline.value && !offlineUnavailable.value)

function unlockPublicAccess() {
  if (publicAccessCode.value.length < 4 || publicAccessCode.value.length > 12) return
  sessionStorage.setItem(`yin-panel-public-access:${publicCode}`, publicAccessCode.value)
  publicAccessReady.value = true
  loadHomeData()
}

function openPage(openMethod: number, url: string, title?: string) {
  switch (openMethod) {
    case 1:
      window.location.href = url
      break
    case 2:
      window.open(url)
      break
    case 3:
      windowShow.value = true
      windowSrc.value = url
      windowTitle.value = title || url
      windowIframeIsLoad.value = true
      break

    default:
      break
  }
}

function getItemOpenUrl(item: Panel.ItemInfo, forceWan = false): string {
  const userAgent = typeof navigator === 'undefined' ? '' : navigator.userAgent
  const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini|Mobile|Tablet/i.test(userAgent)
  const networkMode = panelState.networkMode === PanelStateNetworkModeEnum.lan ? 'lan' : 'wan'
  return resolveItemOpenUrl(item, { networkMode, isMobile, forceWan })
}

function handleItemClick(itemGroupIndex: number, item: Panel.ItemInfo) {
  if (items.value[itemGroupIndex] && items.value[itemGroupIndex].sortStatus) {
    handleEditItem(item)
    return
  }

  const jumpUrl = getItemOpenUrl(item)

  openPage(item.openMethod, jumpUrl, item.title)
}

function handWindowIframeIdLoad(payload: Event) {
  windowIframeIsLoad.value = false
}

function scrollToTop() {
  if (themeRuntimeActive.value)
    themeHostRef.value?.scrollToTop()
  else
    scrollContainerRef.value?.scrollTo({ top: 0, behavior: 'smooth' })
}

function handleFloatingButtonMouseDown(event: MouseEvent) {
  if (event.button === 0) event.preventDefault()
}

async function handleFloatingButtonClick(event: MouseEvent, action: () => unknown | Promise<unknown>) {
  const button = event.currentTarget as HTMLElement | null
  try {
    await action()
  } finally {
    button?.blur()
  }
}

async function getList(forceRefresh = false) {
  if (forceRefresh && !canWrite.value) return
  const transitionGeneration = homeTransitionGeneration
  themeSnapshotVersion.value++
  homeCollectionStatus.value = 'loading'
  homeCollectionError.value = undefined
  loadedGroups.clear()
  if (!activeSpace.value) {
    items.value = []
    filterItems.value = []
    collectionSpaceId.value = null
    homeCollectionStatus.value = 'empty'
    return
  }

  const targetSpaceId = activeSpace.value.id
  if (collectionSpaceId.value !== targetSpaceId) {
    items.value = []
    filterItems.value = []
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

function applyCollectionProgress(result: HomeCollectionResult<ItemGroup, Panel.ItemInfo>) {
  if (activeSpace.value?.id !== result.spaceId) return
  applyGroups(result.groups, result.itemsByGroup, result.spaceId)
}

function applyGroups(data: ItemGroup[], itemsByGroup = new Map<number, Panel.ItemInfo[]>(), spaceId = activeSpace.value?.id) {
  collectionSpaceId.value = spaceId ?? null
  const { groups: flattened } = buildHomeGroupTree(data, itemsByGroup)
  items.value = flattened
  themeSnapshotVersion.value++
  collapsedGroups.value = getInitiallyCollapsedGroups(flattened, !directoryLayout.value)
  flattened.forEach(group => loadedGroups.add(Number(group.id)))
  filterItems.value = items.value
  activeDirectoryId.value = resolveActiveDirectoryId(flattened, activeDirectoryId.value)
}

function groupHidden(index: number) {
  return isHomeGroupHidden(items.value, index, collapsedGroups.value)
}

function toggleGroup(id: number) { const next = new Set(collapsedGroups.value); next.has(id) ? next.delete(id) : next.add(id); collapsedGroups.value = next }
function selectSpace(key: string | number) {
  homeTransitionGeneration++
  homeCollectionLoader.cancel()
  homeSearch.invalidate()
  homeCommandSearch.invalidate()
  homeSearchGeneration++
  remoteCommandItems.value = []
  loadedGroups.clear()
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
  homeSearchGeneration++
  remoteCommandItems.value = []
  loadedGroups.clear()
  activeSpace.value = target
  getList()
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
  sideSwitching.value = true
  if (sideSwitchTimer) clearTimeout(sideSwitchTimer)
  sideSwitchTimer = setTimeout(() => {
    sideSwitching.value = false
    sideSwitchTimer = undefined
  }, 1000)
}

const commandDefinitions = computed(() => [
  { key: 'add', label: t('iconItem.add') },
  { key: 'group', label: t('spaceManage.addGroup') },
  { key: 'space', label: t('spaceManage.createSpace') },
  { key: 'settings', label: t('appLauncher.title') },
  { key: 'top', label: t('spaceManage.backToTop') },
  { key: 'open', label: t('iconItem.currentPageOpen') },
  { key: 'lan', label: t('panelHome.openLanUrl') },
  { key: 'wan', label: t('panelHome.openWanUrl') },
  { key: 'edit', label: t('iconItem.edit') },
  { key: 'copy', label: t('common.copyUrl') },
])

const filteredCommandDefinitions = computed(() => {
  return filterHomeCommands(commandDefinitions.value, commandCenterQuery.value)
})

const allCommandItems = computed(() => items.value.flatMap(group => group.items || []))

const commandCenterItems = computed(() => filterHomeCommandItems(commandCenterQuery.value, remoteCommandItems.value, allCommandItems.value))

function openCommandCenter(query: string) {
  updateCommandCenterQuery(query)
  commandCenterSelectedIndex.value = getInitialHomeCommandSelection(query)
  commandCenterVisible.value = true
}

function closeCommandCenter() {
  homeCommandSearch.invalidate()
  commandCenterVisible.value = false
  commandCenterQuery.value = ''
  remoteCommandItems.value = []
  commandCenterSelectedIndex.value = 0
}

function moveCommandSelection(offset: number) {
  const length = commandCenterQuery.value.startsWith('/') ? filteredCommandDefinitions.value.length : commandCenterItems.value.length
  commandCenterSelectedIndex.value = moveHomeCommandSelection(commandCenterSelectedIndex.value, offset, length)
}

function selectCommandItem(index: number) {
  commandCenterSelectedIndex.value = index
}

function submitCommandCenterSearch(keyword: string) {
  window.open(replaceOrAppendKeywordToUrl(commandCenterSearchEngine.value.url, keyword))
  closeCommandCenter()
}

function executeCommand(command: string) {
  if (!canWrite.value && isHomeCommandWrite(command)) return
  const { argument: keyword } = parseHomeCommand(commandCenterQuery.value)
  if (command === 'add') handleAddItem()
  else if (command === 'group') groupCreateVisible.value = true
  else if (command === 'space') createSpaceVisible.value = true
  else if (command === 'settings') settingModalShow.value = true
  else if (command === 'top') scrollToTop()
  else {
    const item = findHomeCommandItem(keyword, remoteCommandItems.value, allCommandItems.value)
    if (!item) return
    if (command === 'open') openPage(item.openMethod, getItemOpenUrl(item), item.title)
    else if (command === 'lan' && item.lanUrl) openPage(item.openMethod, item.lanUrl, item.title)
    else if (command === 'wan') openPage(item.openMethod, getItemOpenUrl(item, true), item.title)
    else if (command === 'edit') handleEditItem({ ...item })
    else if (command === 'copy') navigator.clipboard?.writeText(item.url)
  }
  closeCommandCenter()
}

function executeCommandItem(item: Panel.ItemInfo) {
  openPage(item.openMethod, getItemOpenUrl(item), item.title)
  closeCommandCenter()
}

function updateCommandCenterQuery(query: string) {
  const wasCommandQuery = commandCenterQuery.value.startsWith('/')
  const isCommandQuery = query.startsWith('/')
  commandCenterQuery.value = query
  if (!query.trim() || query.startsWith('/')) {
    homeCommandSearch.invalidate()
    remoteCommandItems.value = []
  }
  else {
    void homeCommandSearch.search(query).then((matches) => {
      if (matches !== undefined)
        remoteCommandItems.value = matches
    })
  }
  if (wasCommandQuery !== isCommandQuery)
    commandCenterSelectedIndex.value = isCommandQuery ? 0 : -1
}

function isEditableTarget(target: EventTarget | null) {
  const element = target as HTMLElement | null
  return !!element?.closest('input, textarea, select, [contenteditable="true"]')
}

function hasBlockingLayer() {
  if (settingModalShow.value || editItemInfoShow.value || createSpaceVisible.value || groupCreateVisible.value || windowShow.value || dropdownShow.value) return true
  return Array.from(document.querySelectorAll('.n-modal-container, .n-drawer-container')).some((element) => {
    const style = window.getComputedStyle(element)
    if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false
    return Array.from(element.querySelectorAll('.n-modal-body-wrapper, .n-drawer-body-content')).some(container => container.firstElementChild !== null)
  })
}

function handleGlobalKeydown(event: KeyboardEvent) {
  if (commandCenterVisible.value || !authStore.token || (publicCode && !publicAccessReady.value)) return
  if (event.isComposing || isEditableTarget(event.target) || hasBlockingLayer()) return
  if (event.ctrlKey || event.metaKey || event.altKey || event.key.length !== 1) return
  event.preventDefault()
  openCommandCenter(event.key)
}

function submitCreateGroup() {
  if (!canWrite.value) return
  const title = groupName.value.trim()
  if (!title || !activeSpace.value || creatingGroup.value) return
  creatingGroup.value = true
  void homeMutations.createGroup({ title, icon: '', parentId: null }).then(() => {
    groupName.value = ''
    groupCreateVisible.value = false
  }).catch((error: unknown) => {
    ms.error(`${t('common.saveFail')}:${error instanceof Error ? error.message : ''}`)
  }).finally(() => { creatingGroup.value = false })
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

function reloadSpaces(selectLatest = false) {
  void homeSpaceController.refresh(selectLatest)
}
function handleSpacesChanged() {
  reloadSpaces()
}
function submitCreateSpace() {
  if (!canWrite.value) return
  const name = spaceName.value.trim()
  if (!name || creatingSpace.value) return
  creatingSpace.value = true
  createSpace<{ code: number }>(name).then(({ code }) => {
    if (code === 0) { createSpaceVisible.value = false; spaceName.value = ''; reloadSpaces(true) }
  }).finally(() => { creatingSpace.value = false })
}

// 从后端获取组下面的图标
function updateItemIconGroupByNet(itemIconGroupIndex: number, itemIconGroupId: number) {
  getItems<Panel.ItemInfo[]>(activeSpace.value!.id, itemIconGroupId).then(({ data }) => {
    items.value[itemIconGroupIndex].items = data
  })
}

function handleRightMenuSelect(key: string | number) {
  dropdownShow.value = false
  if (!canWrite.value && ['edit', 'delete'].includes(String(key))) return
  // console.log(currentRightSelectItem, key)
  const jumpUrl = currentRightSelectItem.value ? getItemOpenUrl(currentRightSelectItem.value) : ''
  switch (key) {
    case 'newWindows':
      window.open(jumpUrl)
      break
    case 'openWanUrl':
      if (currentRightSelectItem.value)
        openPage(currentRightSelectItem.value.openMethod, getItemOpenUrl(currentRightSelectItem.value, true), currentRightSelectItem.value.title)
      break
    case 'openLanUrl':
      if (currentRightSelectItem.value && currentRightSelectItem.value.lanUrl)
        openPage(currentRightSelectItem.value?.openMethod, currentRightSelectItem.value.lanUrl, currentRightSelectItem.value?.title)
      break
    case 'edit':
      // 这里有个奇怪的问题，如果不使用{...}的方式 父组件的值会同步修改 标记一下
      handleEditItem({ ...currentRightSelectItem.value } as Panel.ItemInfo)
      break
    case 'delete': {
      if (!currentRightSelectItem.value || !activeSpace.value) break
      const selectedItem = currentRightSelectItem.value
      const selectedSpaceId = activeSpace.value.id
      dialog.warning({
        title: t('common.warning'),
        content: t('common.deleteConfirmByName', { name: selectedItem.title }),
        positiveText: t('common.confirm'),
        negativeText: t('common.cancel'),
        onPositiveClick: async () => {
          try {
            await homeMutations.deleteItem(Number(selectedItem.id), selectedSpaceId)
            ms.success(t('common.deleteSuccess'))
          }
          catch (error) {
            ms.error(`${t('common.deleteFail')}:${error instanceof Error ? error.message : ''}`)
          }
        },
      })
      break
    }
    default:
      break
  }
}

function handleContextMenu(e: MouseEvent, itemGroupIndex: number, item: Panel.ItemInfo) {
  if (items.value[itemGroupIndex] && items.value[itemGroupIndex].sortStatus)
    return

  e.preventDefault()
  currentRightSelectItem.value = item
  dropdownShow.value = false
  nextTick().then(() => {
    dropdownShow.value = true
    dropdownMenuX.value = e.clientX
    dropdownMenuY.value = e.clientY
  })
}

function onClickoutside() {
  // message.info('clickoutside')
  dropdownShow.value = false
}

function handleChangeNetwork(mode: PanelStateNetworkModeEnum) {
  panelState.setNetworkMode(mode)
  if (mode === PanelStateNetworkModeEnum.lan)
    ms.success(t('panelHome.changeToLanModelSuccess'))

  else
    ms.success(t('panelHome.changeToWanModelSuccess'))
}

// 结束拖拽
// function handleEndDrag(event: any, itemIconGroup: Panel.ItemIconGroup) {
//   // console.log(event)
//   // console.log(items.value)
// }

async function handleSaveSort(itemGroup: ItemGroup) {
  if (!canWrite.value || !itemGroup.items) return
  const groupId = Number(itemGroup.id)
  if (!Number.isSafeInteger(groupId) || groupId <= 0) return
  const itemIds = itemGroup.items.map(item => Number(item.id))
  if (itemIds.some(id => !Number.isSafeInteger(id) || id <= 0)) return
  try {
    await homeMutations.reorderItems(groupId, itemIds)
    itemGroup.sortStatus = false
    ms.success(t('common.saveSuccess'))
  }
  catch (error) {
    ms.error(`${t('common.saveFail')}:${error instanceof Error ? error.message : ''}`)
  }
}

function getDropdownMenuOptions() {
  const dropdownMenuOptions = [
    {
      label: t('iconItem.newWindowOpen'),
      key: 'newWindows',
    },

  ]

  if (currentRightSelectItem.value?.lanUrl && panelState.networkMode === PanelStateNetworkModeEnum.wan) {
    dropdownMenuOptions.push({
      label: t('panelHome.openLanUrl'),
      key: 'openLanUrl',
    })
  }

  if (currentRightSelectItem.value?.lanUrl && panelState.networkMode === PanelStateNetworkModeEnum.lan) {
    dropdownMenuOptions.push({
      label: t('panelHome.openWanUrl'),
      key: 'openWanUrl',
    })
  }

  if (canWrite.value) {
    dropdownMenuOptions.push({ label: t('common.edit'), key: 'edit' }, { label: t('common.delete'), key: 'delete' })
  }
  return dropdownMenuOptions
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

  if (!isOnline.value) return
  const result = await homeBootstrap.load()
  if (result.cancelled) return
  if (result.monitor?.code === 0) {
    monitorEnabled.value = result.monitor.data.enabled
    monitorResultRefreshInterval.value = result.monitor.data.refresh_interval
  }
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
    isOnline.value = false
  }
}

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown)
  window.addEventListener('online', handleOnline)
  window.addEventListener('offline', handleOffline)
  window.addEventListener('resize', syncThemeEnvironment)
  window.addEventListener('resize', measureThemeMonitorReservation)
  window.visualViewport?.addEventListener('resize', measureThemeMonitorReservation)
  colorSchemeMedia.addEventListener('change', syncThemeEnvironment)
  reducedMotionMedia.addEventListener('change', syncThemeEnvironment)
  runtimeEnvironmentObserver = new MutationObserver(syncThemeEnvironment)
  runtimeEnvironmentObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'lang'] })
  syncThemeEnvironment()
  void updatePwaReady()
  navigator.serviceWorker?.addEventListener('controllerchange', updatePwaReady)
  if (publicCode && !publicAccessReady.value) return
  loadHomeData()
})

onUnmounted(() => {
  if (trustedPolicyPollTimer) clearInterval(trustedPolicyPollTimer)
  if (trustedThemeRestoreTimer) clearTimeout(trustedThemeRestoreTimer)
  delete document.documentElement.dataset.yinLayout
  window.removeEventListener('keydown', handleGlobalKeydown)
  window.removeEventListener('online', handleOnline)
  window.removeEventListener('offline', handleOffline)
  window.removeEventListener('resize', syncThemeEnvironment)
  window.removeEventListener('resize', measureThemeMonitorReservation)
  window.visualViewport?.removeEventListener('resize', measureThemeMonitorReservation)
  themeMonitorResizeObserver?.disconnect()
  if (themeMonitorMeasureFrame) cancelAnimationFrame(themeMonitorMeasureFrame)
  colorSchemeMedia.removeEventListener('change', syncThemeEnvironment)
  reducedMotionMedia.removeEventListener('change', syncThemeEnvironment)
  runtimeEnvironmentObserver?.disconnect()
  navigator.serviceWorker?.removeEventListener('controllerchange', updatePwaReady)
  if (sideSwitchTimer) clearTimeout(sideSwitchTimer)
  homeCollectionLoader.cancel()
  homeBootstrap.cancel()
  homeSpaceController.cancel()
  homeSearch.cancel()
})

function syncThemeEnvironment() {
  runtimeViewport.value = { width: window.innerWidth, height: window.innerHeight }
  runtimeLanguage.value = document.documentElement.lang || navigator.language
  runtimeColorScheme.value = document.documentElement.classList.contains('dark') ? 'dark' : 'light'
  runtimeReducedMotion.value = reducedMotionMedia.matches
  // The theme monitor layer is positioned with viewport units, so its reserved
  // height must be re-measured whenever the viewport changes.
  measureThemeMonitorReservation()
}

function handleOnline() {
  isOnline.value = true
  void loadHomeData(true)
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

// 前端搜索过滤
function itemFrontEndSearch(keyword?: string) {
  keyword = keyword?.trim() || ''
  if (keyword !== '' && panelState.panelConfig.searchBoxSearchIcon) {
    const filteredData: ItemGroup[] = []
    for (let i = 0; i < items.value.length; i++) {
      const element = items.value[i].items?.filter((item: Panel.ItemInfo) => {
        return (
          item.title.toLowerCase().includes(keyword?.toLowerCase() ?? '')
          || item.url.toLowerCase().includes(keyword?.toLowerCase() ?? '')
          || item.description?.toLowerCase().includes(keyword?.toLowerCase() ?? '')
        )
      })
      if (element && element.length > 0)
        filteredData.push({ ...items.value[i], items: element })
    }
    filterItems.value = filteredData
    const query = keyword
    const spaceId = activeSpace.value?.id
    if (!spaceId) return
    const searchGeneration = ++homeSearchGeneration
    void homeSearch.query(spaceId, query).then((matches) => {
      if (searchGeneration !== homeSearchGeneration || activeSpace.value?.id !== spaceId) return
      const groupMap = new Map(items.value.map(group => [Number(group.id), group]))
      const grouped = new Map<number, Panel.ItemInfo[]>()
      for (const item of matches) {
        const groupId = Number(item.itemIconGroupId)
        if (!Number.isSafeInteger(groupId)) continue
        const groupItems = grouped.get(groupId) || []
        groupItems.push(item)
        grouped.set(groupId, groupItems)
      }
      filterItems.value = [...grouped.entries()].flatMap(([groupId, groupItems]) => {
        const group = groupMap.get(groupId)
        return group ? [{ ...group, items: groupItems }] : []
      })
    }).catch(() => {})
  }
  else {
    homeSearchGeneration++
    filterItems.value = items.value
  }
}

function handleSetHoverStatus(groupIndex: number, hoverStatus: boolean) {
  if (items.value[groupIndex])
    items.value[groupIndex].hoverStatus = hoverStatus
}

function handleSetSortStatus(groupIndex: number, sortStatus: boolean) {
  if (!canWrite.value) return
  if (items.value[groupIndex])
    items.value[groupIndex].sortStatus = sortStatus

  // 并未保存排序重新更新数据
  if (!sortStatus) {
    // 单独更新组
    if (items.value[groupIndex] && items.value[groupIndex].id)
      updateItemIconGroupByNet(groupIndex, items.value[groupIndex].id as number)
  }
}

function handleEditItem(item: Panel.ItemInfo) {
  if (!canWrite.value) return
  editItemInfoData.value = item
  editItemInfoShow.value = true
  currentAddItenIconGroupId.value = undefined
}

function handleAddItem(itemIconGroupId?: number) {
  if (!canWrite.value) return
  editItemInfoData.value = null
  editItemInfoShow.value = true
  if (itemIconGroupId)
    currentAddItenIconGroupId.value = itemIconGroupId
}

function createThemeRuntimeError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}

const themePersistence = createThemePersistence(localStorage, {
  userId: () => authStore.userInfo?.id,
  packageId: () => activeThemePackage.value?.manifest.id,
  revision: () => activeThemePackage.value?.revision,
  validateSettings: settings => validateActiveThemeSettings(settings),
})
const validateThemeSettingsSchema = createThemeSettingsSchemaValidator({ origin: window.location.origin })
function validateActiveThemeSettings(settings: Record<string, unknown>) {
  return validateThemeSettingsSchema(activeThemePackage.value || undefined, settings)
}

const homeMutations = createHomeMutationService({
  api: {
    createItem: (spaceId, input) => createItem<Panel.ItemInfo>(spaceId, input),
    createItemWithIcon: (spaceId, input, file) => createItemWithIcon<Panel.ItemInfo>(spaceId, input, file),
    uploadItemIcon: file => uploadImage(file),
    updateItem: (spaceId, itemId, input) => updateItem<Panel.ItemInfo>(spaceId, itemId, input),
    deleteItem: (spaceId, itemId) => deleteSpaceItem<{ code: number; msg?: string }>(spaceId, itemId),
    reorderItems: (spaceId, groupId, itemIds) => sortItems<{ code: number; msg?: string }>(spaceId, groupId, itemIds.map((id, index) => ({ id, sort: index + 1 }))),
    createGroup: (spaceId, input) => createGroup<{ code: number; msg?: string }>(spaceId, input.title, input.icon, input.parentId),
    updateGroup: (spaceId, groupId, input) => updateGroup<{ code: number; msg?: string }>(spaceId, groupId, input.title, input.icon, input.parentId),
    deleteGroup: (spaceId, groupId) => deleteGroup<{ code: number; msg?: string }>(spaceId, groupId),
    reorderGroups: (spaceId, parentId, groupIds) => sortGroups<{ code: number; msg?: string }>(spaceId, parentId, groupIds.map((id, index) => ({ id, sort: index + 1 }))),
  },
  getActiveSpaceId: () => activeSpace.value?.id,
  canWrite: () => canWrite.value,
  invalidateSpace: clearCachedSpace,
  refreshSpace: spaceId => activeSpace.value?.id === spaceId ? getList(true) : undefined,
})

const homeThemeHandlers = createHomeThemeHandlers({
  getSpaces: () => spaces.value,
  getGroups: () => items.value.filter(group => Number.isSafeInteger(Number(group.id))).map(group => ({
    id: Number(group.id),
    parentId: group.parentId,
    title: group.title,
    icon: group.icon,
    sort: group.sort,
    items: group.items || [],
  })),
  getActiveSpaceId: () => activeSpace.value?.id,
  canWriteGroups: () => themeCanWriteGroups.value,
  selectSpace: spaceId => selectSpace(spaceId),
  openItem: (item) => { openPage(item.openMethod, getItemOpenUrl(item), item.title) },
  openEditor: ({ item, groupId }) => item ? handleEditItem(item) : handleAddItem(groupId),
  createItem: async (input) => { await homeMutations.createItem(input) },
  updateItem: async (itemId, input) => { await homeMutations.updateItem(itemId, input) },
  deleteItem: item => confirmThemeDelete(
    t('common.deleteConfirmByName', { name: item.title }),
    spaceId => homeMutations.deleteItem(Number(item.id), spaceId),
  ),
  reorderItems: (groupId, itemIds) => homeMutations.reorderItems(groupId, itemIds),
  createGroup: input => homeMutations.createGroup({ title: input.title, icon: input.icon || '', parentId: input.parentId ?? null }),
  updateGroup: (groupId, input) => homeMutations.updateGroup(groupId, { title: input.title, icon: input.icon || '', parentId: input.parentId ?? null }),
  deleteGroup: group => confirmThemeDelete(
    t('spaceManage.deleteWarnText', { name: group.title || '' }),
    spaceId => homeMutations.deleteGroup(group.id, spaceId),
  ),
  reorderGroups: (parentId, groupIds) => homeMutations.reorderGroups(parentId, groupIds),
  openCommandCenter: () => { commandCenterVisible.value = true },
  toggleSide: togglePanelSide,
  refresh: () => getList(true),
  searchItems: query => activeSpace.value ? homeSearch.query(activeSpace.value.id, query) : [],
  getMonitorSnapshot: async () => {
    return normalizeMonitorSnapshot(await monitorSnapshotController.fetchSnapshot())
  },
  submitSearch: query => itemFrontEndSearch(query),
  getSearchConfiguration: () => ({
    engines: themeSearchEngineConfiguration.value.engines.map(engine => ({ id: engine.id })),
    currentEngineId: themeSearchEngineConfiguration.value.currentSearchEngine.id,
  }),
  submitSearchWithEngine: (query, engineId) => {
    const engine = themeSearchEngineConfiguration.value.engines.find(candidate => candidate.id === engineId)
    if (!engine) throw createThemeRuntimeError('INVALID_ARGUMENT', 'Search engine is not available in the active Space')
    // Record the selection in Core so the next theme snapshot reports the new
    // currentEngineId; otherwise the theme select keeps the previous icon.
    commandCenterSearchEngine.value = { title: engine.title, iconSrc: engine.iconSrc, url: engine.url }
    window.open(replaceOrAppendKeywordToUrl(engine.url, query))
  },
  navigate: (destination) => {
    if (destination.view !== 'home') throw createThemeRuntimeError('UNSUPPORTED_CAPABILITY', 'Core navigation destination is unavailable')
  },
  getSettings: themePersistence.getSettings,
  patchSettings: themePersistence.patchSettings,
  getStorage: themePersistence.getStorage,
  setStorage: themePersistence.setStorage,
  removeStorage: themePersistence.removeStorage,
  openCoreSurface: async (surface) => {
    if (surface === 'theme-settings') {
      await router.push('/settings/style')
      return
    }
    throw createThemeRuntimeError('UNSUPPORTED_CAPABILITY', 'Core surface is unavailable')
  },
  networkFetch: async (input, init) => {
    const url = new URL(input, window.location.origin)
    if (url.origin !== window.location.origin || !url.pathname.startsWith('/api/'))
      throw createThemeRuntimeError('PERMISSION_DENIED', 'Theme network requests must target the Core origin')
    const method = typeof init.method === 'string' ? init.method.toUpperCase() : 'GET'
    if (!['GET', 'HEAD'].includes(method)) throw createThemeRuntimeError('PERMISSION_DENIED', 'Theme network broker allows read-only requests')
    const response = await fetch(url, { method, credentials: 'omit', redirect: 'error' })
    const body = await response.text()
    return { status: response.status, headers: Object.fromEntries(response.headers.entries()), body }
  },
  reportDiagnostic: async entry => { console[entry.level === 'error' ? 'error' : entry.level === 'warn' ? 'warn' : 'info']('[theme]', entry.message, entry.data || {}) },
})

function confirmThemeDelete(message: string, action: (spaceId: number) => Promise<unknown>): Promise<unknown> {
  if (!canWrite.value || !activeSpace.value) throw createThemeRuntimeError('PERMISSION_DENIED', 'The active Space is read-only')
  const confirmedSpaceId = activeSpace.value.id
  return new Promise((resolve, reject) => {
    let settled = false
    dialog.warning({
      title: t('common.warning'),
      content: message,
      positiveText: t('common.confirm'),
      negativeText: t('common.cancel'),
      onPositiveClick: async () => {
        settled = true
        try { resolve(await action(confirmedSpaceId)) }
        catch (error) { reject(error) }
      },
      onNegativeClick: () => {
        settled = true
        reject(createThemeRuntimeError('ABORTED', 'Delete was cancelled'))
      },
      onClose: () => {
        if (!settled) reject(createThemeRuntimeError('ABORTED', 'Delete was cancelled'))
      },
    })
  })
}

async function executeThemeRequest(value: unknown) {
  return executeHomeThemeRequest(homeThemeHandlers, value)
}

async function grantThemeRuntimePermissions() {
  const revision = themeRuntimePackage.value?.revision
  if (!revision || themeRuntimeGrantSaving.value) return
  const permissions = themeRequiredPermissions.value
  const executionMode = selectedThemeExecutionMode.value
  if (executionMode === 'trusted' && (!trustedRuntimeAvailable.value || !trustedRuntimeAcknowledged.value)) return
  if (previewTheme) {
    themeRuntimeGrant.value = { revision, executionMode, available: true, granted: true, permissions }
    return
  }
  themeRuntimeGrantSaving.value = true
  try {
    const result = await setThemeRuntimeGrant(revision, permissions, executionMode)
    if (result.code === 0) {
      if (executionMode === 'trusted') {
        window.location.assign(`/__yin/theme-trusted/${encodeURIComponent(revision)}`)
        return
      }
      themeRuntimeGrant.value = { revision, executionMode, available: true, granted: true, permissions }
      themeRuntimeConsentVisible.value = false
      themeRuntimeFailed.value = false
      themeRuntimeFailureMessage.value = ''
    }
  }
  catch {
    themeRuntimeFailureMessage.value = t('themePackage.runtimeGrantFailed')
  }
  finally {
    themeRuntimeGrantSaving.value = false
  }
}

function openThemeRuntimeConsent() {
  selectedThemeExecutionMode.value = 'sandbox'
  trustedRuntimeAcknowledged.value = false
  themeRuntimeConsentVisible.value = true
}

function handleThemeRuntimeFailure(error: Error) {
  console.error('Theme sandbox failed to start:', error)
  themeRuntimeFailed.value = true
  themeRuntimeFailureMessage.value = error.message || t('themePackage.runtimeLoadFailed')
}
</script>

<template>
  <div class="w-full h-full sun-main" :class="{ 'side-switching': sideSwitching, 'theme-defaults': useThemeColors, 'theme-runtime-yin': themeRuntimeActive && themeRuntimePackage?.manifest.id === 'org.yin.default' }" :data-panel-side="activeSpace?.side || 'yin'">
    <CommandCenter
      :visible="commandCenterVisible"
      :query="commandCenterQuery"
      :items="commandCenterItems"
      :commands="filteredCommandDefinitions"
      :selected-index="commandCenterSelectedIndex"
      :search-engine="commandCenterSearchEngine"
      @update:query="updateCommandCenterQuery"
      @move="moveCommandSelection"
      @select="selectCommandItem"
      @submit-search="submitCommandCenterSearch"
      @execute-item="executeCommandItem"
      @execute-command="executeCommand"
      @close="closeCommandCenter"
    />
    <div v-if="sideSwitching" class="taiji-transition" aria-hidden="true">
      <div class="taiji-aura">
        <div class="taiji-bagua">
          <span v-for="index in 8" :key="index" />
        </div>
        <div class="taiji-symbol" :class="activeSpace?.side === 'yang' ? 'taiji-yang' : 'taiji-yin'">
          <span class="taiji-dot taiji-dot-dark" />
          <span class="taiji-dot taiji-dot-light" />
        </div>
      </div>
    </div>
    <NModal :show="!!publicCode && !publicAccessReady" :mask-closable="false" :closable="false">
      <NCard :title="$t('spaceManage.accessVerification')" style="width: min(92vw, 380px)">
        <NSpace vertical>
          <span>{{ $t('spaceManage.enterAccessCode') }}</span>
          <NInput v-model:value="publicAccessCode" type="password" show-password-on="click" maxlength="12" :placeholder="$t('spaceManage.accessCodeShortPlaceholder')" @keyup.enter="unlockPublicAccess" />
          <NButton type="primary" block @click="unlockPublicAccess">{{ $t('spaceManage.confirmAccess') }}</NButton>
        </NSpace>
      </NCard>
    </NModal>
    <div v-if="homeReady && spaces.length && authStore.token" class="space-status-bar">
      <NDropdown
        trigger="hover"
        :options="spaceSelectorOptions"
        :theme-overrides="{
          color: 'rgba(18, 22, 28, 0.72)',
          optionTextColor: 'rgba(255, 255, 255, 0.92)',
          optionTextColorHover: '#fff',
          optionColorHover: 'rgba(255, 255, 255, 0.14)',
          optionColorActive: 'rgba(125, 211, 252, 0.18)',
          dividerColor: 'rgba(255, 255, 255, 0.18)',
          borderRadius: '10px',
        }"
        @select="selectSpace"
      >
        <NButton quaternary class="space-status-button">
          <span class="space-status-dot" />
          {{ activeSpace ? spaceDisplayName(activeSpace, spaces, authStore.userInfo?.id) : '' }}
          <span class="ml-2 opacity-60">⌄</span>
        </NButton>
      </NDropdown>
    </div>
    <div v-if="homeReady" class="offline-status" :class="{ 'offline-status--theme-hidden': themeRuntimeActive && isOnline }" data-testid="offline-status">
      <span v-if="pwaReady" data-testid="pwa-ready">{{ $t('panelHome.pwaReady') }}</span>
      <template v-if="!isOnline && hasValidCachedHome">
        <span data-testid="offline-readonly">{{ $t('panelHome.offlineReadonly') }}</span>
        <span v-if="cacheUpdatedAt" data-testid="offline-cache-updated">{{ $t('panelHome.cacheUpdatedAt', { time: new Date(cacheUpdatedAt).toLocaleString() }) }}</span>
      </template>
    </div>
    <div v-if="themeSafeMode && homeReady" class="theme-safe-mode-banner" role="status" data-testid="theme-safe-mode-banner">
      <span>{{ $t('themeRecovery.active') }}</span>
      <RouterLink to="/__yin/theme-recovery">{{ $t('themeRecovery.exit') }}</RouterLink>
    </div>
    <WallpaperLayer v-if="homeReady" />
    <ThemeHost
      v-if="themeRuntimeActive && themeRuntimePackage"
      :key="`${themeRuntimePackage.revision}:${themeExecutionMode}:${themeRuntimeGrant.permissions.join(',')}`"
      class="theme-home-host"
      ref="themeHostRef"
      :theme="themeRuntimePackage"
      :snapshot="themeRuntimeSnapshot"
      :environment="themeRuntimeEnvironment"
      :permissions="themeRuntimePermissions"
      :execution-mode="themeExecutionMode"
      :slots="themeRuntimeSlots"
      :title="themeRuntimePackage.manifest.name"
      :execute="executeThemeRequest"
      @failed="handleThemeRuntimeFailure"
    >
      <div
        v-if="monitorEnabled && panelState.panelConfig.systemMonitorShow"
        ref="themeMonitorLayerRef"
        class="theme-runtime-monitor-layer"
        :class="{ 'theme-runtime-monitor-layer--info': panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info, 'theme-runtime-monitor-layer--yin': themeRuntimePackage?.manifest.id === 'org.yin.default' }"
        data-testid="theme-runtime-monitor"
        :data-panel-side="activeSpace?.side || 'yin'"
      >
        <SystemMonitor
          :snapshot-controller="monitorSnapshotController"
          :show-title="panelState.panelConfig.systemMonitorShowTitle"
          :icon-text-color="panelIconTextColor"
        />
      </div>
    </ThemeHost>
    <div v-if="offlineUnavailable" class="offline-unavailable" data-testid="offline-unavailable">
      <NCard :title="$t('panelHome.offlineUnavailable')" size="small">
        <NSpace vertical>
          <span>{{ $t('panelHome.offlineUnavailableDetail') }}</span>
          <NButton type="primary" data-testid="offline-retry-button" @click="retryWhenOnline">{{ $t('panelHome.retryWhenOnline') }}</NButton>
        </NSpace>
      </NCard>
    </div>
    <div v-if="homeReady && !themeRuntimeActive" ref="scrollContainerRef" class="home-scroll-container absolute w-full h-full overflow-auto">
      <div
        class="p-2.5 mx-auto"
        :class="{ 'directory-page': directoryLayout }"
        :style="{
          marginTop: `${panelState.panelConfig.marginTop}%`,
          marginBottom: `${panelState.panelConfig.marginBottom}%`,
          maxWidth: `${panelState.panelConfig.maxWidth ?? 1200}${panelState.panelConfig.maxWidthUnit}`,
          background: directoryLayout ? 'transparent' : undefined,
        }"
      >
        <div v-if="themeRuntimeNeedsConsent" class="theme-runtime-notice" role="status" data-testid="theme-runtime-consent">
          <span>{{ themeRuntimeFailureMessage || $t('themePackage.runtimePrompt', { name: themeRuntimePackage?.manifest.name, permissions: themeRequiredPermissions.join(', ') || $t('themePackage.noRuntimePermissions') }) }}</span>
          <NButton size="small" type="primary" @click="openThemeRuntimeConsent">
            {{ $t('themeTrustedRuntime.review') }}
          </NButton>
        </div>
        <div v-if="themeRuntimeFailed" class="theme-runtime-notice theme-runtime-notice--error" role="alert" data-testid="theme-runtime-fallback">
          <span>{{ $t('themePackage.runtimeLoadFailed') }}</span>
          <a href="/theme-recovery.html">{{ $t('themeRecovery.title') }}</a>
        </div>
        <!-- 头 -->
        <div class="home-header mx-[auto] w-[80%]">
          <div class="home-header-row flex mx-[auto] items-center justify-center text-white">
            <div class="logo cursor-pointer" data-lcp="brand" data-testid="core-side-toggle" role="button" tabindex="0" :aria-label="activeSpace?.side === 'yang' ? 'Switch to Yin-Panel' : 'Switch to Yang-Panel'" @click="togglePanelSide" @keydown.enter="togglePanelSide" @keydown.space.prevent="togglePanelSide">
              <span class="text-2xl md:text-6xl font-bold text-shadow">
                {{ activeSpace?.side === 'yang' ? 'Yang-Panel' : 'Yin-Panel' }}
              </span>
            </div>
            <div class="divider text-base lg:text-2xl mx-[10px]">
              |
            </div>
            <div class="text-shadow">
              <Clock :hide-second="!panelState.panelConfig.clockShowSecond" />
            </div>
          </div>
          <div v-if="panelState.panelConfig.searchBoxShow" class="flex mt-[20px] mx-auto sm:w-full lg:w-[80%]" :class="{ 'directory-search': directoryLayout }">
            <SearchBox :space-id="activeSpace?.id" :session-only="sessionOnlyCache" :directory="directoryLayout" @itemSearch="itemFrontEndSearch" @search-engine-change="commandCenterSearchEngine = $event" />
          </div>
        </div>

        <nav v-if="directoryLayout && directoryRoots.length" class="directory-folders" aria-label="Bookmark groups">
          <button v-for="group in directoryRoots" :key="group.id" type="button" :aria-pressed="Number(activeDirectoryId) === Number(group.id)" :class="{ active: Number(activeDirectoryId) === Number(group.id) }" @click="activeDirectoryId = Number(group.id)">
            <SvgIcon :icon="group.icon || 'mdi-folder-outline'" />
            <span>{{ group.title }}</span>
          </button>
        </nav>
        <div v-if="directoryLayout" class="directory-brand-controls">
          <span>{{ activeSpace?.side === 'yang' ? 'Yang-Panel' : 'Yin-Panel' }}</span>
          <Clock class="directory-clock" :hide-second="!panelState.panelConfig.clockShowSecond" />
        </div>

        <!-- 应用盒子 -->
        <div
          class="home-content"
          :class="{
            'home-content--with-monitor': !directoryLayout && monitorEnabled && panelState.panelConfig.systemMonitorShow,
            'home-content--monitor-info': !directoryLayout && monitorEnabled && panelState.panelConfig.systemMonitorShow && panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info,
            'directory-content': directoryLayout,
          }"
          :style="{ marginLeft: `${panelState.panelConfig.marginX}px`, marginRight: `${panelState.panelConfig.marginX}px` }"
        >
          <!-- 系统监控状态 -->
          <div
            v-if="monitorEnabled && panelState.panelConfig.systemMonitorShow"
            class="system-monitor-layer"
            :class="{
              'system-monitor-layer--info': panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info,
              'system-monitor-layer--directory': directoryLayout,
            }"
          >
            <SystemMonitor
              :snapshot-controller="monitorSnapshotController"
              :show-title="panelState.panelConfig.systemMonitorShowTitle"
              :icon-text-color="panelIconTextColor"
            />
          </div>

          <!-- 组纵向排列 -->
          <div
            v-for="(itemGroup, itemGroupIndex) in directoryItems" :key="itemGroupIndex"
            v-show="!groupHidden(itemGroupIndex)"
            data-item-group data-testid="item-group"
            class="item-list min-h-[110px]"
            :class="{ 'item-list--sorting': itemGroup.sortStatus }"
            @mouseenter="handleSetHoverStatus(itemGroupIndex, true)"
            @mouseleave="handleSetHoverStatus(itemGroupIndex, false)"
          >
            <!-- 分组标题 -->
            <div class="directory-group-heading flex items-center" :style="{ marginLeft: `${10 + (itemGroup.depth || 0) * 24}px` }">
              <span class="group-title text-shadow">
                {{ itemGroup.title }}
              </span>
              <span class="ml-2 cursor-pointer" :title="collapsedGroups.has(Number(itemGroup.id)) ? $t('spaceManage.expandGroup') : $t('spaceManage.collapseGroup')" @click="toggleGroup(Number(itemGroup.id))">
                <SvgIcon :icon="collapsedGroups.has(Number(itemGroup.id)) ? 'mdi-chevron-down' : 'mdi-chevron-up'" />
              </span>
              <div
                v-if="parsePublicCodeFromPath() === '' && authStore.token && canWrite"
                class="group-buttons flex"
                :class="itemGroup.hoverStatus ? 'opacity-100' : 'opacity-0'"
              >
                <span class="mr-2 cursor-pointer" :title="t('common.add')" @click="handleAddItem(itemGroup.id)">
                  <SvgIcon class="group-action-icon" icon="typcn:plus" />
                </span>
                <span class="mr-2 cursor-pointer " :title="t('common.sort')" @click="handleSetSortStatus(itemGroupIndex, !itemGroup.sortStatus)">
                  <SvgIcon class="group-action-icon" icon="ri:drag-drop-line" />
                </span>
              </div>
            </div>

            <!-- 详情图标 -->
            <div v-if="!directoryLayout && panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info">
              <div v-if="itemGroup.items && !collapsedGroups.has(Number(itemGroup.id))">
                <VueDraggable
                  v-model="itemGroup.items" item-key="sort" :animation="300"
                  class="icon-info-box"
                  filter=".not-drag"
                  :disabled="!itemGroup.sortStatus"
                >
                  <div v-for="item, index in itemGroup.items" :key="index" :title="item.description" @contextmenu="(e) => handleContextMenu(e, itemGroupIndex, item)">
                    <AppIcon
                      :class="itemGroup.sortStatus ? 'cursor-move' : 'cursor-pointer'"
                      :item-info="item"
                      :icon-text-color="panelIconTextColor"
                      :icon-text-info-hide-description="panelState.panelConfig.iconTextInfoHideDescription || false"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="0"
                      @click="handleItemClick(itemGroupIndex, item)"
                    />
                  </div>

                  <div v-if="itemGroup.items.length === 0 && loadedGroups.has(Number(itemGroup.id)) && canWrite" class="not-drag">
                    <AppIcon
                      :class="itemGroup.sortStatus ? 'cursor-move' : 'cursor-pointer'"
                      :item-info="{ icon: { itemType: 3, text: 'subway:add' }, title: t('common.add'), url: '', openMethod: 0 }"
                      :icon-text-color="panelIconTextColor"
                      :icon-text-info-hide-description="panelState.panelConfig.iconTextInfoHideDescription || false"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="0"
                      @click="handleAddItem(itemGroup.id)"
                    />
                  </div>
                </VueDraggable>
              </div>
            </div>

            <!-- APP图标宫型盒子 -->
            <div v-if="directoryLayout || panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.icon">
              <div v-if="itemGroup.items && !collapsedGroups.has(Number(itemGroup.id))">
                <VueDraggable
                  v-model="itemGroup.items" item-key="sort" :animation="300"
                  class="icon-small-box"

                  filter=".not-drag"
                  :disabled="!itemGroup.sortStatus"
                >
                  <div v-for="item, index in itemGroup.items" :key="index" :title="item.description" @contextmenu="(e) => handleContextMenu(e, itemGroupIndex, item)">
                    <AppIcon
                      :class="itemGroup.sortStatus ? 'cursor-move' : 'cursor-pointer'"
                      :item-info="item"
                      :icon-text-color="panelIconTextColor"
                      :icon-text-info-hide-description="!panelState.panelConfig.iconTextInfoHideDescription"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="1"
                      :directory="directoryLayout"
                      @click="handleItemClick(itemGroupIndex, item)"
                    />
                  </div>

                  <div v-if="itemGroup.items.length === 0 && loadedGroups.has(Number(itemGroup.id)) && canWrite" class="not-drag">
                    <AppIcon
                      class="cursor-pointer"
                      :item-info="{ icon: { itemType: 3, text: 'subway:add' }, title: $t('common.add'), url: '', openMethod: 0 }"
                      :icon-text-color="panelIconTextColor"
                      :icon-text-info-hide-description="!panelState.panelConfig.iconTextInfoHideDescription"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="1"
                      :directory="directoryLayout"
                      @click="handleAddItem(itemGroup.id)"
                    />
                  </div>
                </VueDraggable>
              </div>
            </div>

            <!-- 编辑栏 -->
            <div v-if="itemGroup.sortStatus" class="flex mt-[10px]">
              <div>
                <NButton color="#2a2a2a6b" @click="handleSaveSort(itemGroup)">
                  <template #icon>
                    <SvgIcon class="text-white font-xl" icon="material-symbols:save" />
                  </template>
                  <div>
                    {{ $t('common.saveSort') }}
                  </div>
                </NButton>
              </div>
            </div>
          </div>
        </div>
        <div class="mt-5 footer" v-html="panelState.panelConfig.footerHtml" />
      </div>
    </div>

    <!-- 右键菜单 -->
    <NDropdown
      v-if="homeReady && parsePublicCodeFromPath() === '' && authStore.token"
      placement="bottom-start" trigger="manual" :x="dropdownMenuX" :y="dropdownMenuY"
      :options="getDropdownMenuOptions()" :show="dropdownShow" :on-clickoutside="onClickoutside" @select="handleRightMenuSelect"
    />

    <!-- 悬浮按钮 -->
    <Teleport to="body">
    <div v-if="homeReady && parsePublicCodeFromPath() === '' && authStore.token && canWrite" class="fixed-element shadow-[0_0_10px_2px_rgba(0,0,0,0.2)]">
      <NButtonGroup vertical>
        <NButton data-testid="floating-refresh-button" color="#2a2a2a6b" :title="$t('common.refresh')" @mousedown="handleFloatingButtonMouseDown" @click="handleFloatingButtonClick($event, refreshCurrentSpace)">
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="material-symbols:refresh-rounded" />
          </template>
        </NButton>
        <NButton data-testid="floating-top-button" color="#2a2a2a6b" :title="$t('spaceManage.backToTop')" @mousedown="handleFloatingButtonMouseDown" @click="handleFloatingButtonClick($event, scrollToTop)">
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="icon-park-outline:to-top" />
          </template>
        </NButton>
        <!-- 网络模式切换按钮组 -->
        <NButton
          v-if="panelState.networkMode === PanelStateNetworkModeEnum.lan && panelState.panelConfig.netModeChangeButtonShow" color="#2a2a2a6b"
          data-testid="floating-wan-button" :title="t('panelHome.changeToWanModel')" @mousedown="handleFloatingButtonMouseDown" @click="handleFloatingButtonClick($event, () => handleChangeNetwork(PanelStateNetworkModeEnum.wan))"
        >
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="material-symbols:lan-outline-rounded" />
          </template>
        </NButton>

        <NButton
          v-if="panelState.networkMode === PanelStateNetworkModeEnum.wan && panelState.panelConfig.netModeChangeButtonShow" color="#2a2a2a6b"
          data-testid="floating-lan-button" :title="t('panelHome.changeToLanModel')" @mousedown="handleFloatingButtonMouseDown" @click="handleFloatingButtonClick($event, () => handleChangeNetwork(PanelStateNetworkModeEnum.lan))"
        >
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="mdi:wan" />
          </template>
        </NButton>

        <NButton data-testid="system-settings-button" color="#2a2a2a6b" @mousedown="handleFloatingButtonMouseDown" @click="handleFloatingButtonClick($event, () => { settingModalShow = !settingModalShow })">
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="majesticons-applications" />
          </template>
        </NButton>
      </NButtonGroup>

      <AppStarter v-model:visible="settingModalShow" @spaces-changed="handleSpacesChanged" />
    </div>
    </Teleport>

    <NBackTop
      v-if="homeReady && !themeRuntimeActive"
      :listen-to="scrollContainerRef"
      :right="10"
      :bottom="10"
      style="background-color:transparent;border: none;box-shadow: none;"
    >
      <div class="shadow-[0_0_10px_2px_rgba(0,0,0,0.2)]">
        <NButton color="#2a2a2a6b">
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="icon-park-outline:to-top" />
          </template>
        </NButton>
      </div>
    </NBackTop>

    <EditItem v-model:visible="editItemInfoShow" :item-info="editItemInfoData" :item-group-id="currentAddItenIconGroupId" :space-id="activeSpace?.id" :mutations="homeMutations" />

    <!-- 弹窗 -->
    <NModal
      v-model:show="windowShow" :mask-closable="false" preset="card"
      style="max-width: 1000px;height: 600px;border-radius: 1rem;" :bordered="true" size="small" role="dialog"
      aria-modal="true"
    >
      <template #header>
        <div class="flex items-center">
          <span class="mr-[20px]">
            {{ windowTitle }}
          </span>

          <NSpin v-if="windowIframeIsLoad" size="small" />
        </div>
      </template>
      <div class="w-full h-full rounded-2xl overflow-hidden border dark:border-zinc-700">
        <div v-if="windowIframeIsLoad" class="flex flex-col p-5">
          <NSkeleton height="50px" width="100%" class="rounded-lg" />
          <NSkeleton height="180px" width="100%" class="mt-[20px] rounded-lg" />
          <NSkeleton height="180px" width="100%" class="mt-[20px] rounded-lg" />
        </div>
        <iframe
          v-show="!windowIframeIsLoad" id="windowIframeId" :src="windowSrc"
          class="w-full h-full" frameborder="0" @load="handWindowIframeIdLoad"
        />
      </div>
    </NModal>
  </div>
  <NModal v-model:show="themeRuntimeConsentVisible" preset="card" class="theme-runtime-consent-modal" style="width: min(92vw, 560px)" :title="$t('themeTrustedRuntime.consentTitle')" :mask-closable="false">
    <div class="theme-runtime-consent-content" data-testid="theme-runtime-consent-dialog">
      <p class="theme-runtime-consent-package">{{ themeRuntimePackage?.manifest.name }} · {{ themeRuntimePackage?.revision?.slice(0, 12) }}</p>
      <p>{{ $t('themeTrustedRuntime.consentIntro') }}</p>
      <NRadioGroup v-model:value="selectedThemeExecutionMode" class="theme-runtime-mode-list" :aria-label="$t('themeTrustedRuntime.modeGroupLabel')">
        <label class="theme-runtime-mode-option" :class="{ 'theme-runtime-mode-option--selected': selectedThemeExecutionMode === 'sandbox' }">
          <NRadio value="sandbox" />
          <span>
              <strong>{{ $t('themeTrustedRuntime.sandboxMode') }}</strong>
              <small>{{ $t('themeTrustedRuntime.sandboxDescription') }}</small>
          </span>
        </label>
        <label class="theme-runtime-mode-option" :class="{ 'theme-runtime-mode-option--selected': selectedThemeExecutionMode === 'trusted' }">
          <NRadio value="trusted" :disabled="!trustedRuntimeAvailable" />
          <span>
            <strong>{{ $t('themeTrustedRuntime.trustedMode') }}</strong>
            <small>{{ trustedRuntimeAvailable ? $t('themeTrustedRuntime.trustedDescription') : $t('themeTrustedRuntime.trustedUnavailable') }}</small>
          </span>
        </label>
      </NRadioGroup>
      <ul class="theme-runtime-permissions" :aria-label="$t('themeTrustedRuntime.permissionsLabel')">
        <li v-for="permission in themeRequiredPermissions" :key="permission">{{ permission }}</li>
        <li v-if="!themeRequiredPermissions.length">{{ $t('themePackage.noRuntimePermissions') }}</li>
      </ul>
      <section v-if="selectedThemeExecutionMode === 'trusted'" class="theme-runtime-risk" role="alert" data-testid="theme-trusted-risk">
        <h3>{{ $t('themeTrustedRuntime.warningTitle') }}</h3>
        <p>{{ $t('themeTrustedRuntime.warning') }}</p>
        <p>{{ $t('themeTrustedRuntime.serverBoundary') }}</p>
        <NCheckbox v-model:checked="trustedRuntimeAcknowledged" data-testid="theme-trusted-acknowledgement">
          {{ $t('themeTrustedRuntime.acknowledge') }}
        </NCheckbox>
      </section>
    </div>
    <template #footer>
      <div class="theme-runtime-consent-actions">
        <NButton quaternary @click="themeRuntimeConsentVisible = false">{{ $t('common.cancel') }}</NButton>
        <NButton
          type="primary"
          :loading="themeRuntimeGrantSaving"
          :disabled="selectedThemeExecutionMode === 'trusted' && (!trustedRuntimeAvailable || !trustedRuntimeAcknowledged)"
          data-testid="theme-runtime-consent-confirm"
          @click="grantThemeRuntimePermissions"
        >
          {{ $t(selectedThemeExecutionMode === 'trusted' ? 'themeTrustedRuntime.trustedEnable' : 'themePackage.runtimeEnable') }}
        </NButton>
      </div>
    </template>
  </NModal>
  <NModal v-model:show="createSpaceVisible" preset="dialog" :title="$t('spaceManage.createSpace')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" :loading="creatingSpace" @positive-click="submitCreateSpace">
    <div data-testid="create-space-modal"><NInput v-model:value="spaceName" :placeholder="$t('spaceManage.newSpaceName')" maxlength="100" show-count @keyup.enter="submitCreateSpace" /></div>
  </NModal>
  <NModal v-model:show="groupCreateVisible" preset="dialog" :title="$t('spaceManage.addGroup')" :positive-text="$t('common.confirm')" :negative-text="$t('common.cancel')" :loading="creatingGroup" @positive-click="submitCreateGroup">
    <div data-testid="create-group-modal"><NInput v-model:value="groupName" :placeholder="$t('spaceManage.groupName')" maxlength="100" @keyup.enter="submitCreateGroup" /></div>
  </NModal>
</template>

<style scoped>
.space-status-bar {
  position: fixed;
  z-index: 20;
  top: 14px;
  left: 50%;
  transform: translateX(-50%);
  padding: var(--yin-component-sidebar-padding);
  border: var(--yin-component-button-border-width) solid var(--yin-border);
  border-radius: var(--yin-component-sidebar-radius);
  background: color-mix(in srgb, var(--yin-surfaceElevated) calc(var(--yin-component-surface-opacity) * 100%), transparent);
  backdrop-filter: blur(var(--yin-component-surface-blur));
  box-shadow: var(--yin-component-menu-shadow);
}
.space-status-button { color: var(--yin-text); min-width: 140px; }
/* These overrides must beat the later scoped `theme-defaults` /
   `:root[data-yin-layout='directory']` rules in the second style block, which
   carry a `[data-v-*]` attribute and therefore outrank a plain global
   selector. Without `!important` the pill keeps the light surface background
   while its label stays forced white, making it invisible. */
:global(.sun-main.theme-runtime-yin .space-status-bar) { padding: 2px 12px; border-color: rgb(255 255 255 / 42%) !important; border-radius: 999px; background: rgb(24 28 32 / 68%) !important; backdrop-filter: blur(8px); box-shadow: 0 4px 18px rgb(0 0 0 / 18%); }
:global(.sun-main.theme-runtime-yin .space-status-button) { min-width: 122px !important; color: #fff !important; }
.space-status-dot { width: 7px; height: 7px; margin-right: 8px; border-radius: 50%; background: var(--yin-success); box-shadow: 0 0 var(--yin-component-surface-glow) var(--yin-success); }
.offline-status { position: fixed; z-index: 21; top: 14px; right: 18px; display: flex; gap: var(--yin-component-group-gap); color: var(--yin-text); font-size: var(--yin-fontSmallSize); text-shadow: var(--yin-effect-text-shadow); }
.theme-safe-mode-banner { position: fixed; z-index: 22; top: 48px; right: 18px; display: flex; align-items: center; gap: 12px; max-width: min(440px, calc(100vw - 36px)); padding: 8px 12px; border: 1px solid var(--yin-border); border-radius: var(--yin-component-card-radius); background: var(--yin-surfaceElevated); color: var(--yin-text); font-size: var(--yin-fontSmallSize); }
.theme-safe-mode-banner a { color: var(--yin-primary); text-decoration: underline; }
.offline-unavailable { position: fixed; z-index: 31; inset: 0; display: grid; place-items: center; padding: 20px; background: rgba(0, 0, 0, 0.48); }
.theme-home-host { position: absolute; z-index: 1; inset: 0; overflow: auto; pointer-events: auto; }
.theme-runtime-monitor-layer { position: absolute; z-index: 5; top: 72px; left: var(--yin-pageGutter); right: var(--yin-pageGutter); max-height: 220px; overflow: auto; pointer-events: none; }
.theme-runtime-monitor-layer > * { pointer-events: none; }
.theme-runtime-monitor-layer--info { max-height: 260px; }
.theme-runtime-monitor-layer--yin { top: clamp(169.5px, 23.1vh, 208px); right: auto; left: 50%; width: min(calc(var(--yin-contentMaxWidth, 1200px) - 30px), calc(100vw - 30px)); max-width: none; max-height: 286px; padding-top: 26px; box-sizing: border-box; transform: translateX(-50%); --yin-text: #fff; --yin-textMuted: rgb(255 255 255 / 82%); --yin-primary: #fff; --yin-border: transparent; --yin-component-app-icon-surface: rgb(42 42 42 / 42%); --yin-component-system-monitor-heading-size: 18px; }
:global(.theme-runtime-monitor-layer--yin .n-progress-graph-circle-rail) { stroke: rgb(255 255 255 / 36%) !important; }
@media (max-width: 640px) {
  :global(.theme-runtime-monitor-layer--yin:not(.theme-runtime-monitor-layer--directory)) { top: 169.5px; }
}
:global(.sun-main.theme-runtime-yin .wallpaper-media) { transform: scale(1.05); }
:global(.sun-main.theme-runtime-yin .offline-status--theme-hidden) { display: none; }
.theme-runtime-notice { display: flex; align-items: center; justify-content: space-between; gap: var(--yin-component-group-gap); margin: var(--yin-component-group-section-spacing) auto; padding: var(--yin-component-card-padding); border: var(--yin-component-button-border-width) solid var(--yin-border); border-radius: var(--yin-component-card-radius); background: var(--yin-surfaceElevated); color: var(--yin-text); }
.theme-runtime-notice--error { border-color: var(--yin-danger); }
.theme-runtime-consent-content { display: grid; gap: var(--yin-spaceMd); color: var(--yin-text); line-height: var(--yin-lineHeightBody); }
.theme-runtime-consent-content > p { margin: 0; }
.theme-runtime-consent-package { color: var(--yin-textMuted); font-size: var(--yin-fontSmallSize); overflow-wrap: anywhere; }
.theme-runtime-mode-list { display: grid; gap: var(--yin-spaceSm); }
.theme-runtime-mode-option { display: flex; min-height: 56px; align-items: flex-start; gap: var(--yin-spaceSm); padding: var(--yin-spaceMd); border: var(--yin-borderWidth) solid var(--yin-border); border-radius: var(--yin-component-card-radius); background: var(--yin-surface); cursor: pointer; }
.theme-runtime-mode-option--selected { border-color: var(--yin-focusRing); box-shadow: inset 0 0 0 var(--yin-effect-focus-width) var(--yin-focusRing); }
.theme-runtime-mode-option > span { display: grid; gap: var(--yin-spaceXs); }
.theme-runtime-mode-option small { color: var(--yin-textMuted); font-size: var(--yin-fontSmallSize); }
.theme-runtime-permissions { display: flex; flex-wrap: wrap; gap: var(--yin-spaceXs); margin: 0; padding: 0; list-style: none; }
.theme-runtime-permissions li { max-width: 100%; padding: var(--yin-spaceXs) var(--yin-spaceSm); border-radius: var(--yin-component-button-radius); background: var(--yin-surface); color: var(--yin-text); font: var(--yin-fontSmallSize)/var(--yin-lineHeightBody) ui-monospace, monospace; overflow-wrap: anywhere; }
.theme-runtime-risk { display: grid; gap: var(--yin-spaceSm); padding: var(--yin-spaceMd); border-inline-start: var(--yin-effect-focus-width) solid var(--yin-danger); background: color-mix(in srgb, var(--yin-danger) 8%, var(--yin-surfaceElevated)); }
.theme-runtime-risk h3, .theme-runtime-risk p { margin: 0; }
.theme-runtime-risk h3 { font-size: var(--yin-fontBodySize); }
.theme-runtime-consent-actions { display: flex; justify-content: flex-end; flex-wrap: wrap; gap: var(--yin-spaceSm); }
.theme-runtime-consent-actions :deep(.n-button) { min-height: max(var(--yin-component-button-height), 44px); }
</style>

<style>
body,
html {
  overflow: hidden;
  background-color: var(--yin-canvas);
}
:root[data-yin-layout='directory'] body { background-color: transparent; }
:root[data-yin-layout='directory'] .directory-folders button { min-height: max(var(--yin-component-button-height), 44px); }
</style>

<style scoped>
.mask {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
}

.sun-main {
  user-select: none;
}

.sun-main.theme-defaults {
  background-color: var(--yin-canvas);
  color: var(--yin-text);
}

:global(:root[data-yin-layout='directory'] .sun-main) { background: transparent; color: var(--yin-text); }

.sun-main.theme-defaults .home-header-row,
.sun-main.theme-defaults .item-list > div:first-child {
  color: var(--yin-text) !important;
}

.sun-main.theme-defaults .text-shadow,
.sun-main.theme-defaults .app-icon-text-shadow {
  text-shadow: none;
}

.sun-main.theme-defaults .space-status-bar,
:root[data-yin-layout='directory'] .space-status-bar {
  border-color: var(--yin-border);
  background-color: var(--yin-surfaceElevated);
}

.sun-main.theme-defaults .space-status-button,
.sun-main.theme-defaults .offline-status,
:root[data-yin-layout='directory'] .space-status-button,
:root[data-yin-layout='directory'] .offline-status {
  color: var(--yin-text);
  text-shadow: none;
}

.sun-main.side-switching {
  animation: panel-content-pulse var(--yin-component-state-space-transition-duration, 1000ms) var(--yin-component-state-space-transition-easing, ease-in-out);
}

@keyframes panel-content-pulse {
  0% {
    filter: brightness(1);
  }
  45% {
    filter: brightness(0.84);
  }
  100% {
    filter: brightness(1);
  }
}

.taiji-transition {
  position: fixed;
  z-index: 30;
  inset: 0;
  display: grid;
  place-items: center;
  pointer-events: none;
}

.taiji-aura {
  position: relative;
  width: clamp(120px, 22vw, 190px);
  aspect-ratio: 1;
  display: grid;
  place-items: center;
  animation: taiji-aura var(--yin-component-state-space-transition-duration, 1000ms) var(--yin-component-state-space-transition-easing, cubic-bezier(0.22, 0.61, 0.36, 1)) both;
}

.taiji-symbol {
  --taiji-start-angle: 0deg;
  position: relative;
  z-index: 1;
  width: 58%;
  aspect-ratio: 1;
  overflow: hidden;
  border: var(--yin-effect-focus-width, 2px) solid color-mix(in srgb, var(--yin-surfaceElevated) 82%, transparent);
  border-radius: 50%;
  background: linear-gradient(90deg, var(--yin-text) 0 50%, var(--yin-surfaceElevated) 50%);
  box-shadow: 0 0 var(--yin-component-surface-glow, 34px) color-mix(in srgb, var(--yin-surfaceElevated) 30%, transparent), 0 0 calc(var(--yin-component-surface-glow, 34px) * 2.35) color-mix(in srgb, var(--yin-text) 30%, transparent);
  animation: taiji-spin var(--yin-component-state-space-transition-duration, 1000ms) var(--yin-component-state-space-transition-easing, cubic-bezier(0.22, 0.61, 0.36, 1)) both;
}

.taiji-symbol.taiji-yang {
  --taiji-start-angle: 180deg;
}

.taiji-symbol::before,
.taiji-symbol::after {
  position: absolute;
  left: 25%;
  width: 50%;
  height: 50%;
  content: '';
  border-radius: 50%;
}

.taiji-symbol::before {
  top: 0;
  background: var(--yin-surfaceElevated);
}

.taiji-symbol::after {
  bottom: 0;
  background: var(--yin-text);
}

.taiji-dot {
  position: absolute;
  z-index: 2;
  width: 12%;
  aspect-ratio: 1;
  border-radius: 50%;
}

.taiji-dot-dark {
  top: 25%;
  left: 44%;
  background: var(--yin-text);
}

.taiji-dot-light {
  bottom: 25%;
  left: 44%;
  background: var(--yin-surfaceElevated);
}

.taiji-bagua {
  position: absolute;
  inset: 0;
  border: 1px solid rgba(255, 255, 255, 0.42);
  border-radius: 50%;
  animation: taiji-spin var(--yin-component-state-space-transition-duration, 1000ms) var(--yin-component-state-space-transition-easing, cubic-bezier(0.22, 0.61, 0.36, 1)) reverse both;
}

.taiji-bagua span {
  position: absolute;
  top: 50%;
  left: 50%;
  width: 16%;
  height: 3px;
  transform: rotate(calc(var(--index, 0) * 45deg)) translateX(255%);
  transform-origin: left center;
  background: rgba(255, 255, 255, 0.72);
  box-shadow: 6px 0 0 rgba(255, 255, 255, 0.72);
}

.taiji-bagua span:nth-child(1) { --index: 0; }
.taiji-bagua span:nth-child(2) { --index: 1; }
.taiji-bagua span:nth-child(3) { --index: 2; }
.taiji-bagua span:nth-child(4) { --index: 3; }
.taiji-bagua span:nth-child(5) { --index: 4; }
.taiji-bagua span:nth-child(6) { --index: 5; }
.taiji-bagua span:nth-child(7) { --index: 6; }
.taiji-bagua span:nth-child(8) { --index: 7; }

@keyframes taiji-aura {
  0% { opacity: 0; transform: scale(0.72); }
  18%, 72% { opacity: 1; transform: scale(1); }
  100% { opacity: 0; transform: scale(0.72); }
}

@keyframes taiji-spin {
  from { transform: rotate(var(--taiji-start-angle, 0deg)); }
  to { transform: rotate(calc(var(--taiji-start-angle, 0deg) + 180deg)); }
}

@media (prefers-reduced-motion: reduce) {
  .sun-main.side-switching {
    animation: none;
  }
}

.cover {
  position: absolute;
  width: 100%;
  height: 100%;
  overflow: hidden;
  transform: scale(1.05);
}

.home-content { position: relative; }
.home-scroll-container { position: relative; z-index: 1; pointer-events: none; }
.home-scroll-container > * { pointer-events: auto; }
:root[data-yin-layout='directory'] .home-scroll-container { background: transparent; }
:root[data-yin-layout='split'] .home-content { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); column-gap: var(--yin-spaceLg); }
:root[data-yin-layout='directory'] .home-header { width: min(640px, 100%); }
:root[data-yin-layout='directory'] .home-header-row { display: none; }
:global(:root[data-yin-layout='directory'] .directory-brand-controls) { display: flex; justify-content: space-between; align-items: center; margin: 14px auto 0; color: var(--yin-text); font-size: var(--yin-fontBodySize); }
:global(:root[data-yin-layout='directory'] .directory-brand-controls > span) { flex: 0 0 auto; white-space: nowrap; }
:global(:root[data-yin-layout='directory'] .directory-brand-controls .directory-clock) { width: auto; flex: 0 0 auto; text-align: right; }
:global(:root[data-yin-layout='directory'] .directory-search) { margin-top: 8px; }
:global(:root[data-yin-layout='directory'] .home-content) { margin-top: var(--yin-component-group-section-spacing); padding: var(--yin-component-card-padding); border: var(--yin-component-card-border-width) var(--yin-component-card-border-style) var(--yin-border); border-radius: var(--yin-component-card-radius); background: var(--yin-surfaceElevated); box-shadow: var(--yin-component-card-shadow); }
:global(:root[data-yin-layout='directory'] .item-list) { min-height: 0; margin-top: 0; padding: var(--yin-component-group-section-spacing) 0; border-bottom: var(--yin-borderWidth) solid var(--yin-border); }
:global(:root[data-yin-layout='directory'] .item-list:last-child) { border-bottom: 0; }
:global(:root[data-yin-layout='directory'] .directory-group-heading) { margin: 0 0 var(--yin-spaceSm) !important; color: var(--yin-text); font-family: var(--yin-fontDisplay); font-size: var(--yin-fontBodySize); font-weight: var(--yin-fontHeadingWeight); }
:global(:root[data-yin-layout='directory'] .directory-group-heading .group-title) { text-shadow: none; }
:global(:root[data-yin-layout='directory'] .directory-folders) { display: flex; max-width: 100%; gap: var(--yin-component-group-gap); margin: var(--yin-component-group-section-spacing) auto 0; overflow-x: auto; padding: var(--yin-spaceXs) 0 var(--yin-spaceSm); scrollbar-width: thin; }
:global(:root[data-yin-layout='directory'] .directory-folders button) { display: flex; flex: 0 0 auto; align-items: center; gap: var(--yin-component-app-icon-gap); min-height: max(var(--yin-component-button-height), 44px); padding: 0 var(--yin-component-button-padding-x); border: var(--yin-component-button-border-width) solid var(--yin-border); border-radius: var(--yin-component-button-radius); background: var(--yin-component-sidebar-surface, var(--yin-surfaceElevated)); color: var(--yin-text); cursor: pointer; transition: all var(--yin-component-state-hover-duration) var(--yin-component-state-easing); }
:global(:root[data-yin-layout='directory'] .directory-folders button.active) { border-color: var(--yin-primary); color: var(--yin-primary); }
:global(:root[data-yin-layout='directory'] .icon-small-box) { display: flex; flex-wrap: wrap; gap: var(--yin-component-group-gap); }
:global(:root[data-yin-layout='directory'] .home-content .group-buttons svg) { color: var(--yin-text); }

.home-content--with-monitor {
  padding-top: 190px;
}

.home-content--monitor-info {
  padding-top: 230px;
}

.system-monitor-layer {
  position: absolute;
  z-index: 1;
  top: 0;
  left: 0;
  right: 0;
  max-width: var(--yin-contentMaxWidth);
  max-height: 210px;
  box-sizing: border-box;
  margin: 0 auto;
  overflow: auto;
  contain: layout;
}

.system-monitor-layer--info {
  max-height: 220px;
}

.system-monitor-layer--directory {
  position: relative;
  z-index: auto;
  inset: auto;
  max-width: none;
  max-height: none;
  margin-bottom: var(--yin-component-group-section-spacing);
  overflow: visible;
  contain: none;
}

.text-shadow {
  text-shadow: var(--yin-effect-text-shadow);
}

.app-icon-text-shadow {
  text-shadow: var(--yin-effect-text-shadow);
}

.item-list { margin-top: var(--yin-component-group-section-spacing); }
.item-list--sorting { padding: var(--yin-component-group-padding); border: var(--yin-component-card-border-width) var(--yin-component-card-border-style) var(--yin-border); border-radius: var(--yin-component-card-radius); background: var(--yin-component-card-surface, var(--yin-surfaceElevated)); box-shadow: var(--yin-component-card-shadow); }
.directory-group-heading { margin-bottom: var(--yin-component-group-gap); color: var(--yin-text); font-family: var(--yin-fontDisplay); font-size: var(--yin-component-group-heading-size); font-weight: var(--yin-component-group-heading-weight); }
.group-buttons { margin-left: var(--yin-spaceSm); transition: opacity var(--yin-component-state-hover-duration) var(--yin-component-state-easing); }
.group-buttons > span { margin-right: var(--yin-spaceSm); color: var(--yin-text); cursor: pointer; }
.group-action-icon { font-size: var(--yin-component-iconography-size); }

.fixed-element {
  position: fixed;
  z-index: 22;
  /* 将元素固定在屏幕上 */
  right: 10px;
  /* 距离屏幕顶部的距离 */
  bottom: 50px;
  /* 距离屏幕左侧的距离 */
}

.icon-info-box {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(var(--yin-activeColumns), minmax(0, 1fr));
  gap: var(--yin-component-group-gap);

}

.icon-small-box {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(75px, 1fr));
  gap: var(--yin-component-group-gap);

}

@media (max-width: 500px) {
  .space-status-bar {
    top: auto;
    bottom: 8px;
    left: auto;
    right: 8px;
    transform: none;
    max-width: calc(100vw - 16px);
  }
  .space-status-button {
    min-width: 0;
    max-width: calc(100vw - 28px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .icon-info-box{
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  }

  .home-header { width: calc(100% - 24px); padding-top: 42px; }
  .home-header-row { gap: 6px; }
  .home-header-row .logo span { font-size: 1.35rem; }
  .home-header-row .divider { margin-left: 4px; margin-right: 4px; }
  .icon-info-box { gap: 10px; grid-template-columns: 1fr; }
  :global(:root[data-yin-layout='directory'] .icon-small-box) { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--yin-component-group-gap); }
  .icon-small-box { gap: 12px 8px; grid-template-columns: repeat(auto-fill, minmax(70px, 1fr)); }
  .system-monitor { overflow: hidden; }
  .home-content--with-monitor { padding-top: 300px; }
  .home-content--monitor-info { padding-top: 380px; }
  .system-monitor-layer { max-height: 320px; }
  .system-monitor-layer--info { max-height: 370px; }
  .system-monitor-layer--directory { max-height: none; }
}
</style>
