<script setup lang="ts">
import { NConfigProvider } from 'naive-ui'
import NaiveProvider from './components/common/NaiveProvider/index.vue'
import HomeShell from './views/home/HomeShell.vue'
import { useTheme } from './hooks/useTheme'
import { useLanguage } from './hooks/useLanguage'
import { usePanelState } from './store'
import { useRoute } from 'vue-router'
import { computed, watch } from 'vue'
import { dismissBootOverlay, setBootProgress } from './core/home/useBootOverlay'

const { theme, themeOverrides } = useTheme()
const { language } = useLanguage()
const panelState = usePanelState()
const route = useRoute()

/**
 * Routes that render the Core's own page instead of the themed shell: the protected
 * surfaces (login, OAuth callback, theme recovery) and the error pages. Everything
 * else is rendered by the theme — the shell owns the runtime and the chrome, and the
 * matched route only mounts a view into the runtime.
 */
const CORE_ONLY_ROUTES = new Set(['login', 'oauthCallback', '404', '500', 'themeRecovery'])
const shellActive = computed(() => !CORE_ONLY_ROUTES.has(String(route.name || '')))

// The themed shell owns its own dismissal (`useBootOverlay` in `HomeChrome`), which
// waits for the theme to paint. A Core-only route (login, OAuth callback, recovery,
// error pages) renders no theme at all, so nothing there would ever retire the boot
// layer — and an undismissed layer blocks the whole page.
watch(shellActive, active => { if (!active) dismissBootOverlay() }, { immediate: true })

// The bundle is parsed and the app is mounting: the first real milestone the boot
// layer's progress bar can report.
setBootProgress(28)

function updateFavicon(url: string | undefined) {
  let link = document.querySelector("link[rel~='icon']") as HTMLLinkElement
  if (!link) {
    link = document.createElement('link')
    link.rel = 'icon'
    document.head.appendChild(link)
  }
  link.href = url || '/assets/favicon.svg'
}

watch(() => panelState.panelConfig.logoImageSrc, (newUrl) => {
  updateFavicon(newUrl)
})

// 首次加载时更新
updateFavicon(panelState.panelConfig.logoImageSrc)
</script>

<template>
  <NConfigProvider
    class="h-full"
    :theme="theme"
    :theme-overrides="themeOverrides"
    :locale="language"
  >
    <NaiveProvider>
      <HomeShell v-if="shellActive" />
      <RouterView v-else />
    </NaiveProvider>
  </NConfigProvider>
</template>
