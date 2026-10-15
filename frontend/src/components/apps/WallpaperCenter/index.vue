<script setup lang="ts">
/**
 * The Core's wallpaper centre.
 *
 * The library, the layer that paints a choice and the stored selection all
 * belong to the Core, so every theme opens this exact picker through the
 * `wallpaper-center` Core surface (`api.ui.openCoreSurface('wallpaper-center')`)
 * or by embedding `<yin-wallpaper-center>`. A theme contributes look and feel,
 * never a second picker.
 */
import { computed, onMounted, ref } from 'vue'
import { NButton, NEmpty, NSpin, useMessage } from 'naive-ui'
import { set as setUserConfig } from '@/api/panel/userConfig'
import { usePanelState } from '@/store'
import { useWallpaperLibrary } from '@/hooks/useWallpaperLibrary'
import { t } from '@/locales'

/** The catalogue holds several hundred entries, so the grid is paged instead of
 *  rendered whole: one page of thumbnails costs a few hundred kilobytes. */
const PAGE_SIZE = 24

const props = withDefaults(defineProps<{
  embedded?: boolean
  /** A host that owns the sink passes its own selection and handles `select`
   *  instead: the Space settings write a Space, not the user config. */
  selection?: { mode: 'theme' | 'library'; entry: string } | null
  /** When false the picker only reports the choice and never writes the user
   *  config, so the embedding host stays the single writer. */
  persist?: boolean
}>(), { embedded: false, selection: null, persist: true })
const emit = defineEmits<{ (e: 'select', value: { mode: 'theme' | 'library'; entry: string }): void }>()

const panelState = usePanelState()
const ms = useMessage()
const { library, loading, load } = useWallpaperLibrary()
const categoryId = ref('')
const page = ref(0)

const categories = computed(() => library.value.categories)
const current = computed(() => categories.value.find(category => category.id === categoryId.value) || categories.value[0] || null)
const items = computed(() => current.value?.items || [])
const pageCount = computed(() => Math.max(1, Math.ceil(items.value.length / PAGE_SIZE)))
const tiles = computed(() => items.value.slice(page.value * PAGE_SIZE, (page.value + 1) * PAGE_SIZE))
const selectedEntry = computed(() => props.selection
  ? (props.selection.mode === 'library' ? props.selection.entry : '')
  : (panelState.panelConfig.wallpaperMode === 'library' ? panelState.panelConfig.wallpaperLibraryEntry : ''))
const followsTheme = computed(() => props.selection
  ? props.selection.mode === 'theme'
  : (!panelState.panelConfig.wallpaperMode || panelState.panelConfig.wallpaperMode === 'theme'))

function selectCategory(id: string) {
  categoryId.value = id
  page.value = 0
}

function stepPage(delta: number) {
  page.value = Math.min(pageCount.value - 1, Math.max(0, page.value + delta))
}

function save() {
  setUserConfig({ panel: panelState.panelConfig }).then((res) => {
    if (res.code === 0)
      ms.success(t('apps.baseSettings.configSaved'))
    else
      ms.error(t('apps.baseSettings.configFailed', { message: res.msg }))
  })
}

/** The default tile: hand the wallpaper back to the active theme. */
function chooseTheme() {
  if (!props.persist) {
    emit('select', { mode: 'theme', entry: '' })
    return
  }
  panelState.panelConfig.wallpaperMode = 'theme'
  save()
}

function choose(entry: { id: string }) {
  if (!props.persist) {
    emit('select', { mode: 'library', entry: entry.id })
    return
  }
  panelState.panelConfig.wallpaperMode = 'library'
  panelState.panelConfig.wallpaperLibraryEntry = entry.id
  save()
}

onMounted(() => {
  load()
})
</script>

