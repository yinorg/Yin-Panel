import { createVNode, defineAsyncComponent, render, type AppContext, type Component, type VNode } from 'vue'
import { NDialogProvider, NLoadingBarProvider, NMessageProvider, NNotificationProvider } from 'naive-ui'
import type { HomeItemMutationCommands } from '@/core/home/mutations'

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
 * Core handlers a bridged component's events call back into. The Core publishes
 * them (for example "the spaces changed, reload the home") and a bridge element
 * forwards its component's emit to the matching handler, so an action taken on a
 * theme-rendered Core page updates the Core's own state.
 */
export interface ThemeBridgeEditorState {
  itemInfo: Panel.ItemInfo | null
  itemGroupId?: number
  spaceId?: number
  /** The Core's write commands. The theme renders the editor; the Core still owns
   *  the write, including the privileged icon upload. */
  mutations: HomeItemMutationCommands
}

export interface ThemeBridgeHandlers {
  spacesChanged?: () => void
  /** The item editor target the Core asked the theme to render, or null when no
   *  edit is pending. */
  getEditorState?: () => ThemeBridgeEditorState | null
  /** The theme-rendered item editor closed; the Core returns to the home. */
  closeEditor?: () => void
}

let handlers: ThemeBridgeHandlers = {}

export function setBridgeHandlers(next: ThemeBridgeHandlers) {
  handlers = next
}

export function getBridgeHandlers() {
  return handlers
}

/**
 * Register one Core component as a custom element.
 *
 * - `mapProps` reads the element's attributes (and any Core-owned data) into the
 *   component's props. Return `null` to defer the mount — the element is not ready
 *   yet (for example the Core data it needs is not published).
 * - `events` maps a component emit (kebab-case, e.g. `spaces-changed`) to a Core
 *   callback, so the Core stays in step with what the user did in the embedded page.
 */
export function registerCoreElement(
  tagName: string,
  loader: () => Promise<{ default: Component }>,
  mapProps: (element: HTMLElement) => Record<string, unknown> | null = () => ({}),
  hooks: { onConnected?: () => void; onDisconnected?: () => void } = {},
  events: Record<string, (...args: unknown[]) => void> = {},
) {
  if (customElements.get(tagName))
    return
  const Component = defineAsyncComponent(loader)
  class CoreElement extends HTMLElement {
    private vnode: VNode | null = null

    connectedCallback() {
      this.mount()
      hooks.onConnected?.()
    }

    disconnectedCallback() {
      this.unmount()
      hooks.onDisconnected?.()
    }

    private mount() {
      const context = getBridgeAppContext()
      if (!context || this.vnode)
        return
      const props = mapProps(this)
      if (!props)
        return
      const listeners = Object.fromEntries(
        Object.entries(events).map(([name, handler]) => [toHandlerKey(name), handler]),
      )
      const vnode = createVNode(NLoadingBarProvider, null, {
        default: () => createVNode(NDialogProvider, null, {
          default: () => createVNode(NNotificationProvider, null, {
            default: () => createVNode(NMessageProvider, null, {
              default: () => createVNode(Component, { ...props, ...listeners }),
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

/** `spaces-changed` → `onSpacesChanged`, the prop Vue looks up for that emit. */
function toHandlerKey(name: string) {
  return `on${name.split('-').map(part => part.charAt(0).toUpperCase() + part.slice(1)).join('')}`
}
