<script setup lang="ts">
import type { UploadFileInfo } from 'naive-ui'
import { NButton, NCard, NColorPicker, NInput, NPopconfirm, NSelect, NSlider, NSwitch, NUpload, NUploadDragger, useMessage } from 'naive-ui'
import { computed, inject, onMounted, ref, watch } from 'vue'
import { useAppStore, usePanelState } from '@/store'
import { PanelPanelConfigStyleEnum } from '@/enums'
import { t } from '@/locales'
import type { Theme } from '@/store/modules/app/helper'
import { getInstalledThemes, getMyTheme, saveThemePreference, uploadWebWallpaper } from '@/api/theme'
import { refreshMyTheme } from '@/hooks/useTheme'
import { usePanelConfigSave } from '@/hooks/usePanelConfigSave'
import { openSettingsAppKey } from '../navigation'

const appStore = useAppStore()
const panelState = usePanelState()
const ms = useMessage()
const { savePanelConfig } = usePanelConfigSave()
const openSettingsApp = inject(openSettingsAppKey, () => {})

// Theme: colour scheme, where the theme comes from, and which package to use.
const themeValue = ref(appStore.theme)
const selectedThemePackage = ref('')
const themeModeValue = ref<'custom' | 'follow-space'>('custom')
const themePackageOptions = ref<{ label: string; value: string }[]>([])
const themeModeOptions = computed(() => [
  { label: t('themeScope.modeCustom'), value: 'custom' },
  { label: t('themeScope.modeFollowSpace'), value: 'follow-space' },
])
const themeOptions: { label: string; key: string; value: Theme }[] = [
  { label: t('apps.appearance.themeStyle.dark'), key: 'dark', value: 'dark' },
  { label: t('apps.appearance.themeStyle.light'), key: 'light', value: 'light' },
  { label: t('apps.appearance.themeStyle.auto'), key: 'Auto', value: 'auto' },
]

// Icon presentation.
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

