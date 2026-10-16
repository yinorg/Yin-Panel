import { createApp } from 'vue'
import App from './App.vue'
import { setupI18n } from './locales'
import { setupAssets, setupScrollbarStyle } from './plugins'
import { setupStore, useAppStore } from './store'
import { setupRouter } from './router'
import 'virtual:svg-icons-register' // svg图标注册

async function bootstrap() {
  const app = createApp(App)
  setupAssets()

  setupScrollbarStyle()

  setupStore(app)

  // The store is initialized first so the visitor's stored language can decide
  // which locale chunk to fetch before the app mounts. Every non-English locale
  // lives in its own chunk, so this keeps the other sixteen off the first load.
  await setupI18n(app, useAppStore().language)

  await setupRouter(app)
  app.mount('#app')
}

bootstrap()
