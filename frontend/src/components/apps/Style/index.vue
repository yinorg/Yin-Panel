<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import type { UploadFileInfo } from 'naive-ui'
import { NButton, NCard, NColorPicker, NGrid, NGridItem, NInput, NInputGroup, NModal, NPopconfirm, NSelect, NSlider, NSwitch, NUpload, NUploadDragger, useMessage } from 'naive-ui'
import { set as setUserConfig } from '../../../api/panel/userConfig'
import { useAuthStore, usePanelState } from '@/store'
import { PanelPanelConfigStyleEnum } from '@/enums'
import { t } from '@/locales'
import { getEnableStatus } from '@/api/system/systemMonitor'
import { getSearchConfig, getSpaces, setSearchConfig, spaceOptions, type Space, type SpaceSearchConfig } from '@/api/panel/space'
import { readSpaceCache, writeSpaceCache } from '@/utils/spaceCache'
import { searchEngineList } from '@/components/deskModule/SearchBox/engines'
import { getThemePackage, getThemePackages, installThemePackage, previewThemePackage, removeThemePackage, setInstanceDefaultTheme, uploadWebWallpaper } from '@/api/theme'
import type { ThemePackage } from '@/utils/theme'
import { activeThemePackageId } from '@/hooks/useTheme'

const authStore = useAuthStore()
const panelState = usePanelState()
const ms = useMessage()
const monitorEnabled = ref(false)
const spaces = ref<Space[]>([])
const selectedSearchSpaceId = ref<number | null>(null)
const selectedSearchEngineUrl = ref(searchEngineList[0].url)
const searchConfigSaving = ref(false)
const themeFileInput = ref<HTMLInputElement | null>(null)
const themePackages = ref<Array<{ id: string; name: string; version: string; verified: boolean }>>([])
const defaultThemeId = ref('')
const themeSaving = ref(false)
const previewFile = ref<File | null>(null)
const previewToken = ref('')
const previewPackage = ref<ThemePackage | null>(null)
const previewVisible = ref(false)
const previewPage = ref<'home' | 'login'>('home')
const previewDevice = ref<'desktop' | 'mobile'>('desktop')
const previewMode = ref<'light' | 'dark'>('light')
const webWallpaperInput = ref<HTMLInputElement | null>(null)
const previewURL = computed(() => `${previewPage.value === 'login' ? '/login' : '/'}?themePreview=${encodeURIComponent(previewToken.value)}&themePreviewMode=${previewMode.value}`)
const previewRemoteDomains = computed(() => Object.values(previewPackage.value?.manifest.wallpapers || {})
  .filter(item => item.kind === 'externalUrl')
  .map(item => new URL(item.source).hostname))
const wallpaperModeOptions = computed(() => [
  { label: t('themeWallpaper.follow'), value: 'theme' },
  { label: t('themeWallpaper.custom'), value: 'custom' },
  { label: t('themeWallpaper.none'), value: 'none' },
])
const wallpaperKindOptions = computed(() => [
  { label: t('themeWallpaper.image'), value: 'image' },
  { label: t('themeWallpaper.video'), value: 'video' },
  { label: t('themeWallpaper.web'), value: 'webBundle' },
  { label: t('themeWallpaper.external'), value: 'externalUrl' },
])
watch(() => panelState.panelConfig.wallpaperMode, (mode) => {
  if (mode === 'custom' && !panelState.panelConfig.wallpaperKind)
    panelState.panelConfig.wallpaperKind = 'image'
}, { immediate: true })
const isAdmin = computed(() => authStore.userInfo?.role === 1)
const themeOptions = computed(() => themePackages.value.map(item => ({ label: `${item.name} (${item.version})`, value: item.id })))

async function loadThemePackages() {
  if (!isAdmin.value) return
  const { code, data } = await getThemePackages()
  if (code === 0) {
    themePackages.value = data.packages
    defaultThemeId.value = data.defaultPackage
  }
}

async function uploadThemePackage(file?: File, confirmed = false): Promise<void> {
  if (!file) return
  const result = await installThemePackage(file, confirmed)
  if (result.code !== 0 && !confirmed && /unverified/i.test(result.msg)) {
    if (window.confirm(t('themePackage.confirmUnverified')))
      return uploadThemePackage(file, true)
  }
  else if (result.code === 0) {
    previewVisible.value = false
    ms.success(t('themePackage.installed'))
    await loadThemePackages()
    if (activeThemePackageId.value === result.data?.id)
      window.location.reload()
  }
  else {
    ms.error(result.msg)
  }
}

