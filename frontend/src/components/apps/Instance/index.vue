<script setup lang="ts">
import { NButton, NCard, NModal, NSelect, NSwitch, useMessage } from 'naive-ui'
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useAppStore, useAuthStore } from '@/store'
import { t } from '@/locales'
import { beginThemeTrial, confirmThemeTrial, getThemePackages, getThemeRevision, getTrustedThemeRuntimePolicy, installThemePackage, previewThemePackage, removeThemePackage, rollbackThemeTrial, setTrustedThemeRuntimePolicy } from '@/api/theme'
import { isBuiltinThemePackage, type ThemePackage } from '@/utils/theme'
import { activeThemePackageId, refreshMyTheme } from '@/hooks/useTheme'
import { useConfirmAction } from '@/hooks/useConfirmAction'

const authStore = useAuthStore()
const appStore = useAppStore()
const ms = useMessage()
const { confirmAction } = useConfirmAction()

const isAdmin = computed(() => authStore.userInfo?.role === 1)
const themeFileInput = ref<HTMLInputElement | null>(null)
const themePackages = ref<Array<{ id: string; name: string; version: string; revision: string; verified: boolean }>>([])
const themeRevisions = ref<Array<{ id: string; packageId: string; version: string; verified: boolean; createdAt: string }>>([])
const defaultThemeId = ref('')
const lastGoodThemeRevision = ref('')
const pendingThemeRevision = ref('')
const trialStartedAt = ref<string | null>(null)
const selectedTrialRevision = ref('')
const selectedTrustedRevision = ref('')
const trustedThemeSupported = ref(false)
const trustedPolicyEnabled = ref(false)
const trustedPolicyLoading = ref(false)
const trustedPolicySaving = ref(false)
const trialNow = ref(Date.now())
const themeSaving = ref(false)
const previewFile = ref<File | null>(null)
const previewToken = ref('')
const previewPackage = ref<ThemePackage | null>(null)
const previewVisible = ref(false)
const previewPage = ref<'home' | 'login'>('home')
const previewDevice = ref<'desktop' | 'mobile'>('desktop')
const previewMode = ref<'light' | 'dark'>('light')
const previewURL = computed(() => `${previewPage.value === 'login' ? '/login' : '/'}?themePreview=${encodeURIComponent(previewToken.value)}&themePreviewMode=${previewMode.value}`)
const previewRemoteDomains = computed(() => Object.values(previewPackage.value?.manifest.wallpapers || {})
  .filter(item => item.kind === 'externalUrl' || item.kind === 'imageUrl')
  .map(item => new URL(item.source).hostname))

const themeOptions = computed(() => themePackages.value.map(item => ({ label: `${item.name} (${item.version})`, value: item.id })))
const trialRevisionOptions = computed(() => themeRevisions.value
  .filter(revision => revision.packageId === defaultThemeId.value)
  .map(revision => ({ label: `${revision.version} · ${revision.id.slice(0, 12)}`, value: revision.id })))
const trustedRevisionOptions = computed(() => themeRevisions.value
  .map(revision => ({ label: `${themePackages.value.find(item => item.id === revision.packageId)?.name || revision.packageId} · ${revision.version} · ${revision.id.slice(0, 12)}`, value: revision.id })))
const trialRevisionSelected = computed(() => trialRevisionOptions.value.some(option => option.value === selectedTrialRevision.value))
const trialSecondsLeft = computed(() => {
  if (!pendingThemeRevision.value || !trialStartedAt.value) return 0
  return Math.max(0, 30 - Math.floor((trialNow.value - new Date(trialStartedAt.value).getTime()) / 1000))
})
let trialTicker: number | undefined
let trustedRevisionGeneration = 0
onBeforeUnmount(() => {
  if (trialTicker !== undefined) window.clearInterval(trialTicker)
})
watch(defaultThemeId, (packageId) => {
  selectedTrialRevision.value = themePackages.value.find(item => item.id === packageId)?.revision || ''
})

async function loadThemePackages() {
  if (!isAdmin.value) return
  const { code, data } = await getThemePackages()
  if (code === 0) {
    themePackages.value = data.packages
    themeRevisions.value = data.revisions || []
    defaultThemeId.value = data.defaultPackage
    lastGoodThemeRevision.value = data.lastGoodRevision
    pendingThemeRevision.value = data.pendingRevision
    trialStartedAt.value = data.trialStartedAt || null
    if (!selectedTrialRevision.value || !themeRevisions.value.some(revision => revision.id === selectedTrialRevision.value && revision.packageId === defaultThemeId.value)) {
      selectedTrialRevision.value = themePackages.value.find(item => item.id === defaultThemeId.value)?.revision || ''
    }
    if (!selectedTrustedRevision.value || !themeRevisions.value.some(revision => revision.id === selectedTrustedRevision.value))
      selectedTrustedRevision.value = themeRevisions.value[0]?.id || ''
  }
}

