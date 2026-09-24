<script setup lang="ts">
import { inject, ref, watch } from 'vue'
import GenericProgress from '../components/GenericProgress/index.vue'
import { correctionNumberByCardStyle } from './common'
import type { PanelPanelConfigStyleEnum } from '../../../../enums'
import { bytesToSize } from '../../../../utils/cmn'
import { monitorSnapshotKey } from '../snapshot'

interface Prop {
  cardTypeStyle: PanelPanelConfigStyleEnum
  refreshInterval: number
  textColor: string
  progressColor: string
  progressRailColor: string
  path: string
}

const props = defineProps<Prop>()
const diskState = ref<SystemMonitor.DiskInfo | null>(null)
const snapshot = inject(monitorSnapshotKey)

watch(
  () => snapshot?.value?.DISK_INFO?.[props.path],
  (value) => {
    if (value) diskState.value = value
  },
  { immediate: true },
)

function formatdiskSize(v: number): string {
  return bytesToSize(v)
}

function formatdiskToByte(v: number): number {
  return v
}

</script>

<template>
  <GenericProgress
    :progress-color="progressColor"
    :progress-rail-color="progressRailColor"
    :progress-height="5"
    :percentage="correctionNumberByCardStyle(diskState?.usedPercent || 0, cardTypeStyle)"
    :card-type-style="cardTypeStyle"
    :info-card-right-text="`${formatdiskSize(formatdiskToByte(diskState?.used || 0))}/${formatdiskSize(formatdiskToByte(diskState?.free || 0))}`"
    :info-card-left-text="diskState?.mountpoint"
    :text-color="textColor"
  />
</template>
