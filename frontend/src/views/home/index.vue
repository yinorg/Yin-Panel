<script setup lang="ts">
import { VueDraggable } from 'vue-draggable-plus'
import { NBackTop, NButton, NButtonGroup, NCard, NDropdown, NInput, NModal, NSkeleton, NSpin, NSpace, useDialog, useMessage } from 'naive-ui'
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { createGroup, createSpace, getGroups, getItems, getSpaces, sortSpaces, spaceDisplayName, type Space } from '../../api/panel/space'
import Clock from '../../components/deskModule/Clock/index.vue'
import SearchBox from '../../components/deskModule/SearchBox/index.vue'
import { replaceOrAppendKeywordToUrl, searchEngineList, type SearchEngine } from '../../components/deskModule/SearchBox/engines'
import SvgIcon from '../../components/common/SvgIcon/index.vue'
import SvgIconOnline from '../../components/common/SvgIconOnline/index.vue'
import AppIcon from './components/AppIcon/index.vue'
import CommandCenter from './components/CommandCenter/index.vue'
import { deleteItem, sortItems } from '@/api/panel/space'

import { setTitle } from '@/utils/cmn'
import { parsePublicCodeFromPath } from '@/utils/request/axios'
import { usePanelState, useAuthStore } from '@/store'
import { PanelPanelConfigStyleEnum, PanelStateNetworkModeEnum } from '@/enums'
import { t } from '@/locales'
import { getEnableStatus } from '@/api/system/systemMonitor'
import { clearSpaceCache, readSpaceCache, readSpacesCache, writeSpaceCache, writeSpacesCache } from '@/utils/spaceCache'

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

const scrollContainerRef = ref<HTMLElement | undefined>(undefined)

const editItemInfoShow = ref<boolean>(false)
const editItemInfoData = ref<Panel.ItemInfo | null>(null)
const windowShow = ref<boolean>(false)
const windowSrc = ref<string>('')
const windowTitle = ref<string>('')

const windowIframeRef = ref(null)
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
const homeReady = ref(false)
const sideSwitching = ref(false)
let sideSwitchTimer: ReturnType<typeof setTimeout> | undefined
const commandCenterVisible = ref(false)
const commandCenterQuery = ref('')
const commandCenterSelectedIndex = ref(-1)
const commandCenterSearchEngine = ref<SearchEngine>(searchEngineList[0])
const groupCreateVisible = ref(false)
const groupName = ref('')
const creatingGroup = ref(false)

const items = ref<ItemGroup[]>([])
const filterItems = ref<ItemGroup[]>([])
const loadedGroups = new Set<number>()
let groupLoadGeneration = 0
const collapsedGroups = ref<Set<number>>(new Set())
const publicCode = parsePublicCodeFromPath()
const publicAccessCode = ref('')
const publicAccessReady = ref(!publicCode || !!sessionStorage.getItem(`yin-panel-public-access:${publicCode}`))

function getCachedSpace(spaceId: number) { return readSpaceCache(spaceId, authStore.userInfo?.id) }
function saveCachedSpace(spaceId: number, cache: any) { writeSpaceCache(spaceId, cache, authStore.userInfo?.id) }
function clearCachedSpace(spaceId: number) { clearSpaceCache(spaceId, authStore.userInfo?.id) }

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
  const isLan = panelState.networkMode === PanelStateNetworkModeEnum.lan
  if (!forceWan && isLan)
    return item.lanUrl || item.url
  const userAgent = typeof navigator === 'undefined' ? '' : navigator.userAgent
  const isMobile = /Android|webOS|iPhone|iPad|iPod|BlackBerry|IEMobile|Opera Mini|Mobile|Tablet/i.test(userAgent)
  return isMobile && item.mobileUrl ? item.mobileUrl : item.url
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

