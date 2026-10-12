import { getBridgeHandlers, registerCoreElement } from './registry'

/**
 * The Core's application pages as embeddable custom elements.
 *
 * A theme uses one to render a Core page itself — through the
 * `ui.openCoreSurface` surface route — while keeping the exact Core UI, instead of
 * re-implementing it:
 *
 * - `<yin-account>`    → `account`
 * - `<yin-appearance>` → `appearance` (the former `theme-settings` surface)
 * - `<yin-layout>`     → `layout`
 * - `<yin-space-manage>` → `space-manage`
 * - `<yin-instance>`   → `instance`
 * - `<yin-users>`      → `users`
 * - `<yin-about>`      → `about`
 * - `<yin-item-editor>` → `item-editor` (the Core supplies the target and the
 *   write commands; the icon upload still runs in the Core)
 *
 * `<yin-user-info>` and `<yin-style>` are the names themes were written against
 * before the settings pages were split; they stay registered and resolve to the
 * account and appearance pages.
 */
registerCoreElement('yin-account', () => import('@/components/apps/Account/index.vue'))
registerCoreElement('yin-appearance', () => import('@/components/apps/Appearance/index.vue'))
registerCoreElement('yin-layout', () => import('@/components/apps/Layout/index.vue'))
registerCoreElement('yin-instance', () => import('@/components/apps/Instance/index.vue'))
registerCoreElement('yin-user-info', () => import('@/components/apps/Account/index.vue'))
registerCoreElement('yin-style', () => import('@/components/apps/Appearance/index.vue'))
registerCoreElement('yin-space-manage', () => import('@/components/apps/SpaceManage/index.vue'), undefined, undefined, {
  // Creating or deleting a space on the theme-rendered page must refresh the
  // Core's own home data, exactly as the Core's modal path does.
  'spaces-changed': () => { getBridgeHandlers().spacesChanged?.() },
})
registerCoreElement('yin-users', () => import('@/components/apps/Users/index.vue'))
registerCoreElement('yin-about', () => import('@/components/apps/About/index.vue'))
// The wallpaper centre: a theme embeds the Core picker rather than drawing one.
registerCoreElement('yin-wallpaper-center', () => import('@/components/apps/WallpaperCenter/index.vue'))
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
