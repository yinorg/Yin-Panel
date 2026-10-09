import { getBridgeHandlers, registerCoreElement } from './registry'

/**
 * The Core's application pages as embeddable custom elements.
 *
 * A theme uses one to render a Core page itself — through the
 * `ui.openCoreSurface` surface route — while keeping the exact Core UI, instead of
 * re-implementing it:
 *
 * - `<yin-user-info>`  → `user-info`
 * - `<yin-style>`      → `theme-settings`
 * - `<yin-space-manage>` → `space-manage`
 * - `<yin-users>`      → `users`
 * - `<yin-about>`      → `about`
 * - `<yin-item-editor>` → `item-editor` (the Core supplies the target and the
 *   write commands; the icon upload still runs in the Core)
 */
registerCoreElement('yin-user-info', () => import('@/components/apps/UserInfo/index.vue'))
registerCoreElement('yin-style', () => import('@/components/apps/Style/index.vue'))
registerCoreElement('yin-space-manage', () => import('@/components/apps/SpaceManage/index.vue'), undefined, undefined, {
  // Creating or deleting a space on the theme-rendered page must refresh the
  // Core's own home data, exactly as the Core's modal path does.
  'spaces-changed': () => { getBridgeHandlers().spacesChanged?.() },
})
registerCoreElement('yin-users', () => import('@/components/apps/Users/index.vue'))
registerCoreElement('yin-about', () => import('@/components/apps/About/index.vue'))
registerCoreElement('yin-item-editor', () => import('@/views/home/components/EditItem/index.vue'), () => {
  const state = getBridgeHandlers().getEditorState?.()
  if (!state)
    return null
  return {
    visible: true,
    itemInfo: state.itemInfo,
    itemGroupId: state.itemGroupId,
    spaceId: state.spaceId,
    mutations: state.mutations,
  }
}, {}, {
  'update:visible': (visible: unknown) => {
    if (visible === false)
      getBridgeHandlers().closeEditor?.()
  },
  'done': () => { getBridgeHandlers().closeEditor?.() },
})