async function prepareThemePackage(file?: File) {
  if (!file) return
  const result = await previewThemePackage(file)
  if (result.code !== 0 || !result.data) {
    ms.error(result.msg)
    return
  }
  previewFile.value = file
  previewToken.value = result.data.token
  previewPackage.value = result.data.package
  previewVisible.value = true
}

async function saveDefaultTheme() {
  themeSaving.value = true
  try {
    const detail = await getThemePackage(defaultThemeId.value)
    if (detail.code !== 0) {
      ms.error(t('themeWallpaper.loadFailed'))
      return
    }
    const hosts = Object.values(detail.data.manifest.wallpapers || {})
      .filter(item => item.kind === 'externalUrl')
      .map(item => new URL(item.source).hostname)
    if (hosts.length && !window.confirm(t('themeWallpaper.confirmExternal', { hosts: [...new Set(hosts)].join(', ') })))
      return
    const { code, msg } = await setInstanceDefaultTheme(defaultThemeId.value, hosts.length > 0)
    if (code === 0) {
      ms.success(t('themePackage.saved'))
      window.location.reload()
    }
    else ms.error(msg)
  }
  finally {
    themeSaving.value = false
  }
}

async function deleteTheme(id: string) {
  if (!window.confirm(t('themePackage.removeConfirm')))
    return
  const { code, msg } = await removeThemePackage(id)
  if (code === 0) {
    ms.success(t('themePackage.removed'))
    await loadThemePackages()
    if (activeThemePackageId.value === id)
      window.location.reload()
  }
  else ms.error(msg)
}

const searchSpaceOptions = computed(() => spaceOptions(spaces.value, authStore.userInfo?.id))

const searchEngineOptions = searchEngineList.map(engine => ({
  label: engine.title,
  value: engine.url,
}))

function applySearchConfig(config?: SpaceSearchConfig | null) {
  selectedSearchEngineUrl.value = config?.currentSearchEngine?.url || searchEngineList[0].url
}

async function loadSearchConfig(spaceId: number) {
  const cached = readSpaceCache(spaceId, authStore.userInfo?.id)
  if (cached?.searchConfig?.currentSearchEngine?.url) {
    applySearchConfig(cached.searchConfig)
    return
  }
  const { data } = await getSearchConfig<SpaceSearchConfig>(spaceId)
  if (data) {
    applySearchConfig(data)
    const nextCache = readSpaceCache(spaceId, authStore.userInfo?.id)
    if (nextCache) {
      nextCache.searchConfig = data || { currentSearchEngine: searchEngineList[0] }
      writeSpaceCache(spaceId, nextCache, authStore.userInfo?.id)
    }
  }
}

function selectSearchSpace(spaceId: number) {
  selectedSearchSpaceId.value = spaceId
  loadSearchConfig(spaceId)
}

async function saveDefaultSearchEngine() {
  if (!selectedSearchSpaceId.value || searchConfigSaving.value)
    return
  const engine = searchEngineList.find(item => item.url === selectedSearchEngineUrl.value) || searchEngineList[0]
  const config = { currentSearchEngine: engine }
  searchConfigSaving.value = true
  try {
    const { code, msg } = await setSearchConfig(selectedSearchSpaceId.value, config)
    if (code === 0) {
      const cache = readSpaceCache(selectedSearchSpaceId.value, authStore.userInfo?.id)
      if (cache) {
        cache.searchConfig = config
        writeSpaceCache(selectedSearchSpaceId.value, cache, authStore.userInfo?.id)
      }
      window.dispatchEvent(new CustomEvent('yin-panel-search-config-saved', { detail: { spaceId: selectedSearchSpaceId.value, config } }))
      ms.success(t('apps.baseSettings.searchEngineSaved'))
    }
    else {
      ms.error(`${t('apps.baseSettings.searchEngineSaveFailed')}: ${msg}`)
    }
  }
  finally {
    searchConfigSaving.value = false
  }
}

// 获取后端 enableMonitor 配置
onMounted(async () => {
  try {
    // 修正类型定义，确保与实际 API 响应结构匹配
    interface EnableStatusResponse {
      enabled: boolean
    }
    const { data, code } = await getEnableStatus<EnableStatusResponse>()
    if (code === 0)
      monitorEnabled.value = data.enabled
  }
  catch (error) {
    console.error('Failed to get monitor enable status:', error)
  }
  const { data } = await getSpaces<Space[]>()
  if (data?.length) {
    spaces.value = data
    selectSearchSpace(data[0].id)
  }
  await loadThemePackages()
})

