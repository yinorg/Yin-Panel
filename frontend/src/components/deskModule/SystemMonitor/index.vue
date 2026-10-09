<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, provide, ref } from 'vue'
import { VueDraggable } from 'vue-draggable-plus'
import { NButton, NDropdown, useDialog, useMessage } from 'naive-ui'
import AppIconSystemMonitor from './AppIconSystemMonitor/index.vue'
import { type CardStyle, type MonitorData, MonitorType } from './typings'
import Edit from './Edit/index.vue'
import { deleteByIndex, getAll, saveAll } from './common'
import { usePanelState } from '../../../store'
import { PanelPanelConfigStyleEnum } from '../../../enums'
import { SvgIcon } from '../../common'
import { t } from '../../../locales'
import { monitorSnapshotKey, type MonitorSnapshot } from './snapshot'
import { resolvePanelValue } from '../../../utils/theme'
import type { MonitorSnapshotController } from '../../../core/monitor/snapshotController'

interface MonitorGroup extends Panel.ItemIconGroup {
  sortStatus?: boolean
  hoverStatus: boolean
  items?: Panel.ItemInfo[]
}

const props = defineProps<{
  allowEdit?: boolean
  showTitle?: boolean
  iconTextColor?: string
  snapshotController: MonitorSnapshotController<MonitorSnapshot>
}>()
const panelState = usePanelState()
const iconTextColor = computed(() => props.iconTextColor || resolvePanelValue('var(--yin-text)', panelState.panelConfig.iconTextColor, !!panelState.panelConfig.useThemeDefaults))

const dialog = useDialog()
const ms = useMessage()

const dropdownMenuX = ref(0)
const dropdownMenuY = ref(0)
const dropdownShow = ref(false)
const currentRightSelectIndex = ref<number | null>(null)

const monitorGroup = ref<MonitorGroup>({
  hoverStatus: false,
  sortStatus: false,
})

const editShowStatus = ref<boolean>(false)
const editData = ref<MonitorData | null>(null)
const editIndex = ref<number | null>(null)

function handleAddItem() {
  editShowStatus.value = true
  editData.value = null
  editIndex.value = null
}

function handleSetSortStatus(sortStatus: boolean) {
  monitorGroup.value.sortStatus = sortStatus

  // 并未保存排序重新更新数据
  if (!sortStatus)
    getData()
}

function handleSetHoverStatus(hoverStatus: boolean) {
  monitorGroup.value.hoverStatus = hoverStatus
}

const cardStyle: CardStyle = {
  background: 'var(--yin-component-app-icon-surface, #2a2a2a6b)',
}

const monitorDatas = ref<MonitorData[]>([])
const monitorSnapshot = ref<MonitorSnapshot | null>(null)
provide(monitorSnapshotKey, monitorSnapshot)
let unsubscribeSnapshot = () => {}

function handleClick(index: number, item: MonitorData) {
  if (!props.allowEdit)
    return
  editShowStatus.value = true
  editData.value = item
  editIndex.value = index
}

async function getData() {
  monitorDatas.value = await getAll()

  const defaultExtendParam = {
    backgroundColor: 'var(--yin-component-app-icon-surface, #2a2a2a6b)', color: 'var(--yin-text)', progressColor: 'var(--yin-primary)', progressRailColor: 'var(--yin-border)',
  }
  if (monitorDatas.value.length > 0) {
    const types = new Set(monitorDatas.value.map(item => item.monitorType))
    let changed = false
    for (const monitorType of [MonitorType.memory, MonitorType.network]) {
      if (!types.has(monitorType)) {
        monitorDatas.value.push({ monitorType, extendParam: { ...defaultExtendParam } })
        changed = true
      }
    }
    if (changed) saveAll(monitorDatas.value)
  }

  if (monitorDatas.value.length === 0) {
    // 防止空 - 默认数据
    monitorDatas.value.push(
      {
        extendParam: {
          backgroundColor: 'var(--yin-component-app-icon-surface, #2a2a2a6b)',
          color: 'var(--yin-text)',
          progressColor: 'var(--yin-primary)',
          progressRailColor: 'var(--yin-border)',
        },
        monitorType: MonitorType.cpu,
      },
      {
        extendParam: {
          backgroundColor: 'var(--yin-component-app-icon-surface, #2a2a2a6b)',
          color: 'var(--yin-text)',
          progressColor: 'var(--yin-primary)',
          progressRailColor: 'var(--yin-border)',
        },
        monitorType: MonitorType.memory,
      },
      {
        extendParam: {
          backgroundColor: 'var(--yin-component-app-icon-surface, #2a2a2a6b)',
          color: 'var(--yin-text)',
          progressColor: 'var(--yin-primary)',
          progressRailColor: 'var(--yin-border)',
        },
        monitorType: MonitorType.network,
      },
    )

    // 生成并保存
    saveAll(monitorDatas.value)
  }
  props.snapshotController.setDiskPaths(monitorDatas.value
    .filter(item => item.monitorType === MonitorType.disk)
    .map(item => (item.extendParam as { path?: string } | undefined)?.path || ''))
}

