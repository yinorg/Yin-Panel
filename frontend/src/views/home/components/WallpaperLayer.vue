<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { usePanelState } from '@/store'
import { activeThemeWallpaper } from '@/hooks/useTheme'

const panelState = usePanelState()
const previewThemeDefaults = new URLSearchParams(window.location.search).has('themePreview')
const visible = ref(true)
const online = ref(navigator.onLine)
const reducedMotion = ref(window.matchMedia('(prefers-reduced-motion: reduce)').matches)
const paused = ref(false)
const interacting = ref(false)
const failed = ref(false)
const posterFailed = ref(false)
const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')

const wallpaper = computed(() => {
  const config = panelState.panelConfig
  if (!previewThemeDefaults && config.wallpaperMode === 'none') return null
  if (!previewThemeDefaults && (config.wallpaperMode === 'custom' || (!config.wallpaperMode && config.backgroundImageSrc))) {
    const kind = config.wallpaperKind || 'image'
    const source = config.wallpaperSource || config.backgroundImageSrc || '/assets/bg-forest.webp'
    return { kind, source, poster: config.wallpaperPoster || (kind === 'image' && !/\.gif(?:[?#]|$)/i.test(source) ? source : '/assets/bg-forest.webp'), overlayOpacity: undefined }
  }
  // Default wallpaper. The pre-theme build drew the artwork with no veil at
  // all, so the fallback opacity is 0 rather than the old 0.85 that washed the
  // page out to near-white.
  return activeThemeWallpaper.value || { kind: 'image', source: '/assets/bg-forest.webp', poster: '/assets/bg-forest.webp', overlayOpacity: 0 }
})

const dynamic = computed(() => wallpaper.value?.kind !== 'image' || /\.gif(?:[?#]|$)/i.test(wallpaper.value?.source || ''))
const play = computed(() => dynamic.value && !paused.value && !reducedMotion.value && visible.value && online.value && !failed.value)
const mediaKind = computed(() => wallpaper.value?.kind)
// Static images are painted as a CSS background so their resampling matches the
// pre-theme build exactly; animated media keeps the <img>/<video> path.
const staticImage = computed(() => !!wallpaper.value && !dynamic.value)
// A blur(0px) filter still promotes the layer and changes rasterisation, so it
// is only emitted when a blur is actually configured.
const mediaFilter = computed(() => {
  const blur = panelState.panelConfig.backgroundBlur || 0
  return blur > 0 ? `blur(${blur}px)` : undefined
})
// The mask is a dark veil, never a canvas-coloured wash. Mixing in
// --yin-canvas painted an almost opaque light layer over the artwork, which
// turned the whole home into a flat near-white page. The pre-theme build used
// a fully transparent mask, so the default stays transparent and only a
// user-configured mask dims the artwork.
const maskColor = computed(() => {
  if (previewThemeDefaults || panelState.panelConfig.wallpaperMode === 'theme' || panelState.panelConfig.useThemeDefaults)
    return `rgba(0,0,0,${wallpaper.value?.overlayOpacity ?? 0})`
  return `rgba(0,0,0,${panelState.panelConfig.backgroundMaskNumber ?? 0})`
})
watch(() => wallpaper.value?.source, () => { failed.value = false; posterFailed.value = false; interacting.value = false })

function updateVisibility() {
  visible.value = !document.hidden
  if (!visible.value) interacting.value = false
}
function updateOnline() { online.value = navigator.onLine }
function updateMotion() { reducedMotion.value = motionQuery.matches }
function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') interacting.value = false
}

onMounted(() => {
  document.addEventListener('visibilitychange', updateVisibility)
  window.addEventListener('online', updateOnline)
  window.addEventListener('offline', updateOnline)
  window.addEventListener('keydown', onKeydown)
  motionQuery.addEventListener('change', updateMotion)
})
onUnmounted(() => {
  document.removeEventListener('visibilitychange', updateVisibility)
  window.removeEventListener('online', updateOnline)
  window.removeEventListener('offline', updateOnline)
  window.removeEventListener('keydown', onKeydown)
  motionQuery.removeEventListener('change', updateMotion)
})
</script>

<template>
  <div v-if="wallpaper" class="wallpaper-layer" data-testid="wallpaper-layer" :style="{ '--wallpaper-blur': `${panelState.panelConfig.backgroundBlur || 0}px` }">
    <!-- Static images render as a CSS background, matching the pre-theme build.
         An <img object-fit: cover> resamples slightly differently from
         background-size: cover, which shows up as anti-aliasing noise across
         the whole photograph. -->
    <div
      v-if="staticImage"
      class="wallpaper-media wallpaper-media--background"
      :style="{ backgroundImage: `url(${posterFailed ? '/assets/bg-forest.webp' : wallpaper.source})`, filter: mediaFilter }"
    />
    <img
      v-else
      class="wallpaper-media"
      :src="posterFailed ? '/assets/bg-forest.webp' : play && mediaKind === 'image' ? wallpaper.source : wallpaper.poster"
      alt=""
      @error="posterFailed = true"
    >
    <video
      v-if="play && mediaKind === 'video'"
      class="wallpaper-media"
      :src="wallpaper.source"
      :poster="wallpaper.poster"
      autoplay muted loop playsinline
      @error="failed = true"
    />
    <iframe
      v-if="play && (mediaKind === 'webBundle' || mediaKind === 'externalUrl')"
      class="wallpaper-media wallpaper-frame"
      :class="{ 'wallpaper-frame--interactive': interacting }"
      :src="wallpaper.source"
      :title="$t('apps.baseSettings.wallpaper')"
      sandbox="allow-scripts"
      referrerpolicy="no-referrer"
      allow="camera 'none'; microphone 'none'; geolocation 'none'"
      @error="failed = true"
    />
    <div class="wallpaper-mask" :style="{ backgroundColor: maskColor }" />
    <Teleport to="body">
    <div v-if="dynamic && !failed" class="wallpaper-controls">
      <button class="wallpaper-control" type="button" :title="paused ? $t('themeWallpaper.wallpaperPlay') : $t('themeWallpaper.wallpaperPause')" :aria-label="paused ? $t('themeWallpaper.wallpaperPlay') : $t('themeWallpaper.wallpaperPause')" @click="paused = !paused; interacting = false">
        <SvgIcon :icon="paused ? 'tabler-player-play' : 'tabler-player-pause'" />
      </button>
      <button v-if="play && (mediaKind === 'webBundle' || mediaKind === 'externalUrl')" class="wallpaper-control" type="button" :title="interacting ? $t('themeWallpaper.wallpaperExit') : $t('themeWallpaper.wallpaperInteract')" :aria-label="interacting ? $t('themeWallpaper.wallpaperExit') : $t('themeWallpaper.wallpaperInteract')" @click="interacting = !interacting">
        <SvgIcon :icon="interacting ? 'tabler-arrow-back-up' : 'tabler-hand-click'" />
      </button>
    </div>
    </Teleport>
  </div>
</template>

<style scoped>
.wallpaper-layer { position: fixed; z-index: 0; inset: 0; width: 100vw; height: 100dvh; overflow: hidden; background: var(--yin-canvas); }
.wallpaper-media { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transform: scale(1.05); }
.wallpaper-media--background { background-position: center center; background-size: cover; background-repeat: no-repeat; object-fit: fill; }
.wallpaper-frame { border: 0; pointer-events: none; }
.wallpaper-frame--interactive { z-index: 1; pointer-events: auto; filter: none; }
.wallpaper-mask { position: absolute; inset: 0; pointer-events: none; }
.wallpaper-controls { position: fixed; bottom: 16px; right: 16px; z-index: 20; display: flex; gap: 8px; }
.wallpaper-control { width: 36px; height: 36px; display: grid; place-items: center; border: 1px solid var(--yin-border); border-radius: var(--yin-radiusControl); background: var(--yin-surfaceElevated); color: var(--yin-text); box-shadow: var(--yin-shadowPopup); cursor: pointer; }
.wallpaper-control:focus-visible { outline: 2px solid var(--yin-focusRing); }
</style>
