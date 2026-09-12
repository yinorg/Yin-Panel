<script setup lang="ts">
import { inject, onMounted, onUnmounted, ref, watch } from 'vue'
import GenericProgress from '../components/GenericProgress/index.vue'
import { correctionNumber, correctionNumberByCardStyle } from './common'
import type { PanelPanelConfigStyleEnum } from '../../../../enums'
import { monitorSnapshotKey } from '../snapshot'

interface Prop {
  cardTypeStyle: PanelPanelConfigStyleEnum
  refreshInterval: number
  textColor: string
  progressColor: string
  progressRailColor: string
}

const props = defineProps<Prop>()
let timer: ReturnType<typeof setInterval>
const cpuState = ref<SystemMonitor.CPUInfo | null>(null)
const snapshot = inject(monitorSnapshotKey)
watch(() => snapshot?.value?.CPU_INFO, value => { if (value) cpuState.value = value }, { immediate: true })

async function getData() {
  if (snapshot) return
  if (document.hidden) return
  try {
  }
  catch (error) {

  }
}

onMounted(() => {
  getData()
  timer = setInterval(() => {
    getData()
  }, (!props.refreshInterval || props.refreshInterval <= 2000) ? 2000 : props.refreshInterval)
})

onUnmounted(() => {
  clearInterval(timer)
})
</script>

<template>
  <GenericProgress
    :progress-color="progressColor"
    :progress-rail-color="progressRailColor"
    :progress-height="5"
    :percentage="correctionNumberByCardStyle(cpuState?.usages[0] || 0, cardTypeStyle)"
    :card-type-style="cardTypeStyle"
    :info-card-right-text="`${correctionNumber(cpuState?.usages[0] || 0)}%`"
    info-card-left-text="CPU"
    :text-color="textColor"
    style="width: 100%;"
  />
</template>