// Wallpaper.
const webWallpaperInput = ref<HTMLInputElement | null>(null)
const wallpaperModeOptions = computed(() => [
  { label: t('themeWallpaper.follow'), value: 'theme' },
  { label: t('themeWallpaper.center'), value: 'library' },
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

async function loadThemeSettings() {
  try {
    const [installed, mine] = await Promise.all([getInstalledThemes(), getMyTheme()])
    if (installed.code === 0) {
      themePackageOptions.value = [
        { label: t('themePackage.instanceDefault'), value: '' },
        ...installed.data.map(item => ({ label: item.name, value: item.id })),
      ]
    }
    if (mine.code === 0) {
      selectedThemePackage.value = mine.data.preference.packageId || ''
      themeValue.value = mine.data.preference.mode
      themeModeValue.value = mine.data.preference.themeMode || 'custom'
    }
  }
  catch {
    // Theme selection remains on the server's current default when unavailable.
  }
}

function handleChangeTheme(value: Theme) {
  themeValue.value = value
  appStore.setTheme(value)
  saveThemePreference({ packageId: selectedThemePackage.value, mode: value })
}

async function handleChangeThemePackage(packageId: string) {
  selectedThemePackage.value = packageId
  const result = await saveThemePreference({ packageId, mode: themeValue.value, themeMode: themeModeValue.value })
  if (result.code === 0) {
    await refreshMyTheme(appStore)
    ms.success(t('themePackage.saved'))
  }
  else {
    ms.error(result.msg)
  }
}

async function handleChangeThemeMode(value: 'custom' | 'follow-space') {
  themeModeValue.value = value
  const result = await saveThemePreference({ packageId: selectedThemePackage.value, mode: themeValue.value, themeMode: value })
  if (result.code === 0) {
    await refreshMyTheme(appStore)
    ms.success(t('themeScope.saved'))
  }
  else {
    ms.error(result.msg)
  }
}

onMounted(loadThemeSettings)

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
  savePanelConfig()
}

function adoptThemeDefaults() {
  panelState.panelConfig.useThemeDefaults = true
  savePanelConfig()
}

/**
 * The home's surface transparency, defaulted here rather than only in the store:
 * a configuration saved before this setting existed has no key for it, and a
 * slider bound straight to the store would open at zero while the surfaces were
 * actually at the default.
 */
const surfaceOpacity = computed({
  get: () => Number(panelState.panelConfig.surfaceOpacity ?? 0.72),
  set: (value: number) => { panelState.panelConfig.surfaceOpacity = value },
})

function resetAppearanceConfig() {
  panelState.resetAppearanceConfig()
  savePanelConfig()
}
</script>

<template>
  <div class="theme-page rounded-[10px] p-[8px] overflow-auto">
    <NCard style="border-radius:10px" size="small">
      <div class="yin-text-muted mb-[5px] font-bold">
        {{ $t('apps.appearance.identity') }}
      </div>
      <div class="mb-[10px] text-xs opacity-70">
        {{ $t('apps.appearance.identityHint') }}
      </div>

      <div>
        <div>
          {{ $t('apps.baseSettings.textContent') }}
        </div>
        <div class="flex items-center mt-[5px]">
          <NInput v-model:value="panelState.panelConfig.logoText" type="text" show-count :maxlength="20" :placeholder="$t('apps.baseSettings.logoTextPlaceholder')" />
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
      <div class="yin-text-muted mb-[10px] font-bold">
        {{ $t('apps.appearance.theme') }}
      </div>

      <div>
        <div class="mb-1">
          {{ $t('apps.appearance.scheme') }}
        </div>
        <div class="max-w-[200px]">
          <NSelect v-model:value="themeValue" to="body" :options="themeOptions" @update-value="handleChangeTheme" />
        </div>
      </div>

      <div class="mt-3">
        <div class="mb-1">
          {{ $t('themeScope.mode') }}
        </div>
        <div class="max-w-[240px]">
          <NSelect v-model:value="themeModeValue" to="body" :options="themeModeOptions" @update-value="handleChangeThemeMode" />
        </div>
      </div>

      <div class="mt-3">
        <div class="mb-1">
          {{ $t('themePackage.label') }}
        </div>
        <div class="max-w-[240px]">
          <NSelect v-model:value="selectedThemePackage" to="body" :options="themePackageOptions" :disabled="themeModeValue === 'follow-space'" @update-value="handleChangeThemePackage" />
        </div>
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="yin-text-muted mb-[5px] font-bold">
        {{ $t('common.icon') }}
      </div>
      <div class="mt-[5px]">
        <div>
          {{ $t('common.style') }}
        </div>
        <div class="flex items-center mt-[5px]">
          <NSelect v-model:value="panelState.panelConfig.iconStyle" to="body" :options="iconTypeOptions" />
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
      <div class="yin-text-muted mb-[5px] font-bold">
        {{ $t('apps.baseSettings.wallpaper') }}
      </div>
      <NSelect v-model:value="panelState.panelConfig.wallpaperMode" to="body" :options="wallpaperModeOptions" class="mb-2" />
      <NButton size="small" class="mb-2" @click="openSettingsApp('WallpaperCenter')">
        {{ $t('themeWallpaper.center') }}
      </NButton>
      <NSelect v-if="panelState.panelConfig.wallpaperMode === 'custom'" v-model:value="panelState.panelConfig.wallpaperKind" to="body" :options="wallpaperKindOptions" class="mb-2" />
      <NUpload
        v-if="panelState.panelConfig.wallpaperMode === 'custom' && (panelState.panelConfig.wallpaperKind === 'image' || panelState.panelConfig.wallpaperKind === 'video')"
        action="/api/file/uploadImg"
        :show-file-list="false"
        name="imgfile"
        :directory-dnd="true"
        :accept="panelState.panelConfig.wallpaperKind === 'video' ? '.mp4,.webm' : '.png,.jpg,.jpeg,.gif,.webp'"
        @finish="handleUploadBackgroundFinish"
      >
        <NUploadDragger style="width: 100%;">
          <div
            class="h-[200px] w-full border bg-[var(--yin-canvas)] flex justify-center items-center cursor-pointer rounded-[10px]"
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

      <!-- 首页所有表面共用一个值：主题的卡片、监控条、图标方块都读它。 -->
      <div class="flex items-center mt-[10px]">
        <span class="mr-[10px]">{{ $t('apps.baseSettings.surfaceOpacity') }}</span>
        <NSlider v-model:value="surfaceOpacity" class="max-w-[200px]" :min="0.4" :max="1" :step="0.02" />
      </div>
    </NCard>

    <NCard style="border-radius:10px" class="mt-[10px]" size="small">
      <div class="mb-2 text-xs opacity-70">
        {{ $t('common.saveToAccountHint') }}
      </div>
      <NPopconfirm
        @positive-click="resetAppearanceConfig"
      >
        <template #trigger>
          <NButton size="small" quaternary type="error">
            {{ $t('apps.appearance.reset') }}
          </NButton>
        </template>
        {{ $t('apps.baseSettings.resetWarnText') }}
      </NPopconfirm>

      <NButton size="small" quaternary type="success" class="ml-[10px]" @click="savePanelConfig">
        {{ $t('common.saveToAccount') }}
      </NButton>
      <span v-if="panelState.panelConfigDirty" class="ml-[10px] text-xs" style="color:var(--yin-primary)">
        {{ $t('common.unsavedChanges') }}
      </span>
    </NCard>
  </div>
</template>

<style scoped>
.text-shadow{
  text-shadow: 0px 0px 5px gray;
}
</style>