async function loadTrustedRuntimePolicy(revisionId: string) {
  const generation = ++trustedRevisionGeneration
  trustedPolicyEnabled.value = false
  trustedThemeSupported.value = false
  if (!revisionId) return
  trustedPolicyLoading.value = true
  try {
    const [themeResult, policyResult] = await Promise.all([getThemeRevision(revisionId), getTrustedThemeRuntimePolicy(revisionId)])
    if (generation !== trustedRevisionGeneration) return
    trustedThemeSupported.value = themeResult.code === 0 && themeResult.data.manifest.runtime?.supportedModes?.includes('trusted') === true
    trustedPolicyEnabled.value = policyResult.code === 0 && policyResult.data.enabled
  }
  catch {
    if (generation === trustedRevisionGeneration) {
      trustedThemeSupported.value = false
      trustedPolicyEnabled.value = false
    }
  }
  finally {
    if (generation === trustedRevisionGeneration) trustedPolicyLoading.value = false
  }
}

watch(selectedTrustedRevision, revisionId => { void loadTrustedRuntimePolicy(revisionId) })

async function updateTrustedRuntimePolicy(enabled: boolean) {
  if (!selectedTrustedRevision.value || trustedPolicySaving.value || enabled && !trustedThemeSupported.value) return
  trustedPolicySaving.value = true
  try {
    const result = await setTrustedThemeRuntimePolicy(selectedTrustedRevision.value, enabled)
    if (result.code === 0) {
      trustedPolicyEnabled.value = enabled
      ms.success(t(enabled ? 'themeTrustedRuntime.enabled' : 'themeTrustedRuntime.disabled'))
    }
    else ms.error(result.msg)
  }
  catch {
    ms.error(t('themeTrustedRuntime.policyFailed'))
  }
  finally {
    trustedPolicySaving.value = false
  }
}

