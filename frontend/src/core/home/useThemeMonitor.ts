import { computed, nextTick, onUnmounted, ref, watch } from 'vue'
import { getDiskStateByPath, getSnapshot } from '@/api/system/systemMonitor'
import { usePanelState } from '@/store'
import { createMonitorSnapshotController } from '@/core/monitor/snapshotController'
import { normalizeMonitorSnapshot } from '@/core/monitor/themeSnapshot'
import type { CoreMonitorSnapshot } from '@/core/monitor/themeSnapshot'

/**
 * The Core-owned system monitor band that overlays the theme frame, plus the
 * geometry the theme needs in order to leave room for it.
 *
 * Two numbers cross the boundary here, in opposite directions:
 *
 * - `reservedHeight` goes *to* the theme, measured from the real layer box, so the
 *   theme can push its own content below the band;
 * - the theme's search-box bottom comes *back* through `reportSearchBottom`, since
 *   the sandbox is a distinct origin and the Core cannot measure inside it. The
 *   layer follows that report, and only falls back to the stylesheet formula until
 *   the first one arrives.
 */
export function useThemeMonitor(input: {
  /** Whether the theme is actually mounted; the band is meaningless otherwise. */
  isThemeActive: () => boolean
}) {
  const panelState = usePanelState()

  const monitorEnabled = ref(false)
  const monitorResultRefreshInterval = ref<number | undefined>()
  /** Reported by the data composable's bootstrap rather than fetched here. */
  function applyMonitorStatus(status: { enabled: boolean, refreshInterval?: number }) {
    monitorEnabled.value = status.enabled
    monitorResultRefreshInterval.value = status.refreshInterval
  }

  const monitorSnapshotController = createMonitorSnapshotController<CoreMonitorSnapshot, SystemMonitor.DiskInfo>({
    fetchSnapshot: async () => {
      const result = await getSnapshot<CoreMonitorSnapshot>()
      if (result.code !== 0) throw Object.assign(new Error('Monitor data is unavailable'), { code: 'UNSUPPORTED_CAPABILITY' })
      return result.data
    },
    fetchDisk: path => getDiskStateByPath<SystemMonitor.DiskInfo>(path),
    getInterval: async () => Math.max(250, (monitorResultRefreshInterval.value || 10) * 1000),
  })

  const layerRef = ref<HTMLElement>()
  const reservedHeight = ref(0)
  const reportedSearchBottom = ref<number | null>(null)
  const MONITOR_GAP = 24
  const layerTop = computed(() => reportedSearchBottom.value === null
    ? undefined
    : `${Math.round(reportedSearchBottom.value + MONITOR_GAP)}px`)

  let resizeObserver: ResizeObserver | undefined
  let measureFrame = 0
  function measureReservation() {
    if (measureFrame) cancelAnimationFrame(measureFrame)
    measureFrame = requestAnimationFrame(() => {
      measureFrame = 0
      const element = layerRef.value
      // The layer is absolutely positioned inside the scrolling shell, so its
      // offset box is stable across scrolls while a viewport-relative rect is not.
      const nextHeight = input.isThemeActive() && monitorEnabled.value && panelState.panelConfig.systemMonitorShow && element?.isConnected
        ? Math.min(window.innerHeight, Math.max(0, Math.ceil(element.offsetTop + element.offsetHeight)))
        : 0
      if (reservedHeight.value !== nextHeight)
        reservedHeight.value = nextHeight
    })
  }

/** Function ref: the host binds this to the layer element, so the element is
 *  never imported into the host's own scope. */
  function setLayerElement(element: Element | { $el?: unknown } | null) {
    layerRef.value = element instanceof HTMLElement ? element : undefined
  }

  function reportSearchBottom(searchBottom: number) {
    if (reportedSearchBottom.value === searchBottom) return
    reportedSearchBottom.value = searchBottom
    measureReservation()
  }

  watch([() => input.isThemeActive(), monitorEnabled, () => panelState.panelConfig.systemMonitorShow, layerRef], async ([active, enabled, visible]) => {
    resizeObserver?.disconnect()
    resizeObserver = undefined
    await nextTick()
    if (!active || !enabled || !visible) {
      measureReservation()
      return
    }
    const element = layerRef.value
    if (!element) {
      measureReservation()
      return
    }
    if (typeof ResizeObserver !== 'undefined') {
      resizeObserver = new ResizeObserver(measureReservation)
      resizeObserver.observe(element)
    }
    measureReservation()
  }, { flush: 'post', immediate: true })

  onUnmounted(() => {
    resizeObserver?.disconnect()
    if (measureFrame) cancelAnimationFrame(measureFrame)
  })

  return {
    monitorEnabled,
    monitorResultRefreshInterval,
    applyMonitorStatus,
    monitorSnapshotController,
    setLayerElement,
    reservedHeight,
    layerTop,
    measureReservation,
    reportSearchBottom,
    /** The Core hands the theme a plain-structure monitor snapshot. */
    getMonitorSnapshot: async () => normalizeMonitorSnapshot(await monitorSnapshotController.fetchSnapshot()),
  }
}