const iconTypeOptions = [
  {
    label: t('apps.baseSettings.detailIcon'),
    value: PanelPanelConfigStyleEnum.info,
  },
  {
    label: t('apps.baseSettings.smallIcon'),
    value: PanelPanelConfigStyleEnum.icon,
  },
]

const maxWidthUnitOption = [
  {
    label: 'px',
    value: 'px',
  },
  {
    label: '%',
    value: '%',
  },
]

const maxWidth = computed({
  get: () => String(panelState.panelConfig.maxWidth),
  set: val => panelState.panelConfig.maxWidth = Number(val),
})

function handleUploadBackgroundFinish({
  file,
  event,
}: {
  file: UploadFileInfo
  event?: ProgressEvent
}) {
  const res = JSON.parse((event?.target as XMLHttpRequest).response)
  panelState.panelConfig.backgroundImageSrc = res.data.imageUrl
  panelState.panelConfig.wallpaperMode = 'custom'
  panelState.panelConfig.wallpaperKind = /\.(mp4|webm)$/i.test(file.name) ? 'video' : 'image'
  panelState.panelConfig.wallpaperSource = res.data.imageUrl
  panelState.panelConfig.wallpaperPoster = panelState.panelConfig.wallpaperKind === 'video' ? '/assets/bg-forest.webp' : res.data.imageUrl
  return file
}

async function handleWebWallpaper(file?: File) {
  if (!file) return
  const result = await uploadWebWallpaper(file)
  if (result.code !== 0 || !result.data) {
    ms.error(result.msg)
    return
  }
  panelState.panelConfig.wallpaperMode = 'custom'
  panelState.panelConfig.wallpaperKind = 'webBundle'
  panelState.panelConfig.wallpaperSource = result.data.source
  panelState.panelConfig.wallpaperPoster = result.data.poster
  uploadCloud()
}

function uploadCloud() {
  setUserConfig({ panel: panelState.panelConfig }).then((res) => {
    if (res.code === 0)
      ms.success(t('apps.baseSettings.configSaved'))
    else
      ms.error(t('apps.baseSettings.configFailed', { message: res.msg }))
  })
}

function handleUploadFaviconFinish({
  file,
  event,
}: {
  file: UploadFileInfo
  event?: ProgressEvent
}) {
  const res = JSON.parse((event?.target as XMLHttpRequest).response)
  panelState.panelConfig.logoImageSrc = res.data.imageUrl
  return file
}

function resetPanelConfig() {
  panelState.resetPanelConfig()
  uploadCloud()
}

function adoptThemeDefaults() {
  panelState.panelConfig.useThemeDefaults = true
  uploadCloud()
}
</script>

