import { createApp } from 'vue'
import App from './App.vue'
import { setupI18n } from './locales'
import { setupAssets, setupScrollbarStyle } from './plugins'
import { setupStore, useAppStore } from './store'
import { setupRouter } from './router'
import { loadHomeShell } from './views/home/shell'
import { loadNaiveLocale } from './hooks/useLanguage'
import 'virtual:svg-icons-register' // svg图标注册

async function bootstrap() {
  const app = createApp(App)
  setupAssets()

  setupScrollbarStyle()

  setupStore(app)

  // The themed shell is its own chunk (see `views/home/shell.ts`). Starting it
  // here for a themed entry URL lets the download run alongside the locale chunk
  // and the router instead of after the app mounts. The Core's own pages render
  // no theme, so a direct load of one skips the download; the login page — the
  // only one a visitor genuinely waits on at boot — warms the chunk in the
  // background once its own form has painted.
  const CORE_ONLY_ENTRY_PATHS = new Set(['/login', '/oauth/callback', '/404', '/500', '/__yin/theme-recovery'])
  if (!CORE_ONLY_ENTRY_PATHS.has(window.location.pathname))
    void loadHomeShell().catch(() => {})

  // The store is initialized first so the visitor's stored language can decide
  // which locale chunks to fetch before the app mounts. Every non-English locale
  // lives in its own chunk — for the app's messages and for Naive's own control
  // copy — so this keeps the other sixteen off the first load.
  const language = useAppStore().language
  await Promise.all([setupI18n(app, language), loadNaiveLocale(language)])

  await setupRouter(app)
  app.mount('#app')
}

bootstrap()
