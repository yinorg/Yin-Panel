import type { App } from 'vue'
import type { RouteRecordRaw } from 'vue-router'
import { createRouter, createWebHistory } from 'vue-router'
import { setupPageGuard } from './permission'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: 'Home',
    component: () => import('../views/home/index.vue'),
  },
  {
    // A theme-contributed surface (theme-settings, theme-page) rendered on its own
    // route. `HomeShell` (rendered app-level) owns the runtime; this route mounts a
    // view into it. The Core renders no page of its own here, so when the theme does
    // not contribute the view, `ThemeSurface.vue` returns to the home.
    path: '/theme/:view',
    name: 'themeSurface',
    component: () => import('../views/home/ThemeSurface.vue'),
  },

  {
    path: '/login',
    name: 'login',
    component: () => import('../views/login/index.vue'),
  },
  {
    path: '/oauth/callback',
    name: 'oauthCallback',
    component: () => import('../views/oauth/callback/index.vue'),
  },

  {
    path: '/404',
    name: '404',
    component: () => import('../views/exception/404/index.vue'),
  },

  {
    path: '/500',
    name: '500',
    component: () => import('../views/exception/500/index.vue'),
  },

  {
    path: '/__yin/theme-trusted/:revision',
    name: 'trustedThemeHome',
    component: () => import('../views/home/index.vue'),
  },

  {
    path: '/__yin/theme-recovery',
    name: 'themeRecovery',
    component: () => import('../views/theme/recovery/index.vue'),
  },

  // 专门处理公开访问代码的路由
  // 匹配空间公开 FN ID
  {
    path: '/:code([a-z][a-z0-9-]{4,28}[a-z0-9])',
    name: 'PublicAccess',
    component: () => import('../views/home/index.vue'),
  },

  // 必须放在最后，作为兜底
  {
    path: '/:pathMatch(.*)*',
    name: 'notFound',
    redirect: '/404',
  },

  // adminRouter,
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
  scrollBehavior: () => ({ left: 0, top: 0 }),
})

/**
 * A lazily loaded route can 404 after an upgrade: build assets carry a content
 * hash, so the file a previously loaded document asked for no longer exists once
 * a new build lands. Reloading once fetches the current entry document, which
 * references the current chunks.
 *
 * The session flag makes this a single retry — without it a genuinely missing
 * chunk would reload forever — and it is cleared as soon as a navigation
 * succeeds, so a later upgrade can recover on its own.
 */
const CHUNK_RECOVERY_KEY = 'yin-chunk-reload'

function isChunkLoadFailure(error: unknown) {
  const message = error instanceof Error ? error.message : String(error)
  return /dynamically imported module|Failed to fetch|Importing a module script failed|ChunkLoadError/i.test(message)
}

router.onError((error) => {
  if (!isChunkLoadFailure(error)) return
  try {
    if (sessionStorage.getItem(CHUNK_RECOVERY_KEY) === '1') return
    sessionStorage.setItem(CHUNK_RECOVERY_KEY, '1')
  }
  catch {
    // Private mode can refuse storage; a reload is still better than a dead page.
  }
  window.location.reload()
})

router.afterEach(() => {
  try {
    sessionStorage.removeItem(CHUNK_RECOVERY_KEY)
  }
  catch {
    // Nothing to clear.
  }
})

setupPageGuard(router)

export async function setupRouter(app: App) {
  app.use(router)
  await router.isReady()
}
