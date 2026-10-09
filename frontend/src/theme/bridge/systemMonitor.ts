import { createVNode, defineAsyncComponent, render, type VNode } from 'vue'
import { NDialogProvider, NLoadingBarProvider, NMessageProvider, NNotificationProvider } from 'naive-ui'
import { getThemeMonitorBridge } from '@/core/home/monitorBridge'

// Loaded on demand like the Core overlay does, so the monitor stays out of the
// initial bundle.
const SystemMonitor = defineAsyncComponent(() => import('@/components/deskModule/SystemMonitor/index.vue'))

/**
 * `<yin-system-monitor>` — the Core component bridge for the system monitor.
 *
 * A theme (plain JavaScript) cannot mount a Vue component, so the Core registers
 * the monitor as a custom element. The element mounts the exact component the Core
 * overlay uses, into the theme's own DOM, so it inherits the theme's tokens and CSS
 * and renders identically. The data and the display config come from the Core
 * through `getThemeMonitorBridge`; the element holds no state of its own.
 *
 * Attributes:
 * - `show-title="true|false"` overrides the Core's "show title" preference.
 * - `icon-text-color="<css color>"` overrides the Core's icon text colour.
 */
const TAG_NAME = 'yin-system-monitor'

class YinSystemMonitorElement extends HTMLElement {
  private vnode: VNode | null = null

  connectedCallback() {
    this.mount()
  }

  disconnectedCallback() {
    this.unmount()
  }

  private mount() {
    const bridge = getThemeMonitorBridge()
    if (!bridge || this.vnode)
      return
    const showTitle = this.hasAttribute('show-title')
      ? this.getAttribute('show-title') !== 'false'
      : bridge.showTitle
    const props = {
      allowEdit: false,
      showTitle,
      iconTextColor: this.getAttribute('icon-text-color') || bridge.iconTextColor,
      snapshotController: bridge.controller,
    }
    // `useDialog`/`useMessage` resolve from the Naive providers, which the app only
    // provides inside its own tree; re-wrap them here. The app context (Pinia, i18n,
    // global properties) is attached so the store and `$t` resolve as they do inside
    // the app.
    const vnode = createVNode(NLoadingBarProvider, null, {
      default: () => createVNode(NDialogProvider, null, {
        default: () => createVNode(NNotificationProvider, null, {
          default: () => createVNode(NMessageProvider, null, {
            default: () => createVNode(SystemMonitor, props),
          }),
        }),
      }),
    })
    vnode.appContext = bridge.appContext
    this.vnode = vnode
    render(vnode, this)
  }

  private unmount() {
    if (!this.vnode)
      return
    render(null, this)
    this.vnode = null
  }
}

export function registerSystemMonitorElement() {
  if (!customElements.get(TAG_NAME))
    customElements.define(TAG_NAME, YinSystemMonitorElement)
}

registerSystemMonitorElement()
