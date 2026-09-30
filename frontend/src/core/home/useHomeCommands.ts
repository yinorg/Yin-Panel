import { computed, ref, type Ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '@/store'
import { t } from '@/locales'
import { PanelStateNetworkModeEnum } from '@/enums'
import { replaceOrAppendKeywordToUrl, searchEngineList, type SearchEngine } from '@/components/deskModule/SearchBox/engines'
import { createHomeThemeHandlers } from '@/core/home/themeHandlers'
import { executeHomeThemeRequest } from '@/core/home/themeRequest'
import {
  filterHomeCommandItems,
  filterHomeCommands,
  findHomeCommandItem,
  getInitialHomeCommandSelection,
  isHomeCommandWrite,
  moveHomeCommandSelection,
  parseHomeCommand,
} from '@/core/home/commandCenter'
import type { createThemePersistence } from '@/core/home/themePersistence'
import type { createHomeMutationService } from '@/core/home/mutations'
import type { Space } from '@/api/panel/space'

interface CommandGroup {
  id?: number | string
  parentId?: number | null
  title?: string
  icon?: string
  sort?: number
  items?: readonly Panel.ItemInfo[]
}

/**
 * The command centre and the Core-side capability bindings the theme calls into.
 *
 * This is the only place that knows how a theme command maps onto Core actions,
 * which is why it also owns the search-engine list: the theme has to be told the
 * available engines and which one is current, and selecting one changes that state.
 *
 * It deliberately receives its collaborators as an input object full of *handles*
 * rather than importing other composables, so the host stays the single place that
 * decides what is wired to what.
 */
export function useHomeCommands(input: {
  publicCode: string
  publicAccessReady: Ref<boolean>
  // Data channel
  items: Ref<CommandGroup[]>
  activeSpace: Ref<Space | null>
  spaces: Ref<Space[]>
  canWrite: Ref<boolean>
  selectSpace: (spaceId: number | string) => void
  togglePanelSide: () => void
  refreshCollection: (forceRefresh?: boolean) => void
  warmSearchCache: (query?: string) => void
  homeSearch: { query: (spaceId: number, query: string) => Promise<Panel.ItemInfo[]> }
  homeCommandSearch: { invalidate: () => void, search: (query: string) => Promise<Panel.ItemInfo[] | undefined> }
  // Modal layer
  openPage: (openMethod: number, url: string, title?: string) => void
  getItemOpenUrl: (item: Panel.ItemInfo, forceWan?: boolean) => string
  handleEditItem: (item: Panel.ItemInfo) => void
  handleAddItem: (groupId?: number) => void
  handleChangeNetwork: (mode: PanelStateNetworkModeEnum) => void
  homeMutations: ReturnType<typeof createHomeMutationService>
  groupCreateVisible: Ref<boolean>
  createSpaceVisible: Ref<boolean>
  settingModalShow: Ref<boolean>
  editItemInfoShow: Ref<boolean>
  windowShow: Ref<boolean>
  // Theme runtime and monitor
  themeCanWriteGroups: Ref<boolean>
  themePersistence: ReturnType<typeof createThemePersistence>
  createThemeRuntimeError: (code: string, message: string) => Error
  confirmThemeDelete: (message: string, action: (spaceId: number) => Promise<unknown>) => Promise<unknown>
  reportSearchBottom: (searchBottom: number) => void
  getMonitorSnapshot: () => Promise<unknown>
  scrollToTop: () => void
}) {
  const authStore = useAuthStore()
  const router = useRouter()

  const commandCenterVisible = ref(false)
  const commandCenterQuery = ref('')
  const commandCenterSelectedIndex = ref(-1)
  const remoteCommandItems = ref<Panel.ItemInfo[]>([])
  const commandCenterSearchEngine = ref<SearchEngine>(searchEngineList[0])

  /** Stable id for an engine, so the theme can echo back which one is current. */
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

  const filteredCommandDefinitions = computed(() => input.publicCode ? [] : filterHomeCommands(commandDefinitions.value, commandCenterQuery.value))
  const allCommandItems = computed(() => input.items.value.flatMap(group => group.items || []))
  const commandCenterItems = computed(() => filterHomeCommandItems(commandCenterQuery.value, remoteCommandItems.value, allCommandItems.value))

  function openCommandCenter(query: string) {
    updateCommandCenterQuery(query)
    commandCenterSelectedIndex.value = getInitialHomeCommandSelection(query)
    commandCenterVisible.value = true
  }

  function closeCommandCenter() {
    input.homeCommandSearch.invalidate()
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
    if (!input.canWrite.value && isHomeCommandWrite(command)) return
    const { argument: keyword } = parseHomeCommand(commandCenterQuery.value)
    if (command === 'add') input.handleAddItem()
    else if (command === 'group') input.groupCreateVisible.value = true
    else if (command === 'space') input.createSpaceVisible.value = true
    else if (command === 'settings') input.settingModalShow.value = true
    else if (command === 'top') input.scrollToTop()
    else {
      const item = findHomeCommandItem(keyword, remoteCommandItems.value, allCommandItems.value)
      if (!item) return
      if (command === 'open') input.openPage(item.openMethod, input.getItemOpenUrl(item), item.title)
      else if (command === 'lan' && item.lanUrl) input.openPage(item.openMethod, item.lanUrl, item.title)
      else if (command === 'wan') input.openPage(item.openMethod, input.getItemOpenUrl(item, true), item.title)
      else if (command === 'edit') input.handleEditItem({ ...item })
      else if (command === 'copy') navigator.clipboard?.writeText(item.url)
    }
    closeCommandCenter()
  }

  function executeCommandItem(item: Panel.ItemInfo) {
    input.openPage(item.openMethod, input.getItemOpenUrl(item), item.title)
    closeCommandCenter()
  }

  function updateCommandCenterQuery(query: string) {
    const wasCommandQuery = commandCenterQuery.value.startsWith('/')
    const isCommandQuery = query.startsWith('/')
    commandCenterQuery.value = query
    if (!query.trim() || query.startsWith('/')) {
      input.homeCommandSearch.invalidate()
      remoteCommandItems.value = []
    }
    else {
      void input.homeCommandSearch.search(query).then((matches) => {
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

  /** A modal (or a live modal/drawer body) must swallow the shortcut so typing in a
   *  dialog never opens the command centre. */
  function hasBlockingLayer() {
    if (input.settingModalShow.value || input.editItemInfoShow.value || input.createSpaceVisible.value || input.groupCreateVisible.value || input.windowShow.value) return true
    return Array.from(document.querySelectorAll('.n-modal-container, .n-drawer-container')).some((element) => {
      const style = window.getComputedStyle(element)
      if (style.display === 'none' || style.visibility === 'hidden' || Number(style.opacity) === 0) return false
      return Array.from(element.querySelectorAll('.n-modal-body-wrapper, .n-drawer-body-content')).some(container => container.firstElementChild !== null)
    })
  }

  /** Single-letter typing anywhere on the home opens the command centre, unless a
   *  dialog owns the keyboard or the visitor still needs to unlock a public link. */
  function handleGlobalKeydown(event: KeyboardEvent) {
    if (isEditableTarget(event.target)) return
    if (event.isComposing) return
    // Modifier combinations stay browser shortcuts; the forwarded path applies the
    // same rule in the theme before it sends anything.
    if (event.ctrlKey || event.metaKey || event.altKey) return
    if (!handleShortcutKey(event.key)) return
    event.preventDefault()
  }

  /**
   * The shortcut policy itself, kept separate from the DOM event so the sandboxed
   * theme can reach it: keys typed inside a cross-origin frame never bubble to this
   * document's window listener, so without this the global shortcut silently stops
   * working the moment the home is rendered by a theme. Returns whether the key was
   * consumed, which is what tells the DOM path to preventDefault.
   */
  function handleShortcutKey(key: string) {
    // A signed-in visitor and a public-link visitor both get the shortcut; a
    // public link only once its access has been resolved, so typing into the
    // access-code prompt never opens the palette behind it.
    if (commandCenterVisible.value || (!authStore.token && !input.publicCode) || (input.publicCode && !input.publicAccessReady.value)) return false
    if (hasBlockingLayer()) return false
    if (key.length !== 1) return false
    openCommandCenter(key)
    return true
  }

  const homeThemeHandlers = createHomeThemeHandlers({
    getSpaces: () => input.spaces.value,
    getGroups: () => input.items.value.filter(group => Number.isSafeInteger(Number(group.id))).map(group => ({
      id: Number(group.id),
      parentId: group.parentId,
      title: group.title,
      icon: group.icon,
      sort: group.sort,
      items: group.items || [],
    })),
    getActiveSpaceId: () => input.activeSpace.value?.id,
    canWriteGroups: () => input.themeCanWriteGroups.value,
    selectSpace: spaceId => input.selectSpace(spaceId),
    openItem: item => { input.openPage(item.openMethod, input.getItemOpenUrl(item), item.title) },
    openEditor: ({ item, groupId }) => item ? input.handleEditItem(item) : input.handleAddItem(groupId),
    createItem: async (payload) => { await input.homeMutations.createItem(payload) },
    updateItem: async (itemId, payload) => { await input.homeMutations.updateItem(itemId, payload) },
    deleteItem: item => input.confirmThemeDelete(
      t('common.deleteConfirmByName', { name: item.title }),
      spaceId => input.homeMutations.deleteItem(Number(item.id), spaceId),
    ),
    reorderItems: (groupId, itemIds) => input.homeMutations.reorderItems(groupId, itemIds),
    createGroup: group => input.homeMutations.createGroup({ title: group.title, icon: group.icon || '', parentId: group.parentId ?? null }),
    updateGroup: (groupId, group) => input.homeMutations.updateGroup(groupId, { title: group.title, icon: group.icon || '', parentId: group.parentId ?? null }),
    deleteGroup: group => input.confirmThemeDelete(
      t('spaceManage.deleteWarnText', { name: group.title || '' }),
      spaceId => input.homeMutations.deleteGroup(group.id, spaceId),
    ),
    reorderGroups: (parentId, groupIds) => input.homeMutations.reorderGroups(parentId, groupIds),
    openCommandCenter: () => { commandCenterVisible.value = true },
    toggleSide: input.togglePanelSide,
    setNetworkMode: mode => input.handleChangeNetwork(mode === 'lan' ? PanelStateNetworkModeEnum.lan : PanelStateNetworkModeEnum.wan),
    reportLayout: ({ searchBottom }) => input.reportSearchBottom(searchBottom),
    forwardKey: key => handleShortcutKey(key),
    openLink: url => input.openPage(2, url),
    refresh: () => input.refreshCollection(true),
    searchItems: query => input.activeSpace.value ? input.homeSearch.query(input.activeSpace.value.id, query) : [],
    getMonitorSnapshot: input.getMonitorSnapshot,
    submitSearch: query => input.warmSearchCache(query),
    getSearchConfiguration: () => ({
      engines: themeSearchEngineConfiguration.value.engines.map(engine => ({ id: engine.id })),
      currentEngineId: themeSearchEngineConfiguration.value.currentSearchEngine.id,
    }),
    submitSearchWithEngine: (query, engineId) => {
      const engine = themeSearchEngineConfiguration.value.engines.find(candidate => candidate.id === engineId)
      if (!engine) throw input.createThemeRuntimeError('INVALID_ARGUMENT', 'Search engine is not available in the active Space')
      // Record the selection in Core so the next theme snapshot reports the new
      // currentEngineId; otherwise the theme select keeps the previous icon.
      commandCenterSearchEngine.value = { title: engine.title, iconSrc: engine.iconSrc, url: engine.url }
      window.open(replaceOrAppendKeywordToUrl(engine.url, query))
    },
    navigate: (destination) => {
      if (destination.view !== 'home') throw input.createThemeRuntimeError('UNSUPPORTED_CAPABILITY', 'Core navigation destination is unavailable')
    },
    getSettings: input.themePersistence.getSettings,
    patchSettings: input.themePersistence.patchSettings,
    getStorage: input.themePersistence.getStorage,
    setStorage: input.themePersistence.setStorage,
    removeStorage: input.themePersistence.removeStorage,
    openCoreSurface: async (surface) => {
      if (surface === 'theme-settings') {
        await router.push('/settings/style')
        return
      }
      throw input.createThemeRuntimeError('UNSUPPORTED_CAPABILITY', 'Core surface is unavailable')
    },
    // The broker keeps the theme inside the Core's own origin and to read-only
    // methods, so a compromised theme cannot use the page's credentials elsewhere.
    networkFetch: async (request, init) => {
      const url = new URL(request, window.location.origin)
      if (url.origin !== window.location.origin || !url.pathname.startsWith('/api/'))
        throw input.createThemeRuntimeError('PERMISSION_DENIED', 'Theme network requests must target the Core origin')
      const method = typeof init.method === 'string' ? init.method.toUpperCase() : 'GET'
      if (!['GET', 'HEAD'].includes(method)) throw input.createThemeRuntimeError('PERMISSION_DENIED', 'Theme network broker allows read-only requests')
      const response = await fetch(url, { method, credentials: 'omit', redirect: 'error' })
      const body = await response.text()
      return { status: response.status, headers: Object.fromEntries(response.headers.entries()), body }
    },
    reportDiagnostic: async entry => { console[entry.level === 'error' ? 'error' : entry.level === 'warn' ? 'warn' : 'info']('[theme]', entry.message, entry.data || {}) },
  })

  /** Dispatch one theme request through the capability bindings. It lives here
   *  because the handlers are defined here; nothing outside needs to see them. */
  function executeThemeRequest(value: unknown) {
    return executeHomeThemeRequest(homeThemeHandlers, value)
  }

  return {
    executeThemeRequest,
    commandCenterVisible,
    commandCenterQuery,
    commandCenterSelectedIndex,
    remoteCommandItems,
    commandCenterSearchEngine,
    themeSearchEngineConfiguration,
    commandDefinitions,
    filteredCommandDefinitions,
    commandCenterItems,
    openCommandCenter,
    closeCommandCenter,
    moveCommandSelection,
    selectCommandItem,
    submitCommandCenterSearch,
    executeCommand,
    executeCommandItem,
    updateCommandCenterQuery,
    handleGlobalKeydown,
    homeThemeHandlers,
  }
}
