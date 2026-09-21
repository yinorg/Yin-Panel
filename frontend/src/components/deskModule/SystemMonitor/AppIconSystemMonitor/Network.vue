<script setup lang="ts">
import { inject, onMounted, onUnmounted, ref, watch } from 'vue'
import { bytesToSize } from '../../../../utils/cmn'
import { monitorSnapshotKey } from '../snapshot'
import SvgIcon from '../../../common/SvgIcon/index.vue'

const props = defineProps<{ refreshInterval: number; textColor: string; progressColor: string; progressRailColor: string; compact?: boolean }>()
const uploadSpeed = ref('0 B/s')
const downloadSpeed = ref('0 B/s')
let previousSent = 0
let previousRecv = 0
let previousAt = 0
const snapshot = inject(monitorSnapshotKey)
watch(() => snapshot?.value?.NETWORK_INFO, updateValue, { immediate: true })
let timer: ReturnType<typeof setInterval>

async function getData() {
  if (snapshot) return
  if (document.hidden) return
}
function updateValue(data?: { bytesRecv: number; bytesSent: number }[]) {
  if (!data) return
  const bytesSent = data.reduce((sum, item) => sum + item.bytesSent, 0)
  const bytesRecv = data.reduce((sum, item) => sum + item.bytesRecv, 0)
  const now = Date.now()
  if (previousAt) {
    const elapsed = Math.max((now - previousAt) / 1000, 0.001)
    uploadSpeed.value = `${bytesToSize(Math.max(0, bytesSent - previousSent) / elapsed)}/s`
    downloadSpeed.value = `${bytesToSize(Math.max(0, bytesRecv - previousRecv) / elapsed)}/s`
  }
  previousSent = bytesSent
  previousRecv = bytesRecv
  previousAt = now
}
onMounted(() => { getData(); timer = setInterval(getData, props.refreshInterval || 5000) })
onUnmounted(() => clearInterval(timer))
</script>
<template>
  <div class="network-speed" :class="{ 'network-speed--compact': compact }" :style="{ color: textColor }" aria-label="Network speed">
    <template v-if="compact">
      <SvgIcon class="network-speed-icon" icon="material-symbols:lan-outline-rounded" aria-hidden="true" />
      <div class="network-speed-metrics">
        <div class="network-speed-row" :title="`Upload ${uploadSpeed}`">
          <SvgIcon class="network-speed-direction network-speed-direction--upload" icon="icon-park-outline-to-top" aria-hidden="true" />
          <span class="network-speed-value">{{ uploadSpeed }}</span>
        </div>
        <div class="network-speed-row" :title="`Download ${downloadSpeed}`">
          <SvgIcon class="network-speed-direction network-speed-direction--download" icon="icon-park-outline-to-top" aria-hidden="true" />
          <span class="network-speed-value">{{ downloadSpeed }}</span>
        </div>
      </div>
    </template>
    <template v-else>
      <div class="network-speed-row" :title="`Upload ${uploadSpeed}`">
        <span class="network-speed-direction" aria-hidden="true">↑</span>
        <span class="network-speed-value">{{ uploadSpeed }}</span>
      </div>
      <div class="network-speed-row" :title="`Download ${downloadSpeed}`">
        <span class="network-speed-direction" aria-hidden="true">↓</span>
        <span class="network-speed-value">{{ downloadSpeed }}</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.network-speed {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 100%;
  min-width: 0;
  max-width: 100%;
  font-size: 12px;
  line-height: 1.25;
}

.network-speed-row {
  display: flex;
  align-items: center;
  width: 100%;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
}

.network-speed-direction {
  flex: 0 0 14px;
  text-align: center;
}

.network-speed-value {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.network-speed--compact {
  box-sizing: border-box;
  justify-content: center;
  align-items: center;
  gap: 4px;
  width: 70px;
  height: 70px;
  padding: 7px 5px 6px;
  font-size: 10px;
  line-height: 12px;
}

.network-speed-icon {
  flex: 0 0 22px;
  width: 22px;
  height: 22px;
  color: currentColor;
}

.network-speed-metrics {
  display: grid;
  gap: 2px;
  width: 100%;
  min-width: 0;
}

.network-speed--compact .network-speed-row {
  display: grid;
  height: 12px;
  grid-template-columns: 13px minmax(0, 1fr);
  justify-content: initial;
  gap: 2px;
}

.network-speed--compact .network-speed-direction {
  width: 13px;
  height: 13px;
  flex: 0 0 13px;
}

.network-speed-direction--upload {
  color: #8fe388;
}

.network-speed-direction--download {
  color: #8ec5ff;
  transform: rotate(180deg);
}
</style>
