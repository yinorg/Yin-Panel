<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { NAvatar } from 'naive-ui'
import SvgIcon from '../../common/SvgIcon/index.vue'
import { getSearchConfig, type SpaceSearchConfig } from '@/api/panel/space'
import { useAuthStore } from '@/store'
import { readSpaceCache, writeSpaceCache } from '@/utils/spaceCache'
import { replaceOrAppendKeywordToUrl, searchEngineList, type SearchEngine } from './engines'

const props = defineProps<{
  background?: string
  textColor?: string
  spaceId?: number
  sessionOnly?: boolean
  directory?: boolean
}>()

const emits = defineEmits<{
  (e: 'itemSearch', value: string): void
  (e: 'searchEngineChange', value: SearchEngine): void
}>()
const authStore = useAuthStore()
const searchTerm = ref('')
const isFocused = ref(false)
const searchSelectListShow = ref(false)

interface State {
  currentSearchEngine: SearchEngine
}

const defaultState = (): State => ({ currentSearchEngine: searchEngineList[0] })
const state = ref<State>(defaultState())
let loadGeneration = 0

function applySearchConfig(config?: SpaceSearchConfig | null) {
  const current = config?.currentSearchEngine
  state.value = {
    currentSearchEngine: current?.url ? current : defaultState().currentSearchEngine,
  }
  emits('searchEngineChange', state.value.currentSearchEngine)
}

async function loadSearchConfig(spaceId?: number) {
  const generation = ++loadGeneration
  searchSelectListShow.value = false
  applySearchConfig()
  if (!spaceId)
    return

  const cache = readSpaceCache(spaceId, authStore.userInfo?.id, props.sessionOnly)
  if (cache?.searchConfig?.currentSearchEngine?.url) {
    applySearchConfig(cache.searchConfig)
    if (!navigator.onLine) return
  }

  try {
    const { data } = await getSearchConfig<SpaceSearchConfig>(spaceId)
    if (generation !== loadGeneration)
      return
    if (data) {
      applySearchConfig(data)
      const nextCache = readSpaceCache(spaceId, authStore.userInfo?.id, props.sessionOnly)
      if (nextCache) {
        nextCache.searchConfig = data || { currentSearchEngine: defaultState().currentSearchEngine }
        writeSpaceCache(spaceId, nextCache, authStore.userInfo?.id, props.sessionOnly)
      }
    }
  }
  catch {
    if (generation === loadGeneration)
      applySearchConfig()
  }
}

const onFocus = (): void => { isFocused.value = true }
const onBlur = (): void => { isFocused.value = false }
function handleEngineClick() { searchSelectListShow.value = !searchSelectListShow.value }
function handleEngineUpdate(engine: SearchEngine) {
  state.value.currentSearchEngine = engine
  emits('searchEngineChange', engine)
  searchSelectListShow.value = false
}

function handleSearchClick() {
  const fullUrl = replaceOrAppendKeywordToUrl(state.value.currentSearchEngine.url, searchTerm.value)
  handleClearSearchTerm()
  window.open(fullUrl)
}

const handleItemSearch = () => { emits('itemSearch', searchTerm.value) }
function handleClearSearchTerm() {
  searchTerm.value = ''
  emits('itemSearch', searchTerm.value)
}

watch(() => props.spaceId, spaceId => loadSearchConfig(spaceId), { immediate: true })

function handleSavedSearchConfig(event: Event) {
  const detail = (event as CustomEvent<{ spaceId: number; config: SpaceSearchConfig }>).detail
  if (detail?.spaceId !== props.spaceId)
    return
  applySearchConfig(detail.config)
  const cache = readSpaceCache(detail.spaceId, authStore.userInfo?.id, props.sessionOnly)
  if (cache) {
    cache.searchConfig = detail.config
    writeSpaceCache(detail.spaceId, cache, authStore.userInfo?.id, props.sessionOnly)
  }
}

onMounted(() => window.addEventListener('yin-panel-search-config-saved', handleSavedSearchConfig))
onUnmounted(() => window.removeEventListener('yin-panel-search-config-saved', handleSavedSearchConfig))
</script>

