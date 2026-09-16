<script setup lang="ts">
import { VueDraggable } from 'vue-draggable-plus'
import { NBackTop, NButton, NButtonGroup, NCard, NDropdown, NInput, NModal, NSkeleton, NSpin, NSpace, useDialog, useMessage } from 'naive-ui'
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { createSpace, getGroups, getItems, getSpaces, sortSpaces, spaceDisplayName, type Space } from '../../api/panel/space'
import { Clock, SearchBox, SystemMonitor } from '../../components/deskModule'
import { SvgIcon, SvgIconOnline } from '../../components/common'
import { AppIcon, AppStarter, EditItem } from './components'
import { deleteItem, sortItems } from '@/api/panel/space'

import { setTitle } from '@/utils/cmn'
import { parsePublicCodeFromPath } from '@/utils/request/axios'
import { usePanelState, useAuthStore } from '@/store'
import { PanelPanelConfigStyleEnum, PanelStateNetworkModeEnum } from '@/enums'
import { t } from '@/locales'
import { getEnableStatus } from '@/api/system/systemMonitor'

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
const createSpaceVisible = ref(false)
const spaceName = ref('')
const creatingSpace = ref(false)
const monitorEnabled = ref(false)

const items = ref<ItemGroup[]>([])
const filterItems = ref<ItemGroup[]>([])
const loadedGroups = new Set<number>()
const loadingGroups = new Set<number>()
const groupLoadQueue: number[] = []
let activeGroupLoads = 0
const collapsedGroups = ref<Set<number>>(new Set())
let groupObserver: IntersectionObserver | null = null
const publicCode = parsePublicCodeFromPath()
const publicAccessCode = ref('')
const publicAccessReady = ref(!publicCode || !!sessionStorage.getItem(`yin-panel-public-access:${publicCode}`))

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

function handleItemClick(itemGroupIndex: number, item: Panel.ItemInfo) {
  if (items.value[itemGroupIndex] && items.value[itemGroupIndex].sortStatus) {
    handleEditItem(item)
    return
  }

  let jumpUrl = ''

  if (item)
    jumpUrl = (panelState.networkMode === PanelStateNetworkModeEnum.lan ? item.lanUrl : item.url) as string
  if (item.lanUrl === '')
    jumpUrl = item.url

  openPage(item.openMethod, jumpUrl, item.title)
}

function handWindowIframeIdLoad(payload: Event) {
  windowIframeIsLoad.value = false
}

function scrollToTop() {
  scrollContainerRef.value?.scrollTo({ top: 0, behavior: 'smooth' })
}

