<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ThemeEnvironment, ThemeHomeSnapshot, ThemePermission } from '../api/v1'
import type { ThemePackage } from '@/utils/theme'
import { mountThemeSandbox, type ThemeSandboxHandle, type ThemeSandboxStylesheet } from './sandbox'
import { mountThemeDirect, type ThemeDirectHandle } from './direct'
import { rewriteThemeStylesheet } from './resources'
import { sha256Hex } from '@/utils/sha256.js'
import { toThemeItemIcon } from '@/core/home/iconifyResource'

const props = defineProps<{
  theme: ThemePackage
  snapshot: ThemeHomeSnapshot
  environment: Omit<ThemeEnvironment, 'apiVersion'>
  permissions: ThemePermission[]
  executionMode?: 'sandbox' | 'trusted'
  slots: Record<string, string>
  title: string
  execute: (request: unknown) => Promise<unknown>
}>()

const emit = defineEmits<{
  failed: [error: Error]
  ready: []
}>()

const frame = ref<HTMLIFrameElement>()
const shell = ref<HTMLElement>()
const layoutHeight = ref(0)
const trustedHost = ref<HTMLElement>()
let runtime: ThemeSandboxHandle | ThemeDirectHandle | undefined
let disposed = false
let generation = 0
let startTask: Promise<void> | undefined
let activeAssetURLs: string[] = []
let activeAssets: Record<string, string> = {}
onMounted(() => { queueStart() })

watch(() => props.snapshot, (snapshot) => {
  publishSnapshot(scopedSnapshot(snapshot))
}, { deep: true })

watch(() => props.environment, (environment) => {
  runtime?.updateEnvironment({ ...environment, assets: themeAssetURLs(props.theme, activeAssets) })
  runtime?.emit('environment.changed', environment)
}, { deep: true })

watch(() => props.slots, (slots) => {
  const declarations = Object.entries(slots)
    .filter(([name]) => /^[a-z0-9-]+$/.test(name))
    .map(([name, value]) => `--yin-${name}:${value}`)
    .join(';')
  runtime?.updateTokens(`:root{${declarations}}`)
}, { deep: true })

watch(() => `${props.theme.revision || ''}:${props.permissions.join(',')}`, async () => {
  if (disposed) return
  const previous = runtime
  runtime = undefined
  const restartGeneration = ++generation
  await previous?.dispose()
  revokeAssetURLs(activeAssetURLs)
  activeAssetURLs = []
  activeAssets = {}
  await startTask
  if (!disposed && restartGeneration === generation) queueStart()
})

function queueStart() {
  const task = start()
  startTask = task
  void task.finally(() => {
    if (startTask === task) startTask = undefined
  })
}

async function start() {
  const startGeneration = ++generation
  const createdAssetURLs: string[] = []
  try {
    const manifest = props.theme.manifest
    const scriptPath = manifest.entrypoints?.script
    if (!scriptPath || !manifest.contributes?.views?.includes('home') || !manifest.runtime?.supportedModes?.includes(props.executionMode || 'sandbox'))
      throw new Error(`Theme package does not declare a ${props.executionMode || 'sandbox'} home view`)
    const requiredPermissions = manifest.permissions?.required?.map(item => item.name as ThemePermission) || []
    if (requiredPermissions.some(permission => !props.permissions.includes(permission)))
      throw new Error('Theme API permissions have not been granted')

    const script = await loadTextResource(props.theme, scriptPath, 'text/javascript')
    const assets: Record<string, string> = {}
    for (const resource of manifest.resources || []) {
      if (!isThemeMediaResource(resource.mediaType)) continue
      const bytes = await loadResource(props.theme, resource, 48 * 1024 * 1024)
      if (resource.mediaType === 'image/svg+xml') {
        assets[resource.path] = new URL(resource.url!, window.location.origin).href
        continue
      }
      const blob = new Blob([bytes], { type: resource.mediaType })
      const url = URL.createObjectURL(blob)
      createdAssetURLs.push(url)
      assets[resource.path] = url
    }
    const styles: ThemeSandboxStylesheet[] = []
    for (const stylePath of manifest.entrypoints?.styles || []) {
      const resource = findResource(props.theme, stylePath, 'text/css')
      const source = await loadTextResource(props.theme, stylePath, 'text/css')
      styles.push({ text: await rewriteThemeStylesheet(source, resource.url!, manifest.resources || [], assets, window.location.origin, (props.executionMode || 'sandbox') === 'trusted') })
    }
    const initialSnapshot = scopedSnapshot(props.snapshot)
    runtimeSnapshot = initialSnapshot
    const initialEnvironment = { ...props.environment, assets }
    const tokens = `:root{${Object.entries(props.slots).filter(([name]) => /^[a-z0-9-]+$/.test(name)).map(([name, value]) => `--yin-${name}:${value}`).join(';')}}`
    const nextRuntime = (props.executionMode || 'sandbox') === 'trusted'
      ? await mountThemeDirect({
          host: trustedHost.value!,
          script,
          styles,
          tokens,
          snapshot: initialSnapshot,
          environment: initialEnvironment,
          permissions: new Set(props.permissions),
          execute: props.execute,
          onError: error => emit('failed', error),
          contributions: manifest.contributes,
        })
      : await mountThemeSandbox(frame.value!, {
          script,
          styles,
          tokens,
          snapshot: initialSnapshot,
          environment: initialEnvironment,
          permissions: new Set(props.permissions),
          execute: props.execute,
          onError: error => emit('failed', error),
          onLayoutHeight: height => { layoutHeight.value = height },
          contributions: manifest.contributes,
        })
    if (disposed || startGeneration !== generation) {
      await nextRuntime.dispose()
      revokeAssetURLs(createdAssetURLs)
      return
    }
    runtime = nextRuntime
    activeAssetURLs = createdAssetURLs
    activeAssets = assets
    runtimeSnapshot = initialSnapshot
    publishSnapshot(scopedSnapshot(props.snapshot))
    if (JSON.stringify(initialEnvironment) !== JSON.stringify({ ...props.environment, assets: themeAssetURLs(props.theme, activeAssets) })) {
      nextRuntime.updateEnvironment({ ...props.environment, assets: themeAssetURLs(props.theme, activeAssets) })
      nextRuntime.emit('environment.changed', props.environment)
    }
    const currentSlots = Object.entries(props.slots)
      .filter(([name]) => /^[a-z0-9-]+$/.test(name))
      .map(([name, value]) => `--yin-${name}:${value}`)
      .join(';')
    nextRuntime.updateTokens(`:root{${currentSlots}}`)
    emit('ready')
  }
  catch (error) {
    revokeAssetURLs(createdAssetURLs)
    if (!disposed && startGeneration === generation)
      emit('failed', error instanceof Error ? error : new Error(String(error)))
  }
}

