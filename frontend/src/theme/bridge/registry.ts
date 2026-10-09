import { createVNode, defineAsyncComponent, render, type AppContext, type Component, type VNode } from 'vue'
import { NDialogProvider, NLoadingBarProvider, NMessageProvider, NNotificationProvider } from 'naive-ui'

/**
 * The Core component bridge.
 *
 * A theme is plain JavaScript, so it cannot mount a Vue component. The Core
 * registers its own components as custom elements instead; a theme embeds them
 * declaratively and gets the exact Core rendering. Because the theme mounts into
 * the document (light DOM), an embedded element inherits the theme's tokens and
 * CSS, which is what keeps it pixel-identical to the Core's own rendering.
 *
 * Each element mounts the component into its own DOM wrapped in the Naive
 * providers and the app context, so the component resolves its store, i18n and
 * dialog/message exactly as it does inside the Core.
 */
let appContext: AppContext | undefined

export function setBridgeAppContext(context: AppContext | undefined) {
  appContext = context
}

export function getBridgeAppContext() {
  return appContext
}

/**
 * Register one Core component as a custom element.
 *
 * `mapProps` reads the element's attributes (and any Core-owned data) into the
 * component's props. Return `null` to defer the mount — the element is not ready
 * yet (for example the Core data it needs is not published).
 */
export function registerCoreElement(
  tagName: string,
  loader: () => Promise<{ default: Component }>,
  mapProps: (element: HTMLElement) => Record<string, unknown> | null = () => ({}),
) {
  if (customElements.get(tagName))
    return
  const Component = defineAsyncComponent(loader)
  class CoreElement extends HTMLElement {
    private vnode: VNode | null = null

    connectedCallback() {
      this.mount()
    }

    disconnectedCallback() {
      this.unmount()
    }

    private mount() {
      const context = getBridgeAppContext()
      if (!context || this.vnode)
        return
      const props = mapProps(this)
      if (!props)
        return
      const vnode = createVNode(NLoadingBarProvider, null, {
        default: () => createVNode(NDialogProvider, null, {
          default: () => createVNode(NNotificationProvider, null, {
            default: () => createVNode(NMessageProvider, null, {
              default: () => createVNode(Component, props),
            }),
          }),
        }),
      })
      vnode.appContext = context
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
  customElements.define(tagName, CoreElement)
}
