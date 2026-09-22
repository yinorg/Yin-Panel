<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { NAvatar } from 'naive-ui'
import SvgIcon from '../../common/SvgIcon/index.vue'
import { getSearchConfig, type SpaceSearchConfig } from '@/api/panel/space'
import { useAuthStore } from '@/store'
import { readSpaceCache, writeSpaceCache } from '@/utils/spaceCache'
import { replaceOrAppendKeywordToUrl, searchEngineList, type SearchEngine } from './engines'

const props = withDefaults(defineProps<{
  background?: string
  textColor?: string
  spaceId?: number
  sessionOnly?: boolean
}>(), {
  background: '#2a2a2a6b',
  textColor: 'white',
})

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
    <div class="search-container flex rounded-2xl items-center justify-center text-white w-full" :style="{ background, color: textColor }" :class="{ focused: isFocused }">
      <div class="search-box-btn-engine w-[40px] flex justify-center cursor-pointer" @click="handleEngineClick">
        <NAvatar :src="state.currentSearchEngine.iconSrc" style="background-color: transparent;" :size="20" />
      </div>
      <input data-testid="home-search-input" v-model="searchTerm" :placeholder="$t('deskModule.searchBox.inputPlaceholder')" @focus="onFocus" @blur="onBlur" @input="handleItemSearch">
      <div v-if="searchTerm !== ''" class="search-box-btn-clear w-[25px] mr-[10px] flex justify-center cursor-pointer" @click="handleClearSearchTerm">
        <SvgIcon style="width: 20px;height: 20px;" icon="line-md:close-small" />
      </div>
      <div class="search-box-btn-search w-[25px] flex justify-center cursor-pointer" @click="handleSearchClick">
        <SvgIcon style="width: 20px;height: 20px;" icon="iconamoon:search-fill" />
      </div>
    </div>
    <div v-if="searchSelectListShow" class="w-full mt-[10px] rounded-xl p-[10px]" :style="{ background }">
      <div class="flex items-center">
        <div class="flex items-center flex-wrap">
          <div v-for="item, index in searchEngineList" :key="index" :title="item.title" class="w-[40px] h-[40px] cursor-pointer bg-[#ffffff] flex items-center justify-center rounded-xl mr-[10px] mb-[10px]" @click="handleEngineUpdate(item)">
            <NAvatar :src="item.iconSrc" style="background-color: transparent;" :size="20" />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-container {
  border: 1px solid #ccc;
  transition: box-shadow 0.5s,backdrop-filter 0.5s;
  padding: 2px 10px;
  backdrop-filter:blur(2px)
}

.search-container input {
  background-color: transparent;
  box-sizing: border-box;
  width: 100%;
  min-width: 0;
  height: 40px;
  padding: 10px 5px;
  border: none;
  outline: none;
  font-size: 17px;
}

.focused, .search-container:hover {
  box-shadow: 0px 0px 30px -5px rgba(41, 41, 41, 0.45);
  -webkit-box-shadow: 0px 0px 30px -5px rgba(0, 0, 0, 0.45);
  -moz-box-shadow: 0px 0px 30px -5px rgba(0, 0, 0, 0.45);
  backdrop-filter:blur(5px)
}
</style>