let runtimeSnapshot: ThemeHomeSnapshot | undefined

function publishSnapshot(next: ThemeHomeSnapshot) {
  const previous = runtimeSnapshot
  runtimeSnapshot = next
  runtime?.update(next)
  if (!runtime || !previous || previous.version === next.version && JSON.stringify(previous) === JSON.stringify(next)) return
  runtime.emit('state.updated', { status: next.status, version: next.version })
  if (previous.activeSpaceId !== next.activeSpaceId || JSON.stringify(previous.spaces) !== JSON.stringify(next.spaces))
    runtime.emit('space.changed', { activeSpaceId: next.activeSpaceId, spaces: next.spaces })
  if (JSON.stringify(previous.groups) !== JSON.stringify(next.groups))
    runtime.emit('groups.changed', { groups: next.groups })
  if (JSON.stringify(previous.items) !== JSON.stringify(next.items))
    runtime.emit('items.changed', { items: next.items })
}

function scrollToTop() {
  if ((props.executionMode || 'sandbox') === 'trusted') {
    runtime?.scrollToTop()
    return
  }
  shell.value?.scrollTo({ top: 0, behavior: 'smooth' })
}

function themeAssetURLs(theme: ThemePackage, assetURLs: Readonly<Record<string, string>>) {
  const resources = (theme.manifest.resources || []).filter(resource => isThemeMediaResource(resource.mediaType))
  return Object.fromEntries(resources.flatMap(resource => assetURLs[resource.path] ? [[resource.path, assetURLs[resource.path]]] : []))
}

function scopedSnapshot(snapshot: ThemeHomeSnapshot): ThemeHomeSnapshot {
  const permissions = new Set(props.permissions)
  const canReadSpaces = permissions.has('spaces.read')
  const canReadGroups = permissions.has('groups.read')
  const canReadItems = permissions.has('items.read')
  const capabilities = (snapshot.capabilities || []).filter(capability =>
    capability === 'items.write' ? permissions.has('items.write')
      : capability === 'groups.write' && permissions.has('groups.write'))
  return {
    ...snapshot,
    capabilities,
    spaces: canReadSpaces ? snapshot.spaces : [],
    activeSpaceId: canReadSpaces ? snapshot.activeSpaceId : undefined,
    activeSpaceSide: canReadSpaces ? snapshot.activeSpaceSide : undefined,
    activeSpacePairedId: canReadSpaces ? snapshot.activeSpacePairedId : undefined,
    activeSpaceCapabilities: canReadSpaces ? snapshot.activeSpaceCapabilities : [],
    groups: canReadGroups ? snapshot.groups.map(group => ({
      id: String(group.id),
      spaceId: canReadSpaces ? String(group.spaceId) : '',
      parentId: group.parentId === undefined ? undefined : String(group.parentId),
      title: String(group.title),
      icon: group.icon === undefined ? undefined : String(group.icon),
      itemIds: canReadItems ? group.itemIds.map(id => String(id)) : [],
      capabilities: permissions.has('groups.write') && group.capabilities?.includes('groups.write') ? ['groups.write'] : [],
    })) : [],
    items: canReadItems ? snapshot.items.map(item => ({
      id: String(item.id),
      groupId: canReadGroups ? String(item.groupId) : '',
      title: String(item.title),
      description: item.description === undefined ? undefined : String(item.description),
      icon: toThemeItemIcon(item.icon),
      sort: Number(item.sort) || 0,
      capabilities: item.capabilities.map(capability => String(capability)),
    })) : [],
  }
}