<template>
  <section class="wallpaper-center" :class="embedded ? 'wallpaper-center--embedded' : ''">
    <header v-if="!embedded" class="wallpaper-center-head">
      <h2 class="wallpaper-center-title">
        {{ t('themeWallpaper.center') }}
      </h2>
    </header>

    <div v-if="categories.length" class="wallpaper-center-tabs">
      <button
        v-for="category in categories"
        :key="category.id"
        type="button"
        class="wallpaper-center-tab"
        :class="{ active: current && current.id === category.id }"
        :aria-pressed="current && current.id === category.id ? 'true' : 'false'"
        @click="selectCategory(category.id)"
      >
        {{ category.name }}
      </button>
    </div>

    <NSpin :show="loading">
      <div v-if="tiles.length" class="wallpaper-center-grid">
        <button
          v-if="page === 0"
          type="button"
          class="wallpaper-center-item"
          :class="{ active: followsTheme }"
          :aria-pressed="followsTheme ? 'true' : 'false'"
          @click="chooseTheme"
        >
          <span class="wallpaper-center-preview wallpaper-center-preview-empty" />
          <span class="wallpaper-center-name">{{ t('themeWallpaper.follow') }}</span>
        </button>
        <button
          v-for="entry in tiles"
          :key="entry.id"
          type="button"
          class="wallpaper-center-item"
          :class="{ active: selectedEntry === entry.id }"
          :aria-pressed="selectedEntry === entry.id ? 'true' : 'false'"
          @click="choose(entry)"
        >
          <img class="wallpaper-center-preview" :src="entry.poster || entry.source" :alt="entry.name" loading="lazy">
          <span class="wallpaper-center-name">{{ entry.name }}</span>
        </button>
      </div>
    </NSpin>

    <NEmpty v-if="!loading && !categories.length" :description="t('themeWallpaper.centerEmpty')" class="wallpaper-center-empty" />

    <div v-if="pageCount > 1" class="wallpaper-center-pager">
      <NButton size="small" :disabled="page <= 0" @click="stepPage(-1)">
        ‹
      </NButton>
      <span class="wallpaper-center-status">{{ page + 1 }} / {{ pageCount }}</span>
      <NButton size="small" :disabled="page >= pageCount - 1" @click="stepPage(1)">
        ›
      </NButton>
    </div>
  </section>
</template>

<style scoped>
/* The picker is teleported out of the config provider, and this app redefines
   naive-ui's `--n-*` variables as its brand colours (`--n-color` and
   `--n-text-color` were both the target colour), so inheriting them painted the
   active tile's label in the same colour as its own background. The picker reads
   the theme's own tokens instead, which the app does not redefine, so it follows
   whichever theme is active. The local names survive only as the fallback for a
   host that sets no tokens at all. */
.wallpaper-center {
  --wpc-text: var(--yin-text, #1f2329);
  --wpc-muted: var(--yin-muted, #7d8798);
  --wpc-border: var(--yin-border, #e6eaf2);
  --wpc-surface: var(--yin-surface, #fff);
  --wpc-hover: color-mix(in srgb, var(--yin-text, #1f2329) 8%, var(--yin-surface, #fff));
  --wpc-preview: var(--yin-canvas, #eef2f9);
  --wpc-accent: var(--yin-primary, #3b6ef5);

  /* The Core mounts the standalone picker inside a rounded panel that carries no
     padding of its own, so the picker insets itself the way every other app in
     that panel does. Without it the 16px corner radius clips the first stroke of
     the title. An embedded picker sits in its host's padded layout instead and
     must not inset a second time. */
  padding: 16px;
}

.wallpaper-center--embedded { padding: 0; }

.wallpaper-center-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
.wallpaper-center-title { margin: 0; font-size: 16px; font-weight: 700; color: var(--wpc-text); }
.wallpaper-center-tabs { display: flex; flex-wrap: wrap; gap: 6px; padding-bottom: 10px; border-bottom: 1px solid var(--wpc-border); }
.wallpaper-center-tab { padding: 5px 14px; border: 1px solid transparent; border-radius: 8px; background: var(--wpc-surface); color: var(--wpc-muted); font-size: 13px; cursor: pointer; }
.wallpaper-center-tab:hover { background: var(--wpc-hover); color: var(--wpc-text); }
.wallpaper-center-tab.active { border-color: var(--wpc-accent); background: var(--wpc-accent); color: #fff; }
.wallpaper-center-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 10px; margin-top: 12px; }
.wallpaper-center-item { display: block; padding: 0; border: 1px solid var(--wpc-border); border-radius: 10px; background: var(--wpc-surface); cursor: pointer; overflow: hidden; text-align: left; }
.wallpaper-center-item:hover { border-color: var(--wpc-accent); }
.wallpaper-center-item.active { border-color: var(--wpc-accent); box-shadow: 0 0 0 2px var(--wpc-accent) inset; }
.wallpaper-center-item.active .wallpaper-center-name { color: var(--wpc-accent); }
.wallpaper-center-preview { display: block; width: 100%; aspect-ratio: 16 / 10; object-fit: cover; background: var(--wpc-preview); }
.wallpaper-center-preview-empty { background: repeating-linear-gradient(135deg, var(--wpc-preview) 0 8px, var(--wpc-hover) 8px 16px); }
.wallpaper-center-name { display: block; padding: 6px 8px; font-size: 12.5px; color: var(--wpc-text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.wallpaper-center-empty { margin-top: 24px; }
.wallpaper-center-pager { display: flex; align-items: center; justify-content: center; gap: 12px; margin-top: 12px; }
.wallpaper-center-status { font-size: 12.5px; color: var(--wpc-muted); }
</style>
