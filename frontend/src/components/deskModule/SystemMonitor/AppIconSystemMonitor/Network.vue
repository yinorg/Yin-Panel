<script setup lang="ts">
import { inject, onMounted, onUnmounted, ref, watch } from 'vue'
import { bytesToSize } from '../../../../utils/cmn'
import { monitorSnapshotKey } from '../snapshot'

const props = defineProps<{ refreshInterval: number; textColor: string; progressColor: string; progressRailColor: string }>()
const value = ref('0 B/s')
let previous = 0
const snapshot = inject(monitorSnapshotKey)
watch(() => snapshot?.value?.NETWORK_INFO, updateValue, { immediate: true })
let timer: ReturnType<typeof setInterval>

async function getData() {
  if (snapshot) return
  if (document.hidden) return
}
function updateValue(data?: { bytesRecv: number; bytesSent: number }[]) {
  if (!data) return
  const total = data.reduce((sum, item) => sum + item.bytesRecv + item.bytesSent, 0)
  if (previous) value.value = `${bytesToSize(Math.max(0, total - previous) * 1000 / 3000)}/s`
  previous = total
}
onMounted(() => { getData(); timer = setInterval(getData, props.refreshInterval || 5000) })
onUnmounted(() => clearInterval(timer))
</script>
<template><div class="text-sm" :style="{ color: textColor }">NET {{ value }}</div></template>
