import type { AppContext } from 'vue'
import type { MonitorSnapshotController } from '@/core/monitor/snapshotController'
import type { MonitorSnapshot } from '@/components/deskModule/SystemMonitor/snapshot'

/**
 * The Core-owned data behind the `<yin-system-monitor>` component bridge.
 *
 * A theme runs as plain JavaScript, so it cannot mount a Vue component directly.
 * The Core registers the monitor as a custom element instead; the element reads the
 * live controller and the display config from here and mounts the *same* component
 * the Core overlay uses. Because the theme and the Core share one document (light
 * DOM), the element inherits the theme's tokens and CSS, which is what keeps the
 * embedded monitor pixel-identical to the Core's own rendering.
 */
export interface ThemeMonitorBridge {
  controller: MonitorSnapshotController<MonitorSnapshot>
  showTitle: boolean
  iconTextColor: string
  appContext: AppContext
}

let bridge: ThemeMonitorBridge | undefined

export function setThemeMonitorBridge(next: ThemeMonitorBridge | undefined) {
  bridge = next
}

export function getThemeMonitorBridge() {
  return bridge
}

/**
 * Share one poller between the Core overlay and any theme-embedded element.
 *
 * `start`/`stop` are not refcounted on the raw controller, so two consumers would
 * stop each other's polling. This wrapper starts on the first consumer and stops on
 * the last.
 */
export function createSharedMonitorController(controller: MonitorSnapshotController<MonitorSnapshot>): MonitorSnapshotController<MonitorSnapshot> {
  let consumers = 0
  return {
    fetchSnapshot: controller.fetchSnapshot,
    setDiskPaths: controller.setDiskPaths,
    subscribe: controller.subscribe,
    start() {
      if (consumers === 0)
        controller.start()
      consumers += 1
    },
    stop() {
      if (consumers === 0)
        return
      consumers -= 1
      if (consumers === 0)
        controller.stop()
    },
  }
}