async function uploadThemePackage(file?: File, confirmed = false): Promise<void> {
  if (!file) return
  const result = await installThemePackage(file, confirmed)
  if (result.code !== 0 && !confirmed && /unverified|unsigned/i.test(result.msg)) {
    if (await confirmAction({ content: t('themePackage.confirmUnverified'), positiveText: t('themePackage.install') }))
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
  if (!selectedTrialRevision.value) return
  themeSaving.value = true
  try {
    const detail = await getThemeRevision(selectedTrialRevision.value)
    if (detail.code !== 0) {
      ms.error(t('themeWallpaper.loadFailed'))
      return
    }
    const hosts = Object.values(detail.data.manifest.wallpapers || {})
      .filter(item => item.kind === 'externalUrl' || item.kind === 'imageUrl')
      .map(item => new URL(item.source).hostname)
    if (hosts.length && !(await confirmAction({ content: t('themeWallpaper.confirmExternal', { hosts: [...new Set(hosts)].join(', ') }) })))
      return
    const { code, msg } = await beginThemeTrial(selectedTrialRevision.value)
    if (code === 0) {
      ms.success(t('themeTrial.started'))
      await refreshMyTheme(appStore)
      await loadThemePackages()
    }
    else ms.error(msg)
  }
  finally {
    themeSaving.value = false
  }
}

async function confirmThemeTrialSelection() {
  if (!pendingThemeRevision.value) return
  const { code, msg } = await confirmThemeTrial(pendingThemeRevision.value)
  if (code === 0) {
    ms.success(t('themeTrial.confirmed'))
    await refreshMyTheme(appStore)
    await loadThemePackages()
  }
  else ms.error(msg)
}

async function rollbackThemeSelection() {
  const { code, msg } = await rollbackThemeTrial()
  if (code === 0) {
    ms.success(t('themeTrial.rolledBack'))
    await refreshMyTheme(appStore)
    await loadThemePackages()
  }
  else ms.error(msg)
}

async function deleteTheme(id: string) {
  if (!(await confirmAction({ content: t('themePackage.removeConfirm'), positiveText: t('common.delete'), variant: 'error' })))
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

onMounted(async () => {
  trialTicker = window.setInterval(() => { trialNow.value = Date.now() }, 1000)
  await loadThemePackages()
})
</script>

<template>
  <div v-if="isAdmin" class="theme-page rounded-[10px] p-[8px] overflow-auto">
    <NCard size="small">
      <div class="yin-text-muted mb-1 font-bold">{{ $t('themePackage.title') }}</div>
      <div class="mb-3 text-xs opacity-70">{{ $t('apps.instance.scopeHint') }}</div>
      <div class="flex flex-wrap items-center gap-2">
        <input ref="themeFileInput" type="file" accept=".yin-theme,.zip" class="hidden" @change="prepareThemePackage(($event.target as HTMLInputElement).files?.[0]); ($event.target as HTMLInputElement).value = ''">
        <NButton size="small" @click="themeFileInput?.click()">{{ $t('themePackage.install') }}</NButton>
        <NSelect v-model:value="defaultThemeId" to="body" :options="themeOptions" class="min-w-[220px] flex-1" />
        <NButton size="small" type="primary" :loading="themeSaving" :disabled="!trialRevisionSelected" @click="saveDefaultTheme">{{ $t('themeTrial.start') }}</NButton>
      </div>
      <div class="mt-3 flex flex-wrap items-center gap-2">
        <NSelect v-model:value="selectedTrialRevision" to="body" :options="trialRevisionOptions" class="min-w-[260px] flex-1" :placeholder="$t('themeTrial.selectRevision')" />
        <NButton size="small" type="success" :disabled="!pendingThemeRevision || trialSecondsLeft <= 0" @click="confirmThemeTrialSelection">{{ $t('themeTrial.confirm') }}</NButton>
        <NButton size="small" type="warning" :disabled="!pendingThemeRevision" @click="rollbackThemeSelection">{{ $t('themeTrial.rollback') }}</NButton>
      </div>
      <div v-if="pendingThemeRevision" class="mt-2 text-sm" role="status" data-testid="theme-trial-status">
        {{ $t('themeTrial.pending', { revision: pendingThemeRevision.slice(0, 12), lastGood: lastGoodThemeRevision.slice(0, 12), time: `${Math.floor(trialSecondsLeft / 60)}:${String(trialSecondsLeft % 60).padStart(2, '0')}` }) }}
      </div>
      <div class="mt-3 divide-y">
        <div v-for="item in themePackages" :key="item.id" class="flex items-center justify-between gap-3 py-2">
          <span class="min-w-0 truncate">{{ item.name }} <span class="yin-text-muted">{{ item.id }} · {{ item.version }} · {{ item.verified ? $t('themePackage.verified') : $t('themePackage.unverified') }}</span></span>
          <NButton v-if="!isBuiltinThemePackage(item.id)" size="tiny" tertiary type="error" @click="deleteTheme(item.id)">{{ $t('common.delete') }}</NButton>
        </div>
      </div>
      <section class="mt-4 border-t pt-3" aria-labelledby="trusted-runtime-heading" data-testid="theme-trusted-policy">
        <h3 id="trusted-runtime-heading" class="mb-1 text-sm font-semibold">{{ $t('themeTrustedRuntime.policyTitle') }}</h3>
        <p class="mb-3 text-sm opacity-75">{{ $t('themeTrustedRuntime.policyDescription') }}</p>
        <NSelect v-model:value="selectedTrustedRevision" to="body" :options="trustedRevisionOptions" :placeholder="$t('themeTrustedRuntime.selectRevision')" />
        <div class="mt-3 flex min-h-11 flex-wrap items-center justify-between gap-3">
          <span class="min-w-0 flex-1">{{ trustedThemeSupported ? $t('themeTrustedRuntime.policySupported') : $t('themeTrustedRuntime.policyUnsupported') }}</span>
          <NSwitch
            :value="trustedPolicyEnabled"
            :loading="trustedPolicyLoading || trustedPolicySaving"
            :disabled="!selectedTrustedRevision || !trustedThemeSupported || trustedPolicyLoading || trustedPolicySaving"
            :aria-label="$t('themeTrustedRuntime.policyTitle')"
            data-testid="theme-trusted-policy-switch"
            @update:value="updateTrustedRuntimePolicy"
          />
        </div>
      </section>
    </NCard>

    <NModal v-model:show="previewVisible" preset="card" :title="$t('themeWallpaper.preview')" style="width: min(96vw, 1280px)">
      <div class="flex flex-wrap items-center gap-2 mb-3">
        <span>{{ previewPackage?.manifest.name }} · {{ previewPackage?.manifest.packageVersion }} · {{ previewPackage?.verified ? $t('themePackage.verified') : $t('themePackage.unverified') }}</span>
        <NSelect v-model:value="previewPage" to="body" :options="[{ label: $t('themeWallpaper.home'), value: 'home' }, { label: $t('themeWallpaper.login'), value: 'login' }]" style="width: 130px" />
        <NSelect v-model:value="previewDevice" to="body" :options="[{ label: $t('themeWallpaper.desktop'), value: 'desktop' }, { label: $t('themeWallpaper.mobile'), value: 'mobile' }]" style="width: 130px" />
        <NSelect v-model:value="previewMode" to="body" :options="[{ label: $t('themeWallpaper.light'), value: 'light' }, { label: $t('themeWallpaper.dark'), value: 'dark' }]" style="width: 110px" />
        <NButton type="primary" @click="uploadThemePackage(previewFile || undefined)">{{ $t('themePackage.install') }}</NButton>
      </div>
      <div v-if="previewRemoteDomains.length" class="mb-2 text-sm">
        {{ $t('themeWallpaper.remoteDomain') }}: {{ previewRemoteDomains.join(', ') }}
      </div>
      <div class="flex justify-center overflow-auto bg-neutral-800 p-2">
        <iframe :key="previewURL + previewDevice" :src="previewURL" :title="$t('themeWallpaper.preview')" :style="{ width: previewDevice === 'mobile' ? '390px' : '100%', height: 'min(70vh, 700px)', border: '0', background: '#fff', flexShrink: 0 }" />
      </div>
    </NModal>
  </div>
</template>
