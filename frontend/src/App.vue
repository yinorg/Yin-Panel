<script setup lang="ts">
import { NConfigProvider } from 'naive-ui'
import NaiveProvider from './components/common/NaiveProvider/index.vue'
import HomeShell from './views/home/HomeShell.vue'
import { useTheme } from './hooks/useTheme'
import { useLanguage } from './hooks/useLanguage'
import { usePanelState } from './store'
import { useRoute } from 'vue-router'
import { computed, watch } from 'vue'

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