// 获取组数据
function getList() {
  groupObserver?.disconnect()
  groupObserver = null
  loadedGroups.clear()
  loadingGroups.clear()
  groupLoadQueue.length = 0
  if (!activeSpace.value) {
    items.value = []
    filterItems.value = []
    return
  }

  getGroups<{ code: number; data: ItemGroup[] }>(activeSpace.value.id).then(({ code, data }) => {
    if (code !== 0 || !data) return
    const byParent = new Map<number, ItemGroup[]>()
    data.forEach(group => { const key = group.parentId || 0; const list = byParent.get(key) || []; list.push({ ...group, items: [] }); byParent.set(key, list) })
    const flattened: ItemGroup[] = []
    function append(parentId: number, depth: number) { (byParent.get(parentId) || []).forEach(group => { group.depth = depth; flattened.push(group); append(Number(group.id), depth + 1) }) }
    append(0, 0)
    items.value = flattened
    const initiallyCollapsed = new Set<number>()
    flattened.forEach(group => { if ((byParent.get(Number(group.id)) || []).length) initiallyCollapsed.add(Number(group.id)) })
    collapsedGroups.value = initiallyCollapsed
    filterItems.value = items.value
    nextTick(() => { document.querySelectorAll('[data-item-group]').forEach((el, index) => observeGroup(el, index)) })
  })
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

function loadGroupItems(index: number) {
  const group = items.value[index]
  if (!group || !activeSpace.value || loadedGroups.has(Number(group.id)) || loadingGroups.has(Number(group.id))) return
  loadingGroups.add(Number(group.id))
  groupLoadQueue.push(index)
  processGroupLoadQueue()
}
function processGroupLoadQueue() {
  if (activeGroupLoads >= 2 || groupLoadQueue.length === 0) return
  const index = groupLoadQueue.shift()!
  const group = items.value[index]
  if (!group || !activeSpace.value) { processGroupLoadQueue(); return }
  activeGroupLoads++
  const groupId = Number(group.id)
  getItems<{ code: number; data: Panel.ItemInfo[] }>(activeSpace.value.id, groupId, 1, 100).then(res => { if (res.code === 0) { group.items = res.data || []; if (group.items.length > 40) collapsedGroups.value = new Set([...collapsedGroups.value, groupId]) } }).finally(() => { loadedGroups.add(groupId); loadingGroups.delete(groupId); activeGroupLoads--; processGroupLoadQueue() })
}
function toggleGroup(id: number) { const next = new Set(collapsedGroups.value); next.has(id) ? next.delete(id) : next.add(id); collapsedGroups.value = next }
function observeGroup(el: Element, index: number) {
  if (!groupObserver) groupObserver = new IntersectionObserver(entries => entries.forEach(entry => { if (entry.isIntersecting) loadGroupItems(Number((entry.target as HTMLElement).dataset.groupIndex)) }), { rootMargin: '100px' })
  ;(el as HTMLElement).dataset.groupIndex = String(index)
  groupObserver.observe(el)
}

function selectSpace(key: string | number) {
  loadedGroups.clear()
  const selected = spaces.value.find(space => space.id === Number(key))
  if (selected) { activeSpace.value = selected; getList() }
}

function reloadSpaces(selectLatest = false) {
  getSpaces<{ code: number; data: Space[] }>().then(({ code, data }) => {
    if (code !== 0 || !data?.length) return
    spaces.value = sortSpaces(data, authStore.userInfo?.id)
    if (selectLatest) activeSpace.value = data[data.length - 1]
    getList()
  })
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
  getItems<{ code: number; data: Panel.ItemInfo[] }>(activeSpace.value!.id, itemIconGroupId).then((res) => {
    if (res.code === 0)
      items.value[itemIconGroupIndex].items = res.data
  })
}

function handleRightMenuSelect(key: string | number) {
  dropdownShow.value = false
  // console.log(currentRightSelectItem, key)
  let jumpUrl = panelState.networkMode === PanelStateNetworkModeEnum.lan ? currentRightSelectItem.value?.lanUrl : currentRightSelectItem.value?.url
  if (currentRightSelectItem.value?.lanUrl === '')
    jumpUrl = currentRightSelectItem.value.url
  switch (key) {
    case 'newWindows':
      window.open(jumpUrl)
      break
    case 'openWanUrl':
      if (currentRightSelectItem.value)
        openPage(currentRightSelectItem.value?.openMethod, currentRightSelectItem.value?.url, currentRightSelectItem.value?.title)
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
              getList()
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
  getList()
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

function loadHomeData() {
  getEnableStatus<{ enabled: boolean }>().then(({ code, data }) => {
    if (code === 0) monitorEnabled.value = data.enabled
  })
  getSpaces<{ code: number; data: Space[] }>().then(({ code, data }) => {
    if (code === 0 && data?.length) { spaces.value = sortSpaces(data, authStore.userInfo?.id); activeSpace.value = spaces.value[0]; getList() }
  })

  // 更新同步云端配置
  panelState.updatePanelConfigByCloud()

  // 设置标题
  if (panelState.panelConfig.logoText)
    setTitle(panelState.panelConfig.logoText)
}

onMounted(() => {
  if (publicCode && !publicAccessReady.value) return
  loadHomeData()
})

onUnmounted(() => {
  groupObserver?.disconnect()
  groupObserver = null
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
  <div class="w-full h-full sun-main">
    <NModal :show="!!publicCode && !publicAccessReady" :mask-closable="false" :closable="false">
      <NCard :title="$t('spaceManage.accessVerification')" style="width: min(92vw, 380px)">
        <NSpace vertical>
          <span>{{ $t('spaceManage.enterAccessCode') }}</span>
          <NInput v-model:value="publicAccessCode" type="password" show-password-on="click" maxlength="12" :placeholder="$t('spaceManage.accessCodeShortPlaceholder')" @keyup.enter="unlockPublicAccess" />
          <NButton type="primary" block @click="unlockPublicAccess">{{ $t('spaceManage.confirmAccess') }}</NButton>
        </NSpace>
      </NCard>
    </NModal>
    <div v-if="spaces.length && authStore.token" class="space-status-bar">
      <NDropdown trigger="hover" :options="spaces.map(space => ({ label: spaceDisplayName(space, spaces, authStore.userInfo?.id), key: space.id }))" @select="selectSpace">
        <NButton quaternary class="space-status-button">
          <span class="space-status-dot" />
          {{ activeSpace ? spaceDisplayName(activeSpace, spaces, authStore.userInfo?.id) : '' }}
          <span class="ml-2 opacity-60">⌄</span>
        </NButton>
      </NDropdown>
    </div>
    <div
      class="cover wallpaper" :style="{
        filter: `blur(${panelState.panelConfig.backgroundBlur}px)`,
        background: `url(${panelState.panelConfig.backgroundImageSrc}) no-repeat`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
      }"
    />
    <div class="mask" :style="{ backgroundColor: `rgba(0,0,0,${panelState.panelConfig.backgroundMaskNumber})` }" />
    <div ref="scrollContainerRef" class="absolute w-full h-full overflow-auto">
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
            <div class="logo">
              <span class="text-2xl md:text-6xl font-bold text-shadow">
                {{ panelState.panelConfig.logoText }}
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
            <SearchBox @itemSearch="itemFrontEndSearch" />
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
            data-item-group
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

                  <div v-if="itemGroup.items.length === 0" class="not-drag">
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

                  <div v-if="itemGroup.items.length === 0" class="not-drag">
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
      v-if="parsePublicCodeFromPath() === '' && authStore.token"
      placement="bottom-start" trigger="manual" :x="dropdownMenuX" :y="dropdownMenuY"
      :options="getDropdownMenuOptions()" :show="dropdownShow" :on-clickoutside="onClickoutside" @select="handleRightMenuSelect"
    />

    <!-- 悬浮按钮 -->
    <div v-if="parsePublicCodeFromPath() === '' && authStore.token" class="fixed-element shadow-[0_0_10px_2px_rgba(0,0,0,0.2)]">
      <NButtonGroup vertical>
        <NButton color="#2a2a2a6b" :title="$t('spaceManage.backToTop')" @click="scrollToTop">
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="icon-park-outline:to-top" />
          </template>
        </NButton>
        <!-- 网络模式切换按钮组 -->
        <NButton
          v-if="panelState.networkMode === PanelStateNetworkModeEnum.lan && panelState.panelConfig.netModeChangeButtonShow" color="#2a2a2a6b"
          :title="t('panelHome.changeToWanModel')" @click="handleChangeNetwork(PanelStateNetworkModeEnum.wan)"
        >
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="material-symbols:lan-outline-rounded" />
          </template>
        </NButton>

        <NButton
          v-if="panelState.networkMode === PanelStateNetworkModeEnum.wan && panelState.panelConfig.netModeChangeButtonShow" color="#2a2a2a6b"
          :title="t('panelHome.changeToLanModel')" @click="handleChangeNetwork(PanelStateNetworkModeEnum.lan)"
        >
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="mdi:wan" />
          </template>
        </NButton>

        <NButton color="#2a2a2a6b" @click="settingModalShow = !settingModalShow">
          <template #icon>
            <SvgIcon class="text-white font-xl" icon="majesticons-applications" />
          </template>
        </NButton>
      </NButtonGroup>

      <AppStarter v-model:visible="settingModalShow" />
    </div>

    <NBackTop
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
    <NInput v-model:value="spaceName" :placeholder="$t('spaceManage.newSpaceName')" maxlength="100" show-count @keyup.enter="submitCreateSpace" />
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
