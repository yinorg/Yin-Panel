<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ThemeEnvironment, ThemeHomeSnapshot, ThemePermission } from '../api/v1'
import type { ThemePackage } from '@/utils/theme'
import { mountThemeSandbox, type ThemeSandboxHandle, type ThemeSandboxStylesheet } from './sandbox'
import { mountThemeDirect, type ThemeDirectHandle } from './direct'
import { rewriteThemeStylesheet } from './resources'
import { sha256Hex } from '@/utils/sha256.js'
import { toThemeItemIcon } from '@/core/home/iconifyResource'

/**
 * Mount trusted themes into the document (light DOM) instead of a shadow root
 * (model "theme = app, WordPress-style"): Core components the theme embeds are
 * then styled by the document's own CSS and render pixel-identically. The theme
 * stylesheet is scoped to its container so it cannot reach the Core chrome.
 * Set to false to fall back to the previous shadow-root mount.
 */
const DIRECT_LIGHT_MOUNT = true

const props = defineProps<{
  theme: ThemePackage
  snapshot: ThemeHomeSnapshot
  environment: Omit<ThemeEnvironment, 'apiVersion'>
  permissions: ThemePermission[]
  executionMode?: 'sandbox' | 'trusted'
  /**
   * How the package is mounted, independent of the permission `executionMode`.
   * `direct` mounts it same-origin inside a shadow root of this document (used by
   * the `trusted` route and by first-party built-in themes); `sandbox` keeps the
   * legacy frame. Defaults to `direct` for the trusted execution mode so existing
   * callers that only pass `executionMode` keep working.
   */
  mountMode?: 'sandbox' | 'direct'
  slots: Record<string, string>
  title: string
  execute: (request: unknown) => Promise<unknown>
}>()

const emit = defineEmits<{
  failed: [error: Error]
  ready: []
}>()

const mountMode = computed<'sandbox' | 'direct'>(() => props.mountMode ?? ((props.executionMode || 'sandbox') === 'trusted' ? 'direct' : 'sandbox'))
/** First-party themes ship with the panel. Their implicit permissions resolve
 *  over the API, so on the very first tick the grant may not be in hand yet; the
 *  host remounts when it arrives (the permission list is part of the key), so a
 *  direct-mount built-in must tolerate that gap instead of failing closed. */
const isBuiltinPackage = computed(() => props.theme.manifest.id === 'org.yin.default' || props.theme.manifest.id === 'org.yin.glass')
/**
 * The explicit trusted-route takeover is the fullscreen, frame-replacing mount
 * with its own exit toolbar. A built-in theme mounting directly on the normal
 * home is same-origin too but stays inside the shell, so the Core's monitor layer
 * keeps painting above it exactly as it did over the frame.
 */
