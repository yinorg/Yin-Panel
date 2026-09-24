<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { ThemeEnvironment, ThemeHomeSnapshot, ThemePermission } from '../api/v1'
import type { ThemePackage } from '@/utils/theme'
import { mountThemeSandbox, type ThemeSandboxHandle, type ThemeSandboxStylesheet } from './sandbox'

const props = defineProps<{
  theme: ThemePackage
  snapshot: ThemeHomeSnapshot
  environment: Omit<ThemeEnvironment, 'apiVersion'>
  permissions: ThemePermission[]
  slots: Record<string, string>
  title: string
  execute(request: unknown): Promise<unknown>
}>()

const emit = defineEmits<{
  failed: [error: Error]
  ready: []
}>()

const frame = ref<HTMLIFrameElement>()
let runtime: ThemeSandboxHandle | undefined
let disposed = false
let generation = 0

onMounted(() => { void start() })

watch(() => props.snapshot, (snapshot) => {
  runtime?.update(scopedSnapshot(snapshot))
}, { deep: true })

watch(() => props.slots, (slots) => {
  const declarations = Object.entries(slots)
    .filter(([name]) => /^[a-z0-9-]+$/.test(name))
    .map(([name, value]) => `--yin-${name}:${value}`)
    .join(';')
  runtime?.updateTokens(`:root{${declarations}}`)
}, { deep: true })

watch(() => `${props.theme.revision || ''}:${props.permissions.join(',')}`, async () => {
  if (!runtime || disposed) return
  const previous = runtime
  runtime = undefined
  generation += 1
  await previous.dispose()
  if (!disposed) void start()
})

async function start() {
  const startGeneration = ++generation
  try {
    const manifest = props.theme.manifest
    const scriptPath = manifest.entrypoints?.script
    if (!scriptPath || !manifest.contributes?.views?.includes('home') || !manifest.runtime?.supportedModes?.includes('sandbox'))
      throw new Error('Theme package does not declare a sandbox home view')
    const requiredPermissions = manifest.permissions?.required?.map(item => item.name as ThemePermission) || []
    if (requiredPermissions.some(permission => !props.permissions.includes(permission)))
      throw new Error('Theme API permissions have not been granted')

    const script = await loadTextResource(props.theme, scriptPath, 'text/javascript')
    const styles: ThemeSandboxStylesheet[] = []
    for (const stylePath of manifest.entrypoints?.styles || []) {
      const resource = findResource(props.theme, stylePath, 'text/css')
      styles.push({ href: resource.url!, text: await loadTextResource(props.theme, stylePath, 'text/css') })
    }
    const assets = Object.fromEntries((manifest.resources || []).filter(resource => resource.url).map((resource) => {
      validateResourceURL(resource.url!)
      return [resource.path, new URL(resource.url!, window.location.origin).href]
    }))
    const tokens = `:root{${Object.entries(props.slots).filter(([name]) => /^[a-z0-9-]+$/.test(name)).map(([name, value]) => `--yin-${name}:${value}`).join(';')}}`
    const nextRuntime = await mountThemeSandbox(frame.value!, {
      script,
      styles,
      tokens,
      snapshot: scopedSnapshot(props.snapshot),
      environment: { ...props.environment, assets },
      permissions: new Set(props.permissions),
      execute: props.execute,
      onError: (error) => emit('failed', error),
    })
    if (disposed || startGeneration !== generation) {
      await nextRuntime.dispose()
      return
    }
    runtime = nextRuntime
    emit('ready')
  }
  catch (error) {
    if (!disposed && startGeneration === generation)
      emit('failed', error instanceof Error ? error : new Error(String(error)))
  }
}

function scopedSnapshot(snapshot: ThemeHomeSnapshot): ThemeHomeSnapshot {
  const permissions = new Set(props.permissions)
  const canReadSpaces = permissions.has('spaces.read')
  const canReadGroups = permissions.has('groups.read')
  const canReadItems = permissions.has('items.read')
  return {
    ...snapshot,
    spaces: canReadSpaces ? snapshot.spaces : [],
    activeSpaceId: canReadSpaces ? snapshot.activeSpaceId : undefined,
    groups: canReadGroups ? snapshot.groups.map(group => ({ ...group, spaceId: canReadSpaces ? group.spaceId : '', itemIds: canReadItems ? group.itemIds : [] })) : [],
    items: canReadItems ? snapshot.items.map(item => ({ ...item, groupId: canReadGroups ? item.groupId : '' })) : [],
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
  const response = await fetch(new URL(resource.url!, window.location.origin), { credentials: 'omit', cache: 'no-store', redirect: 'error' })
  if (!response.ok) throw new Error(`Theme resource request failed: ${path}`)
  const bytes = await response.arrayBuffer()
  if (bytes.byteLength > 900_000) throw new Error(`Theme entrypoint exceeds the current runtime message limit: ${path}`)
  const digest = await crypto.subtle.digest('SHA-256', bytes)
  const actual = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
  if (actual.toLowerCase() !== resource.sha256.toLowerCase()) throw new Error(`Theme resource integrity check failed: ${path}`)
  return new TextDecoder('utf-8', { fatal: true }).decode(bytes)
}

function validateResourceURL(value: string) {
  const url = new URL(value, window.location.origin)
  if (url.origin !== window.location.origin || url.username || url.password || !url.pathname.startsWith('/api/theme/'))
    throw new Error('Theme resources must use a same-origin immutable theme asset URL')
}

onBeforeUnmount(() => {
  disposed = true
  generation += 1
  void runtime?.dispose()
})
</script>

<template>
  <iframe
    ref="frame"
    class="theme-host"
    sandbox="allow-scripts"
    referrerpolicy="no-referrer"
    :title="title"
    data-testid="theme-home-frame"
  />
</template>

<style scoped>
.theme-host {
  display: block;
  width: 100%;
  height: 100%;
  min-height: 100%;
  border: 0;
  background: var(--yin-canvas);
}
</style>
