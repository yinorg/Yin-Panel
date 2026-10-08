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
}

defineProps<Prop>()
const memoryState = ref<SystemMonitor.MemoryInfo | null>(null)
const snapshot = inject(monitorSnapshotKey)
watch(() => snapshot?.value?.MEMORY_INFO, value => { if (value) memoryState.value = value }, { immediate: true })

function formatMemorySize(v: number): string {
  return bytesToSize(v)
}

</script>

<template>
  <GenericProgress
    :progress-color="progressColor"
    :progress-rail-color="progressRailColor"
    :progress-height="5"
    :percentage="correctionNumberByCardStyle(memoryState?.usedPercent || 0, cardTypeStyle)"
    :card-type-style="cardTypeStyle"
    :info-card-right-text="`${formatMemorySize(memoryState?.used || 0)}/${formatMemorySize(memoryState?.total || 0)}`"
    info-card-left-text="RAM"
    :text-color="textColor"
  />
</template>