const trustedTakeover = computed(() => (props.executionMode || 'sandbox') === 'trusted')

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
  const lightMount = mountMode.value === 'direct' && DIRECT_LIGHT_MOUNT
  const scope = lightMount ? `yin-theme-${Math.random().toString(36).slice(2, 10)}` : ''
  try {
    const manifest = props.theme.manifest
    const scriptPath = manifest.entrypoints?.script
    if (!scriptPath || !manifest.contributes?.views?.includes('home') || !manifest.runtime?.supportedModes?.includes(props.executionMode || 'sandbox'))
      throw new Error(`Theme package does not declare a ${props.executionMode || 'sandbox'} home view`)
    const requiredPermissions = manifest.permissions?.required?.map(item => item.name as ThemePermission) || []
    const missingPermissions = requiredPermissions.filter(permission => !props.permissions.includes(permission))
    // A theme mounted directly runs in this document's own origin, so a missing
    // grant is fatal: there is no isolation to fall back on and the code would run
    // with full page privileges while silently failing every capability.
    // The sandbox is the opposite case. It is a cross-origin frame with no
    // token and no access to this document, and the dispatcher still rejects
    // every command whose permission was not granted — so a missing grant costs
    // the theme capabilities, not safety. Refusing to start would leave the
    // visitor on a public link looking at the Core home instead, and the theme
    // already hides write affordances based on the snapshot's capabilities.
    if (missingPermissions.length && mountMode.value === 'direct' && !isBuiltinPackage.value)
      throw new Error(`Theme API permissions have not been granted: ${missingPermissions.join(', ')}`)

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
      styles.push({ text: await rewriteThemeStylesheet(source, resource.url!, manifest.resources || [], assets, window.location.origin, mountMode.value === 'direct' ? (lightMount ? { prefix: `.${scope}` } : { shadow: true }) : {}) })
    }
    const initialSnapshot = scopedSnapshot(props.snapshot)
    runtimeSnapshot = initialSnapshot
    const initialEnvironment = { ...props.environment, assets }
    const tokens = `:root{${Object.entries(props.slots).filter(([name]) => /^[a-z0-9-]+$/.test(name)).map(([name, value]) => `--yin-${name}:${value}`).join(';')}}`
    const nextRuntime = mountMode.value === 'direct'
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
          takeover: trustedTakeover.value,
          light: lightMount,
          scope,
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
  if (mountMode.value === 'direct' && trustedTakeover.value) {
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
    // Restate the effective set so the theme can never see an authorization the
    // dispatcher would refuse.
    permissions: [...permissions],
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
      // 这个映射是主题真正收到内容的唯一关卡：它逐字段重建 item，没列出的字段到不了
      // 主题。所以 `openMethod` 和 `url` 都必须写在这里——只在上游
      // （`createThemeHomeSnapshot`、`toThemeItem`）加字段是无效的，主题照样拿不到。
      //
      // `url` 是书签地址，主题需要它才能在自己的点击里开窗。
      //
      // 别把这条理解成某种边界：主题框现在是**面板同源**的（`sandbox.ts` 里的
      // `allow-same-origin`，为的是让 DevTools 设备模拟的合成输入能投递进来），所以
      // 主题本来就能直接读面板的 localStorage、cookie 和 DOM。把地址放进快照，是为了
      // 让主题不必去戳面板内部状态就能开窗，而不是为了把地址挡在某个隔离之外 ——
      // 那个隔离已经不存在了。信任前提与代价见 `sandbox.ts`。
      openMethod: Number(item.openMethod) || 1,
      url: typeof item.url === 'string' ? item.url : undefined,
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
  // The response is served `immutable` (see AssetV2) and its URL carries the
  // package revision, so it is cacheable by construction. This deliberately does
  // NOT set `cache: 'no-store'`: that overrode the server's own caching decision
  // and made the theme impossible to load offline, which silently dropped the
  // page back to the C3 fallback list. Freshness still does not depend on the
  // cache — the sha256 check below rejects any byte that is not the exact
  // resource the manifest names.
  const response = await fetch(new URL(resource.url!, window.location.origin), { credentials: 'omit', redirect: 'error' })
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

<!--
  The theme frame carries no `sandbox` attribute on purpose: `mountThemeSandbox`
  sets it, and that is the only place the tokens are defined. Binding it here as
  well looks correct but is dead — the mount overwrites it immediately, which is
  how a token change here can appear to do nothing. See `sandbox.ts`.
-->
<template>
  <div ref="shell" class="theme-runtime-shell">
    <slot />
    <template v-if="mountMode === 'direct'">
      <div ref="trustedHost" class="theme-host theme-host--direct" :class="{ 'theme-host--takeover': trustedTakeover }" :aria-label="title" data-testid="theme-trusted-host" />
      <div v-if="trustedTakeover" class="theme-trusted-toolbar" data-testid="theme-trusted-toolbar">
        <span>{{ title }}</span>
        <button type="button" @click="leaveTrustedRuntime">{{ $t('themeTrustedRuntime.trustedExit') }}</button>
      </div>
    </template>
    <iframe
      v-else
      ref="frame"
      class="theme-host"
      referrerpolicy="no-referrer"
      :title="title"
      :style="{ height: layoutHeight ? `${layoutHeight}px` : '100%' }"
      data-testid="theme-home-frame"
    />
  </div>
</template>

<style scoped>
.theme-runtime-shell { position: absolute; z-index: 1; inset: 0; overflow: auto; overscroll-behavior: contain; background: transparent; pointer-events: auto; }
.theme-host--direct { position: relative; display: block; width: 100%; height: auto; min-height: 100%; }
.theme-host--takeover { position: fixed; inset: 0; z-index: 40; min-height: 100%; }
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