// 获取组数据
async function getList(forceRefresh = false) {
  const generation = ++groupLoadGeneration
  loadedGroups.clear()
  if (!activeSpace.value) {
    items.value = []
    filterItems.value = []
    return
  }

  const spaceId = activeSpace.value.id
  let groups: ItemGroup[] | undefined
  let cache = getCachedSpace(spaceId)
  if (!forceRefresh) {
    groups = cache.groups
  }
  if (!groups) {
    if (forceRefresh) {
      clearCachedSpace(spaceId)
      cache = getCachedSpace(spaceId)
    }
    const { data } = await getGroups<ItemGroup[]>(spaceId)
    if (!data) return
    groups = data
    cache.groups = groups
  }

  const itemsByGroup = new Map<number, Panel.ItemInfo[]>()
  const pendingGroups = groups.filter(group => !cache.items[String(group.id)])
  groups.forEach((group) => {
    const cachedItems = cache.items[String(group.id)]
    if (cachedItems) itemsByGroup.set(Number(group.id), cachedItems)
  })
  let nextGroup = 0
  const loadNextGroups = async () => {
    while (nextGroup < pendingGroups.length) {
      const group = pendingGroups[nextGroup++]
      let groupItems: Panel.ItemInfo[] = []
      try {
        const { data } = await getItems<Panel.ItemInfo[]>(spaceId, Number(group.id), 1, 100)
        groupItems = data || []
      }
      catch {
        // Keep the group at a stable height when an item request fails.
      }
      cache.items[String(group.id)] = groupItems
      itemsByGroup.set(Number(group.id), groupItems)
    }
  }
  await Promise.all(Array.from({ length: Math.min(2, pendingGroups.length) }, loadNextGroups))

  if (generation === groupLoadGeneration && activeSpace.value?.id === spaceId) {
    saveCachedSpace(spaceId, cache)
    applyGroups(groups, itemsByGroup)
  }
}

function applyGroups(data: ItemGroup[], itemsByGroup = new Map<number, Panel.ItemInfo[]>()) {
    const byParent = new Map<number, ItemGroup[]>()
    data.forEach(group => { const key = group.parentId || 0; const list = byParent.get(key) || []; list.push({ ...group, items: itemsByGroup.get(Number(group.id)) || [] }); byParent.set(key, list) })
    const flattened: ItemGroup[] = []
    function append(parentId: number, depth: number) { (byParent.get(parentId) || []).forEach(group => { group.depth = depth; flattened.push(group); append(Number(group.id), depth + 1) }) }
    append(0, 0)
    items.value = flattened
    const initiallyCollapsed = new Set<number>()
    flattened.forEach(group => {
      if ((byParent.get(Number(group.id)) || []).length || (group.items?.length || 0) > 40)
        initiallyCollapsed.add(Number(group.id))
    })
    collapsedGroups.value = initiallyCollapsed
    flattened.forEach(group => loadedGroups.add(Number(group.id)))
    filterItems.value = items.value
}

function groupHidden(index: number) {
  const group = items.value[index]
  if (!group?.parentId) return false
  let parent = items.value.find(item => Number(item.id) === Number(group.parentId))
  while (parent) {
    if (collapsedGroups.value.has(Number(parent.id))) return true
    parent = parent.parentId ? items.value.find(item => Number(item.id) === Number(parent?.parentId)) : undefined
  }
  return false
}

function toggleGroup(id: number) { const next = new Set(collapsedGroups.value); next.has(id) ? next.delete(id) : next.add(id); collapsedGroups.value = next }
function selectSpace(key: string | number) {
  groupLoadGeneration++
  loadedGroups.clear()
  const selected = spaces.value.find(space => space.id === Number(key))
  if (selected) { activeSpace.value = selected; getList() }
}
function togglePanelSide() {
  if (sideSwitching.value || !activeSpace.value?.pairedSpaceId) return
  const yin = activeSpace.value
  const target = yin.side === 'yang'
    ? spaces.value.find(space => space.id === yin.pairId)
    : { ...yin, id: yin.pairedSpaceId!, name: `${yin.name}-B`, side: 'yang' as const, pairId: yin.id, pairedSpaceId: yin.id }
  if (!target) return
  activeSpace.value = target
  getList()
  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
  sideSwitching.value = true
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
  const query = commandCenterQuery.value.replace(/^\//, '').trim().toLowerCase()
  return commandDefinitions.value.filter(command => command.key.includes(query) || command.label.toLowerCase().includes(query))
})

const allCommandItems = computed(() => items.value.flatMap(group => group.items || []))

const commandCenterItems = computed(() => {
  const query = commandCenterQuery.value.trim().toLowerCase()
  if (commandCenterQuery.value.startsWith('/') || !query) return []
  const seen = new Set<number>()
  return allCommandItems.value.filter((item) => {
    if (item.id !== undefined && seen.has(Number(item.id))) return false
    if (item.id !== undefined) seen.add(Number(item.id))
    return [item.title, item.url, item.description].some(value => value?.toLowerCase().includes(query))
  })
})