onMounted(() => {
  unsubscribeSnapshot = props.snapshotController.subscribe((snapshot) => { monitorSnapshot.value = snapshot })
  void getData().finally(() => props.snapshotController.start())
})

onUnmounted(() => {
  unsubscribeSnapshot()
  props.snapshotController.stop()
})

function handleSaveDone() {
  getData()
}

async function handleSaveSort() {
  const { code } = await saveAll(monitorDatas.value)
  if (code === 0)
    monitorGroup.value.sortStatus = false
}

function handleContextMenu(e: MouseEvent, index: number | null, item: MonitorData) {
  if (index !== null) {
    e.preventDefault()
    currentRightSelectIndex.value = index
  }

  nextTick().then(() => {
    dropdownShow.value = true
    dropdownMenuX.value = e.clientX
    dropdownMenuY.value = e.clientY
  })
}

function getDropdownMenuOptions() {
  const dropdownMenuOptions = [
    {
      label: t('common.delete'),
      key: 'delete',
    },
  ]

  return dropdownMenuOptions
}

function onClickoutside() {
  // message.info('clickoutside')
  dropdownShow.value = false
}

async function deleteOneByIndex(index: number) {
  const res = await deleteByIndex(index)
  if (res)
    getData()
}

function handleRightMenuSelect(key: string | number) {
  dropdownShow.value = false

  switch (key) {
    case 'delete':
      dialog.warning({
        title: t('common.warning'),
        content: t('common.deleteConfirm'),
        positiveText: t('common.confirm'),
        negativeText: t('common.cancel'),
        onPositiveClick: () => {
          if (monitorDatas.value.length <= 1) {
            ms.warning(t('common.leastOne'))
            return
          }
          if (currentRightSelectIndex.value !== null)
            deleteOneByIndex(currentRightSelectIndex.value)
        },
      })

      break
    default:
      break
  }
}
</script>

