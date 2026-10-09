import { getThemeMonitorBridge } from '@/core/home/monitorBridge'
import { registerCoreElement } from './registry'

/**
 * `<yin-system-monitor>` — the Core component bridge for the system monitor.
 *
 * The element mounts the exact component the Core overlay uses, into the theme's
 * own DOM, so it inherits the theme's tokens and CSS and renders identically. The
 * data and the display config come from the Core through `getThemeMonitorBridge`.
 *
 * Attributes:
 * - `show-title="true|false"` overrides the Core's "show title" preference.
 * - `icon-text-color="<css color>"` overrides the Core's icon text colour.
 */
registerCoreElement('yin-system-monitor', () => import('@/components/deskModule/SystemMonitor/index.vue'), (element) => {
  const bridge = getThemeMonitorBridge()
  if (!bridge)
    return null
  const showTitle = element.hasAttribute('show-title')
    ? element.getAttribute('show-title') !== 'false'
    : bridge.showTitle
  return {
    allowEdit: false,
    showTitle,
    iconTextColor: element.getAttribute('icon-text-color') || bridge.iconTextColor,
    snapshotController: bridge.controller,
  }
})