function openCommandCenter(query: string) {
  commandCenterQuery.value = query
  commandCenterSelectedIndex.value = query.startsWith('/') ? 0 : -1
  commandCenterVisible.value = true
}

function closeCommandCenter() {
  commandCenterVisible.value = false
  commandCenterQuery.value = ''
  commandCenterSelectedIndex.value = 0
}

function moveCommandSelection(offset: number) {
  const length = commandCenterQuery.value.startsWith('/') ? filteredCommandDefinitions.value.length : commandCenterItems.value.length
  if (!length) return
  commandCenterSelectedIndex.value = (commandCenterSelectedIndex.value + offset + length) % length
}

function selectCommandItem(index: number) {
  commandCenterSelectedIndex.value = index
}

function submitCommandCenterSearch(keyword: string) {
  window.open(replaceOrAppendKeywordToUrl(commandCenterSearchEngine.value.url, keyword))
  closeCommandCenter()
}

function findCommandItem(query: string) {
  const keyword = query.trim().toLowerCase()
  return allCommandItems.value.find(item => !keyword || [item.title, item.url, item.description].some(value => value?.toLowerCase().includes(keyword)))
}

function executeCommand(command: string) {
  const parts = commandCenterQuery.value.trim().replace(/^\//, '').split(/\s+/)
  const keyword = parts.slice(1).join(' ')
  if (command === 'add') handleAddItem()
  else if (command === 'group') groupCreateVisible.value = true
  else if (command === 'space') createSpaceVisible.value = true
  else if (command === 'settings') settingModalShow.value = true
  else if (command === 'top') scrollToTop()
  else {
    const item = findCommandItem(keyword)
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
  const title = groupName.value.trim()
  if (!title || !activeSpace.value || creatingGroup.value) return
  creatingGroup.value = true
  createGroup<{ code: number }>(activeSpace.value.id, title).then(({ code }) => {
    if (code === 0) {
      groupName.value = ''
      groupCreateVisible.value = false
      clearCachedSpace(activeSpace.value!.id)
      getList(true)
    }
  }).finally(() => { creatingGroup.value = false })
}

async function refreshCurrentSpace() {
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
  getSpaces<Space[]>().then(({ data }) => {
    const nextSpaces = sortSpaces(data || [], authStore.userInfo?.id)
    const targetSpaceId = selectLatest ? data?.[data.length - 1]?.id : activeSpace.value?.id
    spaces.value = nextSpaces
    activeSpace.value = nextSpaces.find(space => space.id === targetSpaceId) || nextSpaces[0] || null
    writeSpacesCache(spaces.value, authStore.userInfo?.id)
    getList()
  })
}
function handleSpacesChanged() {
  reloadSpaces()
}
function submitCreateSpace() {
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
    case 'delete':
      dialog.warning({
        title: t('common.warning'),
        content: t('common.deleteConfirmByName', { name: currentRightSelectItem.value?.title }),
        positiveText: t('common.confirm'),
        negativeText: t('common.cancel'),
        onPositiveClick: () => {
          deleteItem(activeSpace.value!.id, currentRightSelectItem.value?.id as number).then(({ code, msg }) => {
            if (code === 0) {
              ms.success(t('common.deleteSuccess'))
              clearCachedSpace(activeSpace.value!.id)
              getList(true)
            }
            else {
              ms.error(`${t('common.deleteFail')}:${msg}`)
            }
          })
        },
      })

      break
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

function handleEditSuccess(item: Panel.ItemInfo) {
  if (activeSpace.value) clearCachedSpace(activeSpace.value.id)
  getList(true)
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

function handleSaveSort(itemGroup: ItemGroup) {
  const saveItems: Common.SortItemRequest[] = []
  if (itemGroup.items) {
    for (let i = 0; i < itemGroup.items.length; i++) {
      const element = itemGroup.items[i]
      saveItems.push({
        id: element.id as number,
        sort: i + 1,
      })
    }

    sortItems(activeSpace.value!.id, itemGroup.id as number, saveItems).then(({ code, msg }) => {
      if (code === 0) {
        ms.success(t('common.saveSuccess'))
        itemGroup.sortStatus = false
        clearCachedSpace(activeSpace.value!.id)
        getList(true)
      }
      else {
        ms.error(`${t('common.saveFail')}:${msg}`)
      }
    })
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

  dropdownMenuOptions.push({
    label: t('common.edit'),
    key: 'edit',
  }, {
    label: t('common.delete'),
    key: 'delete',
  })
  return dropdownMenuOptions
}

async function loadHomeData() {
  homeReady.value = false
  const [monitorResult] = await Promise.all([
    getEnableStatus<{ enabled: boolean }>().catch(() => ({ code: -1, data: { enabled: false } })),
    panelState.updatePanelConfigByCloud(),
  ])
  if (monitorResult.code === 0) monitorEnabled.value = monitorResult.data.enabled

  if (panelState.panelConfig.logoText)
    setTitle(panelState.panelConfig.logoText)

  const useCachedSpaces = () => {
    const cached = readSpacesCache(authStore.userInfo?.id) as Space[]
    if (cached.length) {
      spaces.value = sortSpaces(cached, authStore.userInfo?.id)
      activeSpace.value = spaces.value[0]
    }
  }
  if (!navigator.onLine) {
    useCachedSpaces()
  }
  else {
    try {
      const { data } = await getSpaces<Space[]>()
      if (data?.length) {
        spaces.value = sortSpaces(data, authStore.userInfo?.id)
        writeSpacesCache(spaces.value, authStore.userInfo?.id)
        activeSpace.value = spaces.value[0]
      }
    }
    catch {
      useCachedSpaces()
    }
  }

  if (activeSpace.value) await getList()
  homeReady.value = true
}

onMounted(() => {
  window.addEventListener('keydown', handleGlobalKeydown)
  if (publicCode && !publicAccessReady.value) return
  loadHomeData()
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
  if (sideSwitchTimer) clearTimeout(sideSwitchTimer)
  groupLoadGeneration++
})

// 前端搜索过滤
function itemFrontEndSearch(keyword?: string) {
  keyword = keyword?.trim()
  if (keyword !== '' && panelState.panelConfig.searchBoxSearchIcon) {
    const filteredData = ref<ItemGroup[]>([])
    for (let i = 0; i < items.value.length; i++) {
      const element = items.value[i].items?.filter((item: Panel.ItemInfo) => {
        return (
          item.title.toLowerCase().includes(keyword?.toLowerCase() ?? '')
          || item.url.toLowerCase().includes(keyword?.toLowerCase() ?? '')
          || item.description?.toLowerCase().includes(keyword?.toLowerCase() ?? '')
        )
      })
      if (element && element.length > 0)
        filteredData.value.push({ items: element, hoverStatus: false })
    }
    filterItems.value = filteredData.value
  }
  else {
    filterItems.value = items.value
  }
}

function handleSetHoverStatus(groupIndex: number, hoverStatus: boolean) {
  if (items.value[groupIndex])
    items.value[groupIndex].hoverStatus = hoverStatus
}

function handleSetSortStatus(groupIndex: number, sortStatus: boolean) {
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
  editItemInfoData.value = item
  editItemInfoShow.value = true
  currentAddItenIconGroupId.value = undefined
}

function handleAddItem(itemIconGroupId?: number) {
  editItemInfoData.value = null
  editItemInfoShow.value = true
  if (itemIconGroupId)
    currentAddItenIconGroupId.value = itemIconGroupId
}
</script>

<template>
  <div class="w-full h-full sun-main" :class="{ 'side-switching': sideSwitching }">
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
    <div
      v-if="homeReady" class="cover wallpaper" :style="{
        filter: `blur(${panelState.panelConfig.backgroundBlur}px)`,
        background: `url(${panelState.panelConfig.backgroundImageSrc}) no-repeat`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
      }"
    />
    <div v-if="homeReady" class="mask" :style="{ backgroundColor: `rgba(0,0,0,${panelState.panelConfig.backgroundMaskNumber})` }" />
    <div v-if="homeReady" ref="scrollContainerRef" class="absolute w-full h-full overflow-auto">
      <div
        class="p-2.5 mx-auto"
        :style="{
          marginTop: `${panelState.panelConfig.marginTop}%`,
          marginBottom: `${panelState.panelConfig.marginBottom}%`,
          maxWidth: (panelState.panelConfig.maxWidth ?? '1200') + panelState.panelConfig.maxWidthUnit,
        }"
      >
        <!-- 头 -->
        <div class="home-header mx-[auto] w-[80%]">
          <div class="home-header-row flex mx-[auto] items-center justify-center text-white">
            <div class="logo cursor-pointer" data-lcp="brand" @click="togglePanelSide">
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
          <div v-if="panelState.panelConfig.searchBoxShow" class="flex mt-[20px] mx-auto sm:w-full lg:w-[80%]">
            <SearchBox :space-id="activeSpace?.id" @itemSearch="itemFrontEndSearch" @search-engine-change="commandCenterSearchEngine = $event" />
          </div>
        </div>

        <!-- 应用盒子 -->
        <div :style="{ marginLeft: `${panelState.panelConfig.marginX}px`, marginRight: `${panelState.panelConfig.marginX}px` }">
          <!-- 系统监控状态 -->
          <div v-if="monitorEnabled && panelState.panelConfig.systemMonitorShow" class="flex mx-auto">
            <SystemMonitor
              :show-title="panelState.panelConfig.systemMonitorShowTitle"
            />
          </div>

          <!-- 组纵向排列 -->
          <div
            v-for="(itemGroup, itemGroupIndex) in filterItems" :key="itemGroupIndex"
            v-show="!groupHidden(itemGroupIndex)"
            data-item-group data-testid="item-group"
            class="item-list mt-[50px] min-h-[110px]"
            :class="itemGroup.sortStatus ? 'shadow-2xl border shadow-[0_0_30px_10px_rgba(0,0,0,0.3)]  p-[10px] rounded-2xl' : ''"
            @mouseenter="handleSetHoverStatus(itemGroupIndex, true)"
            @mouseleave="handleSetHoverStatus(itemGroupIndex, false)"
          >
            <!-- 分组标题 -->
            <div class="text-white text-xl font-extrabold mb-[20px] flex items-center" :style="{ marginLeft: `${10 + (itemGroup.depth || 0) * 24}px` }">
              <span class="group-title text-shadow">
                {{ itemGroup.title }}
              </span>
              <span class="ml-2 cursor-pointer" :title="collapsedGroups.has(Number(itemGroup.id)) ? $t('spaceManage.expandGroup') : $t('spaceManage.collapseGroup')" @click="toggleGroup(Number(itemGroup.id))">
                <SvgIconOnline :icon="collapsedGroups.has(Number(itemGroup.id)) ? 'mdi:chevron-down' : 'mdi:chevron-up'" />
              </span>
              <div
                v-if="parsePublicCodeFromPath() === '' && authStore.token"
                class="group-buttons ml-2 delay-100 transition-opacity flex"
                :class="itemGroup.hoverStatus ? 'opacity-100' : 'opacity-0'"
              >
                <span class="mr-2 cursor-pointer" :title="t('common.add')" @click="handleAddItem(itemGroup.id)">
                  <SvgIcon class="text-white font-xl" icon="typcn:plus" />
                </span>
                <span class="mr-2 cursor-pointer " :title="t('common.sort')" @click="handleSetSortStatus(itemGroupIndex, !itemGroup.sortStatus)">
                  <SvgIcon class="text-white font-xl" icon="ri:drag-drop-line" />
                </span>
              </div>
            </div>

            <!-- 详情图标 -->
            <div v-if="panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info">
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
                      :icon-text-color="panelState.panelConfig.iconTextColor"
                      :icon-text-info-hide-description="panelState.panelConfig.iconTextInfoHideDescription || false"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="0"
                      @click="handleItemClick(itemGroupIndex, item)"
                    />
                  </div>

                  <div v-if="itemGroup.items.length === 0 && loadedGroups.has(Number(itemGroup.id))" class="not-drag">
                    <AppIcon
                      :class="itemGroup.sortStatus ? 'cursor-move' : 'cursor-pointer'"
                      :item-info="{ icon: { itemType: 3, text: 'subway:add' }, title: t('common.add'), url: '', openMethod: 0 }"
                      :icon-text-color="panelState.panelConfig.iconTextColor"
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
            <div v-if="panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.icon">
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
                      :icon-text-color="panelState.panelConfig.iconTextColor"
                      :icon-text-info-hide-description="!panelState.panelConfig.iconTextInfoHideDescription"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="1"
                      @click="handleItemClick(itemGroupIndex, item)"
                    />
                  </div>

                  <div v-if="itemGroup.items.length === 0 && loadedGroups.has(Number(itemGroup.id))" class="not-drag">
                    <AppIcon
                      class="cursor-pointer"
                      :item-info="{ icon: { itemType: 3, text: 'subway:add' }, title: $t('common.add'), url: '', openMethod: 0 }"
                      :icon-text-color="panelState.panelConfig.iconTextColor"
                      :icon-text-info-hide-description="!panelState.panelConfig.iconTextInfoHideDescription"
                      :icon-text-icon-hide-title="panelState.panelConfig.iconTextIconHideTitle || false"
                      :style="1"
                      @click="handleAddItem(itemGroup.id)"
                    />
                  </div>
                </vuedraggable>
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
    <div v-if="homeReady && parsePublicCodeFromPath() === '' && authStore.token" class="fixed-element shadow-[0_0_10px_2px_rgba(0,0,0,0.2)]">
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

    <NBackTop
      v-if="homeReady"
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

    <EditItem v-model:visible="editItemInfoShow" :item-info="editItemInfoData" :item-group-id="currentAddItenIconGroupId" :space-id="activeSpace?.id" @done="handleEditSuccess" />

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
          v-show="!windowIframeIsLoad" id="windowIframeId" ref="windowIframeRef" :src="windowSrc"
          class="w-full h-full" frameborder="0" @load="handWindowIframeIdLoad"
        />
      </div>
    </NModal>
  </div>
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
  padding: 3px;
  border: 1px solid rgba(255, 255, 255, 0.22);
  border-radius: 999px;
  background: rgba(18, 22, 28, 0.46);
  backdrop-filter: blur(14px);
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.18);
}
.space-status-button { color: white; min-width: 140px; }
.space-status-dot { width: 7px; height: 7px; margin-right: 8px; border-radius: 50%; background: #7dd3fc; box-shadow: 0 0 10px #7dd3fc; }
</style>

<style>
body,
html {
  overflow: hidden;
  background-color: rgb(54, 54, 54);
}
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

.sun-main.side-switching {
  animation: panel-content-pulse 1000ms ease-in-out;
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
  animation: taiji-aura 1000ms cubic-bezier(0.22, 0.61, 0.36, 1) both;
}

.taiji-symbol {
  --taiji-start-angle: 0deg;
  position: relative;
  z-index: 1;
  width: 58%;
  aspect-ratio: 1;
  overflow: hidden;
  border: 3px solid rgba(255, 255, 255, 0.82);
  border-radius: 50%;
  background: linear-gradient(90deg, #18212b 0 50%, #f4efe2 50%);
  box-shadow: 0 0 34px rgba(255, 255, 255, 0.3), 0 0 80px rgba(13, 18, 24, 0.3);
  animation: taiji-spin 1000ms cubic-bezier(0.22, 0.61, 0.36, 1) both;
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
  background: #f4efe2;
}

.taiji-symbol::after {
  bottom: 0;
  background: #18212b;
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
  background: #18212b;
}

.taiji-dot-light {
  bottom: 25%;
  left: 44%;
  background: #f4efe2;
}

.taiji-bagua {
  position: absolute;
  inset: 0;
  border: 1px solid rgba(255, 255, 255, 0.42);
  border-radius: 50%;
  animation: taiji-spin 1000ms cubic-bezier(0.22, 0.61, 0.36, 1) reverse both;
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

.text-shadow {
  text-shadow: 2px 2px 50px rgb(0, 0, 0);
}

.app-icon-text-shadow {
  text-shadow: 2px 2px 5px rgb(0, 0, 0);
}

.fixed-element {
  position: fixed;
  /* 将元素固定在屏幕上 */
  right: 10px;
  /* 距离屏幕顶部的距离 */
  bottom: 50px;
  /* 距离屏幕左侧的距离 */
}

.icon-info-box {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 18px;

}

.icon-small-box {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(75px, 1fr));
  gap: 18px;

}

@media (max-width: 500px) {
  .space-status-bar {
    top: 8px;
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
  .icon-small-box { gap: 12px 8px; grid-template-columns: repeat(auto-fill, minmax(70px, 1fr)); }
  .system-monitor { overflow: hidden; }
}
</style>
