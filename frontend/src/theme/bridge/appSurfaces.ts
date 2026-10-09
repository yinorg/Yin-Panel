import { registerCoreElement } from './registry'

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
 */
registerCoreElement('yin-user-info', () => import('@/components/apps/UserInfo/index.vue'))
registerCoreElement('yin-style', () => import('@/components/apps/Style/index.vue'))
registerCoreElement('yin-space-manage', () => import('@/components/apps/SpaceManage/index.vue'))
registerCoreElement('yin-users', () => import('@/components/apps/Users/index.vue'))
registerCoreElement('yin-about', () => import('@/components/apps/About/index.vue'))