function findResource(theme: ThemePackage, path: string, mediaType: string) {
  const resource = theme.manifest.resources?.find(item => item.path === path)
  if (!resource || resource.mediaType !== mediaType || !resource.url)
    throw new Error(`Theme resource is missing or has an invalid type: ${path}`)
  validateResourceURL(resource.url)
  return resource
}

async function loadTextResource(theme: ThemePackage, path: string, mediaType: string) {
  const resource = findResource(theme, path, mediaType)
  const bytes = await loadResource(theme, resource, 900_000)
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
}

async function loadResource(theme: ThemePackage, resource: NonNullable<ThemePackage['manifest']['resources']>[number], maxBytes: number) {
  validateResourceURL(resource.url!)
  const response = await fetch(new URL(resource.url!, window.location.origin), { credentials: 'omit', cache: 'no-store', redirect: 'error' })
  if (!response.ok) throw new Error(`Theme resource request failed: ${resource.path}`)
  const bytes = await response.arrayBuffer()
  if (bytes.byteLength > maxBytes) throw new Error(`Theme resource exceeds the runtime size limit: ${resource.path}`)
  const actual = await sha256Hex(bytes)
  if (actual.toLowerCase() !== resource.sha256.toLowerCase()) throw new Error(`Theme resource integrity check failed: ${resource.path}`)
  return bytes
}

function isThemeMediaResource(mediaType: string) {
  return mediaType.startsWith('image/') || mediaType === 'font/woff2' || mediaType.startsWith('video/')
}

function revokeAssetURLs(urls: readonly string[]) {
  for (const url of urls) URL.revokeObjectURL(url)
}

function validateResourceURL(value: string) {
  const url = new URL(value, window.location.origin)
  if (url.origin !== window.location.origin || url.username || url.password || !url.pathname.startsWith('/api/theme/'))
    throw new Error('Theme resources must use a same-origin immutable theme asset URL')
}

function leaveTrustedRuntime() {
  window.location.replace('/')
}

onBeforeUnmount(() => {
  disposed = true
  generation += 1
  void runtime?.dispose()
  revokeAssetURLs(activeAssetURLs)
  activeAssetURLs = []
  activeAssets = {}
})

defineExpose({ scrollToTop })
</script>

<template>
  <div ref="shell" class="theme-runtime-shell">
    <slot />
    <template v-if="(executionMode || 'sandbox') === 'trusted'">
      <div ref="trustedHost" class="theme-host theme-host--trusted" :aria-label="title" data-testid="theme-trusted-host" />
      <div class="theme-trusted-toolbar" data-testid="theme-trusted-toolbar">
        <span>{{ title }}</span>
        <button type="button" @click="leaveTrustedRuntime">{{ $t('themeTrustedRuntime.trustedExit') }}</button>
      </div>
    </template>
    <iframe
      v-else
      ref="frame"
      class="theme-host"
      sandbox="allow-scripts"
      referrerpolicy="no-referrer"
      :title="title"
      :style="{ height: layoutHeight ? `${layoutHeight}px` : '100%' }"
      data-testid="theme-home-frame"
    />
  </div>
</template>

<style scoped>
.theme-runtime-shell { position: absolute; z-index: 1; inset: 0; overflow: auto; overscroll-behavior: contain; background: transparent; pointer-events: auto; }
.theme-host--trusted { position: fixed; inset: 0; z-index: 40; }
.theme-trusted-toolbar { position: fixed; z-index: 50; top: 8px; right: 8px; display: flex; align-items: center; gap: var(--yin-spaceSm); max-width: calc(100vw - 16px); padding: var(--yin-spaceXs) var(--yin-spaceSm); border: var(--yin-borderWidth) solid var(--yin-border); border-radius: var(--yin-component-button-radius); background: var(--yin-surfaceElevated); color: var(--yin-text); font: var(--yin-fontBodyWeight) var(--yin-fontSmallSize)/var(--yin-lineHeightBody) var(--yin-fontBody); box-shadow: var(--yin-shadowPopup); }
.theme-trusted-toolbar span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.theme-trusted-toolbar button { min-height: 44px; padding: 0 var(--yin-spaceSm); border: 0; border-radius: var(--yin-component-button-radius); background: var(--yin-danger); color: var(--yin-onPrimary); font: inherit; cursor: pointer; }
.theme-trusted-toolbar button:focus-visible { outline: var(--yin-effect-focus-width) solid var(--yin-focusRing); outline-offset: 2px; }
</style>

<style scoped>
.theme-host {
  display: block;
  width: 100%;
  height: 100%;
  min-height: 100%;
  border: 0;
  background: transparent;
}
</style>