<template>
  <div class="theme-page rounded-[10px] p-[8px] overflow-auto">
    <NCard v-if="isAdmin" size="small">
      <div class="text-slate-500 mb-2 font-bold">{{ $t('themePackage.title') }}</div>
      <div class="flex flex-wrap items-center gap-2">
        <input ref="themeFileInput" type="file" accept=".yin-theme,.zip" class="hidden" @change="prepareThemePackage(($event.target as HTMLInputElement).files?.[0]); ($event.target as HTMLInputElement).value = ''">
        <NButton size="small" @click="themeFileInput?.click()">{{ $t('themePackage.install') }}</NButton>
        <NSelect v-model:value="defaultThemeId" :options="themeOptions" class="min-w-[220px] flex-1" />
        <NButton size="small" type="primary" :loading="themeSaving" @click="saveDefaultTheme">{{ $t('themePackage.setDefault') }}</NButton>
      </div>
      <div class="mt-3 divide-y">
        <div v-for="item in themePackages" :key="item.id" class="flex items-center justify-between gap-3 py-2">
          <span class="min-w-0 truncate">{{ item.name }} <span class="text-gray-500">{{ item.id }} · {{ item.version }} · {{ item.verified ? $t('themePackage.verified') : $t('themePackage.unverified') }}</span></span>
          <NButton v-if="item.id !== 'org.yin.default'" size="tiny" tertiary type="error" @click="deleteTheme(item.id)">{{ $t('common.delete') }}</NButton>
        </div>
      </div>
    </NCard>
    <NModal v-model:show="previewVisible" preset="card" :title="$t('themeWallpaper.preview')" style="width: min(96vw, 1280px)">
      <div class="flex flex-wrap items-center gap-2 mb-3">
        <span>{{ previewPackage?.manifest.name }} · {{ previewPackage?.manifest.packageVersion }} · {{ previewPackage?.verified ? $t('themePackage.verified') : $t('themePackage.unverified') }}</span>
        <NSelect v-model:value="previewPage" :options="[{ label: $t('themeWallpaper.home'), value: 'home' }, { label: $t('themeWallpaper.login'), value: 'login' }]" style="width: 130px" />
        <NSelect v-model:value="previewDevice" :options="[{ label: $t('themeWallpaper.desktop'), value: 'desktop' }, { label: $t('themeWallpaper.mobile'), value: 'mobile' }]" style="width: 130px" />
        <NSelect v-model:value="previewMode" :options="[{ label: $t('themeWallpaper.light'), value: 'light' }, { label: $t('themeWallpaper.dark'), value: 'dark' }]" style="width: 110px" />
        <NButton type="primary" @click="uploadThemePackage(previewFile || undefined)">{{ $t('themePackage.install') }}</NButton>
      </div>
      <div v-if="previewRemoteDomains.length" class="mb-2 text-sm">
        {{ $t('themeWallpaper.remoteDomain') }}: {{ previewRemoteDomains.join(', ') }}
      </div>
      <div class="flex justify-center overflow-auto bg-neutral-800 p-2">
        <iframe :key="previewURL + previewDevice" :src="previewURL" :title="$t('themeWallpaper.preview')" :style="{ width: previewDevice === 'mobile' ? '390px' : '100%', height: 'min(70vh, 700px)', border: '0', background: '#fff', flexShrink: 0 }" />
      </div>
    </NModal>
    <NCard style="border-radius:10px" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        LOGO
      </div>

      <div>
        <div>
          {{ $t('apps.baseSettings.textContent') }}
        </div>
        <div class="flex items-center mt-[5px]">
          <NInput v-model:value="panelState.panelConfig.logoText" type="text" show-count :maxlength="20" placeholder="请输入文字" />
        </div>
      </div>

      <div class="mt-[10px]">
        <div>
          Favicon
        </div>
        <div class="flex items-center mt-[5px]">
          <NUpload
            action="/api/file/uploadImg"
            :show-file-list="false"
            name="imgfile"
            :headers="{
              Authorization: `Bearer ${authStore.token}`,
            }"
            accept=".ico,.png,.svg"
            @finish="handleUploadFaviconFinish"
          >
            <div class="flex items-center">
              <img
                v-if="panelState.panelConfig.logoImageSrc"
                :src="panelState.panelConfig.logoImageSrc"
                class="w-[32px] h-[32px] mr-[10px] border border-gray-200 rounded"
              >
              <NButton size="small" class="mr-[10px]">
                {{ panelState.panelConfig.logoImageSrc ? $t('common.change') : $t('common.upload') }}
              </NButton>
              <NButton
                v-if="panelState.panelConfig.logoImageSrc"
                size="small"
                @click.stop="panelState.panelConfig.logoImageSrc = ''"
              >
                {{ $t('common.clear') }}
              </NButton>
            </div>
          </NUpload>
        </div>
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.clock') }}
      </div>
      <div class="flex items-center mt-[5px]">
        <span class="mr-[10px]">{{ $t('apps.baseSettings.clockSecondShow') }}</span>
        <NSwitch v-model:value="panelState.panelConfig.clockShowSecond" />
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.searchEngine') }}
      </div>
      <NSelect
        :value="selectedSearchSpaceId"
        :options="searchSpaceOptions"
        :placeholder="$t('apps.baseSettings.selectSpace')"
        @update:value="selectSearchSpace"
      />
      <div class="flex items-center mt-[10px]">
        <NSelect v-model:value="selectedSearchEngineUrl" :options="searchEngineOptions" />
        <NButton class="ml-[10px]" type="primary" :loading="searchConfigSaving" @click="saveDefaultSearchEngine">
          {{ $t('apps.baseSettings.saveSearchEngine') }}
        </NButton>
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.searchBar') }}
      </div>
      <div class="flex items-center mt-[5px]">
        <span class="mr-[10px]">{{ $t('common.show') }}</span>
        <NSwitch v-model:value="panelState.panelConfig.searchBoxShow" />
      </div>
      <div v-if="panelState.panelConfig.searchBoxShow" class="flex items-center mt-[5px]">
        <span class="mr-[10px]">{{ $t('apps.baseSettings.searchBarSearchItem') }}</span>
        <NSwitch v-model:value="panelState.panelConfig.searchBoxSearchIcon" />
      </div>
    </NCard>

    <NCard v-if="monitorEnabled" style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.systemMonitorStatus') }}
      </div>
      <div class="flex items-center mt-[5px]">
        <span class="mr-[10px]">{{ $t('common.show') }}</span>
        <NSwitch v-model:value="panelState.panelConfig.systemMonitorShow" />
      </div>
      <div v-if="panelState.panelConfig.systemMonitorShow" class="flex items-center mt-[5px]">
        <span class="mr-[10px]">{{ $t('apps.baseSettings.showTitle') }}</span>
        <NSwitch v-model:value="panelState.panelConfig.systemMonitorShowTitle" />
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('common.icon') }}
      </div>
      <div class="mt-[5px]">
        <div>
          {{ $t('common.style') }}
        </div>
        <div class="flex items-center mt-[5px]">
          <NSelect v-model:value="panelState.panelConfig.iconStyle" :options="iconTypeOptions" />
        </div>
      </div>

      <div v-if="panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.info" class="mt-[5px]">
        <div>
          {{ $t('apps.baseSettings.hideDescription') }}
        </div>
        <div class="flex items-center mt-[5px]">
          <NSwitch v-model:value="panelState.panelConfig.iconTextInfoHideDescription" />
        </div>
      </div>

      <div v-if="panelState.panelConfig.iconStyle === PanelPanelConfigStyleEnum.icon" class="mt-[5px]">
        <div>
          {{ $t('apps.baseSettings.hideTitle') }}
        </div>
        <div class="flex items-center mt-[5px]">
          <NSwitch v-model:value="panelState.panelConfig.iconTextIconHideTitle" />
        </div>
      </div>

      <div class="mt-[5px]">
        <div>
          {{ $t('common.textColor') }}
        </div>
        <NButton class="mt-2" size="tiny" tertiary @click="adoptThemeDefaults">{{ $t('themePackage.adoptDefaults') }}</NButton>
        <div class="flex items-center mt-[5px]">
          <NColorPicker
            v-model:value="panelState.panelConfig.iconTextColor"
            :show-alpha="false"
            size="small"
            :modes="['hex']"
            :swatches="[
              '#000000',
              '#ffffff',
              '#18A058',
              '#2080F0',
              '#F0A020',
            ]"
          />
        </div>
      </div>
    </NCard>
    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.wallpaper') }}
      </div>
      <NSelect v-model:value="panelState.panelConfig.wallpaperMode" :options="wallpaperModeOptions" class="mb-2" />
      <NSelect v-if="panelState.panelConfig.wallpaperMode === 'custom'" v-model:value="panelState.panelConfig.wallpaperKind" :options="wallpaperKindOptions" class="mb-2" />
      <NUpload
        v-if="panelState.panelConfig.wallpaperMode === 'custom' && (panelState.panelConfig.wallpaperKind === 'image' || panelState.panelConfig.wallpaperKind === 'video')"
        action="/api/file/uploadImg"
        :show-file-list="false"
        name="imgfile"
        :headers="{
          Authorization: `Bearer ${authStore.token}`,
        }"
        :directory-dnd="true"
        :accept="panelState.panelConfig.wallpaperKind === 'video' ? '.mp4,.webm' : '.png,.jpg,.jpeg,.gif,.webp'"
        @finish="handleUploadBackgroundFinish"
      >
        <NUploadDragger style="width: 100%;">
          <div
            class="h-[200px] w-full border bg-slate-100 flex justify-center items-center cursor-pointer rounded-[10px]"
            :style="{ background: `url(${panelState.panelConfig.wallpaperPoster || panelState.panelConfig.backgroundImageSrc}) no-repeat`, backgroundSize: 'cover' }"
          >
            <div class="text-shadow text-white">
              {{ $t('apps.baseSettings.uploadOrDragText') }}
            </div>
          </div>
        </NUploadDragger>
      </NUpload>
      <template v-if="panelState.panelConfig.wallpaperMode === 'custom'">
        <div v-if="panelState.panelConfig.wallpaperKind === 'webBundle'" class="mt-2">
          <input ref="webWallpaperInput" type="file" accept=".yin-wallpaper,.zip" class="hidden" @change="handleWebWallpaper(($event.target as HTMLInputElement).files?.[0]); ($event.target as HTMLInputElement).value = ''">
          <NButton size="small" @click="webWallpaperInput?.click()">{{ $t('themeWallpaper.uploadWeb') }}</NButton>
        </div>
        <div v-else class="mt-2">
          <div class="mb-1">{{ $t('themeWallpaper.source') }}</div>
          <NInput v-model:value="panelState.panelConfig.wallpaperSource" type="text" size="small" clearable />
        </div>
        <div v-if="panelState.panelConfig.wallpaperKind === 'video' || panelState.panelConfig.wallpaperKind === 'externalUrl'" class="mt-2">
          <div class="mb-1">{{ $t('themeWallpaper.poster') }}</div>
          <NInput v-model:value="panelState.panelConfig.wallpaperPoster" type="text" size="small" clearable />
        </div>
      </template>

      <div class="flex items-center mt-[10px]">
        <span class="mr-[10px]">{{ $t('apps.baseSettings.vague') }}</span>
        <NSlider v-model:value="panelState.panelConfig.backgroundBlur" class="max-w-[200px]" :step="2" :max="20" />
      </div>

      <div class="flex items-center mt-[10px]">
        <span class="mr-[10px]">{{ $t('apps.baseSettings.mask') }}</span>
        <NSlider v-model:value="panelState.panelConfig.backgroundMaskNumber" class="max-w-[200px]" :step="0.1" :max="1" />
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.contentArea') }}
      </div>

      <NGrid cols="2">
        <NGridItem span="12 400:12">
          <div class="flex items-center mt-[5px]">
            <span class="mr-[10px]">{{ $t('apps.baseSettings.netModeChangeButtonShow') }}</span>
            <NSwitch v-model:value="panelState.panelConfig.netModeChangeButtonShow" />
          </div>
        </NGridItem>

        <NGridItem span="12 400:12">
          <div class="flex items-center mt-[10px]">
            <span class="mr-[10px]">{{ $t('apps.baseSettings.maxWidth') }}</span>
            <div class="flex">
              <NInputGroup>
                <NInput
                  v-model:value="maxWidth"
                  size="small"
                  :maxlength="10"
                  :style="{ width: '100px' }"
                  placeholder="1200"
                />
                <NSelect v-model:value="panelState.panelConfig.maxWidthUnit" :style="{ width: '80px' }" :options="maxWidthUnitOption" size="small" />
              </NInputGroup>
            </div>
          </div>
        </NGridItem>
        <NGridItem span="12 400:12">
          <div class="flex items-center mt-[10px]">
            <span class="mr-[10px]">{{ $t('apps.baseSettings.leftRightMargin') }}</span>
            <NSlider v-model:value="panelState.panelConfig.marginX" class="max-w-[200px]" :step="1" :max="100" />
          </div>
        </NGridItem>
        <NGridItem span="12 400:12">
          <div class="flex items-center mt-[10px]">
            <span class="mr-[10px]">{{ $t('apps.baseSettings.topMargin') }} (%)</span>
            <NSlider v-model:value="panelState.panelConfig.marginTop" class="max-w-[200px]" :step="1" :max="50" />
          </div>
        </NGridItem>
        <NGridItem span="12 400:6">
          <div class="flex items-center mt-[10px]">
            <span class="mr-[10px]">{{ $t('apps.baseSettings.bottomMargin') }} (%)</span>
            <NSlider v-model:value="panelState.panelConfig.marginBottom" class="max-w-[200px]" :step="1" :max="50" />
          </div>
        </NGridItem>
      </NGrid>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="text-slate-500 mb-[5px] font-bold">
        {{ $t('apps.baseSettings.customFooter') }}
      </div>

      <NInput
        v-model:value="panelState.panelConfig.footerHtml"
        type="textarea"
        clearable
      />
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <NPopconfirm
        @positive-click="resetPanelConfig"
      >
        <template #trigger>
          <NButton size="small" quaternary type="error">
            {{ $t('common.reset') }}
          </NButton>
        </template>
        {{ $t('apps.baseSettings.resetWarnText') }}
      </NPopconfirm>

      <NButton size="small" quaternary type="success" class="ml-[10px]" @click="uploadCloud">
        {{ $t('common.save') }}
      </NButton>
    </NCard>
  </div>
</template>

<style scoped>
.text-shadow{
  text-shadow: 0px 0px 5px gray;
}
</style>
