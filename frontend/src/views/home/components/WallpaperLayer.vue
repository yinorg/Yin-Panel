<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { usePanelState } from '@/store'
import { activeThemeWallpaper } from '@/hooks/useTheme'
import { useWallpaperLibrary } from '@/hooks/useWallpaperLibrary'
import { publishWallpaperTone } from '@/core/home/wallpaperTone'

const panelState = usePanelState()
// The Core wallpaper library: a stored selection is an id, so the layer resolves
// it here. The picker shares this request, but the layer must be able to paint a
// choice on its own, before the picker has ever been opened.
const wallpaperLibrary = useWallpaperLibrary()
const previewThemeDefaults = new URLSearchParams(window.location.search).has('themePreview')
const visible = ref(true)
const online = ref(navigator.onLine)
const reducedMotion = ref(window.matchMedia('(prefers-reduced-motion: reduce)').matches)
const failed = ref(false)
const posterFailed = ref(false)
const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')

const wallpaper = computed(() => {
  const config = panelState.panelConfig
  if (!previewThemeDefaults && config.wallpaperMode === 'none') return null
  if (!previewThemeDefaults && config.wallpaperMode === 'library') {
    const entry = wallpaperLibrary.find(config.wallpaperLibraryEntry)
    // An entry the library no longer carries (a rebuilt or narrowed pack) falls
    // through to the theme's own wallpaper rather than leaving the page bare.
    if (entry) return { kind: entry.kind, source: entry.source, poster: entry.poster || entry.source, overlayOpacity: 0 }
  }
  if (!previewThemeDefaults && (config.wallpaperMode === 'custom' || (!config.wallpaperMode && config.backgroundImageSrc))) {
    const kind = config.wallpaperKind || 'image'
    const source = config.wallpaperSource || config.backgroundImageSrc || '/assets/bg-forest.webp'
    return { kind, source, poster: config.wallpaperPoster || (kind === 'image' && !/\.gif(?:[?#]|$)/i.test(source) ? source : '/assets/bg-forest.webp'), overlayOpacity: undefined }
  }
  // A Space can name a default wallpaper for its home. It only applies while the
  // visitor has not picked one of their own, so a personal choice always wins.
  if (!previewThemeDefaults && (config.wallpaperMode === undefined || config.wallpaperMode === 'theme')) {
    const space = panelState.spaceWallpaper
    if (space && space.mode === 'library') {
      const entry = wallpaperLibrary.find(space.libraryEntry)
      if (entry) return { kind: entry.kind, source: entry.source, poster: entry.poster || entry.source, overlayOpacity: 0 }
    }
    if (space && space.mode === 'custom' && space.source) {
      const kind = space.kind || 'image'
      return { kind, source: space.source, poster: space.poster || space.source, overlayOpacity: undefined }
    }
  }
  // Default wallpaper. The pre-theme build drew the artwork with no veil at
  // all, so the fallback opacity is 0 rather than the old 0.85 that washed the
  // page out to near-white.
  return activeThemeWallpaper.value || { kind: 'image', source: '/assets/bg-forest.webp', poster: '/assets/bg-forest.webp', overlayOpacity: 0 }
})

const dynamic = computed(() => wallpaper.value?.kind !== 'image' || /\.gif(?:[?#]|$)/i.test(wallpaper.value?.source || ''))
const play = computed(() => dynamic.value && !reducedMotion.value && visible.value && online.value && !failed.value)
const mediaKind = computed(() => wallpaper.value?.kind)
// Kinds the layer paints in a frame. A library `page` entry is the same shape as
// a web-bundle or remote wallpaper: an HTML document the asset route sandboxes.
const framed = computed(() => mediaKind.value === 'webBundle' || mediaKind.value === 'externalUrl' || mediaKind.value === 'page')
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
watch(() => wallpaper.value?.source, () => { failed.value = false; posterFailed.value = false })
// The Core owns the wallpaper, so this is the one place that can read it. The
// tone travels to the theme through the snapshot; a framed or cross-origin
// artwork resolves to no tone rather than a guessed one.
watch(() => wallpaper.value?.poster || wallpaper.value?.source || null, source => { void publishWallpaperTone(source) }, { immediate: true })
// The library must be loaded whenever a selection exists — not only at mount:
// the panel config arrives after the layer mounts on a cold load, and the picker
// can switch the mode while the page is open.
watch(() => [panelState.panelConfig.wallpaperMode, panelState.panelConfig.wallpaperLibraryEntry] as const, ([mode]) => {
  if (mode === 'library') void wallpaperLibrary.load()
}, { immediate: true })

function updateVisibility() { visible.value = !document.hidden }
function updateOnline() { online.value = navigator.onLine }
function updateMotion() { reducedMotion.value = motionQuery.matches }

onMounted(() => {
  document.addEventListener('visibilitychange', updateVisibility)
  window.addEventListener('online', updateOnline)
  window.addEventListener('offline', updateOnline)
  motionQuery.addEventListener('change', updateMotion)
})
onUnmounted(() => {
  document.removeEventListener('visibilitychange', updateVisibility)
  window.removeEventListener('online', updateOnline)
  window.removeEventListener('offline', updateOnline)
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
      v-if="play && framed"
      class="wallpaper-media wallpaper-frame"
      :src="wallpaper.source"
      :title="$t('apps.baseSettings.wallpaper')"
      sandbox="allow-scripts"
      referrerpolicy="no-referrer"
      allow="camera 'none'; microphone 'none'; geolocation 'none'"
      @error="failed = true"
    />
    <div class="wallpaper-mask" :style="{ backgroundColor: maskColor }" />
  </div>
</template>

<style scoped>
.wallpaper-layer { position: fixed; z-index: 0; inset: 0; width: 100vw; height: 100dvh; overflow: hidden; background: var(--yin-canvas); }
.wallpaper-media { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; transform: scale(1.05); }
.wallpaper-media--background { background-position: center center; background-size: cover; background-repeat: no-repeat; object-fit: fill; }
.wallpaper-frame { border: 0; pointer-events: none; }
.wallpaper-mask { position: absolute; inset: 0; pointer-events: none; }
</style>