<template>
  <div class="search-box w-full" @keydown.enter="handleSearchClick" @keydown.esc="handleClearSearchTerm">
    <div class="search-container flex items-center justify-center w-full" :style="props.directory ? undefined : { background, color: textColor }" :class="{ focused: isFocused, 'search-container--directory': directory }">
      <div class="search-box-btn-engine flex justify-center cursor-pointer" @click="handleEngineClick">
        <NAvatar :src="state.currentSearchEngine.iconSrc" style="background-color: transparent;" :size="20" />
      </div>
      <input data-testid="home-search-input" v-model="searchTerm" :placeholder="$t('deskModule.searchBox.inputPlaceholder')" @focus="onFocus" @blur="onBlur" @input="handleItemSearch">
      <div v-if="searchTerm !== ''" class="search-box-btn-clear w-[25px] flex justify-center cursor-pointer" @click="handleClearSearchTerm">
        <SvgIcon style="width: 20px;height: 20px;" icon="line-md:close-small" />
      </div>
      <div class="search-box-btn-search w-[25px] flex justify-center cursor-pointer" @click="handleSearchClick">
        <SvgIcon style="width: 20px;height: 20px;" icon="iconamoon:search-fill" />
      </div>
    </div>
    <div v-if="searchSelectListShow" class="search-engines w-full mt-[10px]" :class="{ 'search-engines--directory': directory }" :style="directory ? undefined : { background }">
      <div class="flex items-center">
        <div class="flex items-center flex-wrap">
          <div v-for="item, index in searchEngineList" :key="index" :title="item.title" class="search-engine-option cursor-pointer flex items-center justify-center" @click="handleEngineUpdate(item)">
            <NAvatar :src="item.iconSrc" style="background-color: transparent;" :size="20" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-container--directory { min-height: var(--yin-component-search-box-height); padding: 0 var(--yin-component-input-padding-x); border: var(--yin-component-search-box-border-width) solid var(--yin-border); border-radius: var(--yin-component-search-box-radius); background: var(--yin-surfaceElevated); color: var(--yin-text); box-shadow: var(--yin-component-search-box-shadow); }
.search-container--directory input { color: var(--yin-text); }
.search-container--directory input::placeholder { color: var(--yin-textMuted); }
.search-engines--directory { border: var(--yin-component-search-box-border-width) solid var(--yin-border); border-radius: var(--yin-component-search-box-radius); background: var(--yin-surfaceElevated); box-shadow: var(--yin-component-search-box-shadow); }
.search-engine-option { width: var(--yin-component-search-box-option-size); height: var(--yin-component-search-box-option-size); margin: 0 var(--yin-component-search-box-option-gap) var(--yin-component-search-box-option-gap) 0; border-radius: var(--yin-component-input-radius); background: var(--yin-surface); }
</style>

<style scoped>
.search-container {
  border: var(--yin-component-search-box-border-width) solid var(--yin-border);
  border-radius: var(--yin-component-search-box-radius);
  transition: box-shadow var(--yin-component-state-hover-duration) var(--yin-component-state-easing), backdrop-filter var(--yin-component-state-hover-duration) var(--yin-component-state-easing);
  min-height: var(--yin-component-search-box-height);
  padding: 0 var(--yin-component-input-padding-x);
  backdrop-filter: blur(var(--yin-component-search-box-blur));
  background: var(--yin-component-search-box-surface);
  color: var(--yin-text);
}
.search-engines { padding: var(--yin-component-menu-padding); border-radius: var(--yin-component-menu-radius); background: var(--yin-component-menu-surface, var(--yin-surfaceElevated)); box-shadow: var(--yin-component-menu-shadow); }
.search-engines { margin-top: var(--yin-component-search-box-option-gap); }
.search-box-btn-engine { flex: 0 0 var(--yin-component-search-box-option-size); }
.search-box-btn-clear { margin-right: var(--yin-component-search-box-option-gap); }

.search-container input {
  background-color: transparent;
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  height: var(--yin-component-search-box-height);
  padding: 0 var(--yin-spaceXs);
  border: none;
  outline: none;
  font: var(--yin-fontBodyWeight) var(--yin-fontBodySize)/var(--yin-lineHeightBody) var(--yin-fontBody);
}

.focused, .search-container:hover {
  box-shadow: var(--yin-component-search-box-shadow);
  backdrop-filter: blur(var(--yin-component-search-box-blur));
}
</style>