<template>
  <div class="system-monitor w-full">
    <div
      class="system-monitor-content"
      :class="{ 'system-monitor-content--sorting': monitorGroup.sortStatus }"
      @mouseenter="handleSetHoverStatus(true)"
      @mouseleave="handleSetHoverStatus(false)"
    >
      <!-- 分组标题 -->
      <div class="system-monitor-header flex items-center">
        <span v-if="showTitle" class="text-shadow">
          {{ $t('deskModule.systemMonitor.systemState') }}
        </span>
        <div
          v-if="allowEdit"
          class="system-monitor-buttons flex"
          :class="monitorGroup.hoverStatus ? 'opacity-100' : 'opacity-0'"
        >
          <span class="mr-2 cursor-pointer" @click="handleAddItem()">
            <SvgIcon class="system-monitor-action-icon" icon="typcn:plus" />
          </span>
          <span class="mr-2 cursor-pointer" @click="handleSetSortStatus(!monitorGroup.sortStatus)">
            <SvgIcon class="system-monitor-action-icon" icon="ri:drag-drop-line" />
          </span>
        </div>
      </div>

      <!-- 详情图标 -->
      <template v-if="panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info">
        <VueDraggable
          v-model="monitorDatas" item-key="sort" :animation="300"
          class="icon-info-box"
          filter=".not-drag"
          :disabled="!monitorGroup.sortStatus"
        >
          <div
            v-for="item, index in monitorDatas" :key="index"
            :title="item.description"
            @click="handleClick(index, item)"
            @contextmenu="(e) => handleContextMenu(e, index, item)"
          >
            <AppIconSystemMonitor
              :extend-param="item.extendParam"
              :icon-text-icon-hide-title="false"
              :card-type-style="panelState.panelConfig.iconStyle"
              :monitor-type="item.monitorType"
              :card-style="cardStyle"
              :icon-text-color="iconTextColor"
            />
          </div>
        </VueDraggable>
      </template>

      <!-- APP图标宫型盒子 -->
      <template v-if="panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.icon">
        <VueDraggable
          v-model="monitorDatas" item-key="sort" :animation="300"
          class="icon-small-box"
          filter=".not-drag"
          :disabled="!monitorGroup.sortStatus"
        >
          <div
            v-for="item, index in monitorDatas" :key="index"
            :title="item.description"
            @click="handleClick(index, item)"
            @contextmenu="(e) => handleContextMenu(e, index, item)"
          >
            <AppIconSystemMonitor
              :extend-param="item.extendParam"
              :icon-text-icon-hide-title="false"
              :card-type-style="panelState.panelConfig.iconStyle"
              :monitor-type="item.monitorType"
              :card-style="cardStyle"
              :icon-text-color="iconTextColor"
            />
          </div>
        </VueDraggable>
      </template>

      <!-- 编辑栏 -->
      <template v-if="monitorGroup.sortStatus && allowEdit">
        <div class="system-monitor-edit-bar flex mt-[10px]">
          <NButton class="theme-tool-button" @click="handleSaveSort()">
            <template #icon>
              <SvgIcon class="text-white font-xl" icon="material-symbols:save" />
            </template>
            <div>
              {{ $t('common.saveSort') }}
            </div>
          </NButton>
        </div>
      </template>
    </div>

    <Edit v-model:visible="editShowStatus" :monitor-data="editData" :index="editIndex" @done="handleSaveDone" />

    <NDropdown
      placement="bottom-start" trigger="manual" :x="dropdownMenuX" :y="dropdownMenuY"
      :options="getDropdownMenuOptions()" :show="dropdownShow" :on-clickoutside="onClickoutside" @select="handleRightMenuSelect"
    />
  </div>
</template>

<style scoped>
.text-shadow {
  text-shadow: var(--yin-effect-text-shadow);
}

.app-icon-text-shadow {
  text-shadow: var(--yin-effect-text-shadow);
}

.system-monitor-content { margin-top: var(--yin-component-system-monitor-gap); border-radius: var(--yin-component-system-monitor-radius); transition: box-shadow var(--yin-component-state-hover-duration) var(--yin-component-state-easing), border-color var(--yin-component-state-hover-duration) var(--yin-component-state-easing), padding var(--yin-component-state-hover-duration) var(--yin-component-state-easing); }
.system-monitor-content--sorting { padding: var(--yin-component-system-monitor-padding); border: var(--yin-component-card-border-width) var(--yin-component-card-border-style) var(--yin-border); box-shadow: var(--yin-component-card-shadow); }
.system-monitor-header { margin: 0 0 var(--yin-component-system-monitor-gap) var(--yin-spaceSm); color: var(--yin-text); font-family: var(--yin-fontDisplay); font-size: var(--yin-component-system-monitor-heading-size); font-weight: var(--yin-component-system-monitor-heading-weight); }
.system-monitor-buttons { margin-left: var(--yin-spaceSm); transition: opacity var(--yin-component-state-hover-duration) var(--yin-component-state-easing); }
.system-monitor-buttons > span { margin-right: var(--yin-spaceSm); color: var(--yin-text); cursor: pointer; }
.system-monitor-action-icon { font-size: 20px; }

.icon-info-box {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: var(--yin-component-system-monitor-gap);

}

.icon-small-box {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(75px, 1fr));
	gap: var(--yin-component-system-monitor-gap);

}

@media (max-width: 500px) {
  .icon-info-box{
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  }
}
</style>
