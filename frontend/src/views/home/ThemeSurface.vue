<script setup lang="ts">
import { computed, inject, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import type { useHomeShell } from '@/core/home/useHomeShell'

/**
 * A theme-contributed surface rendered on its own route (`/theme/:view`).
 *
 * The Core keeps the theme's runtime mounted on the home (it owns the theme's
 * `setup` and its data channel); this route mounts one of the theme's other views
 * into that same runtime. The Core renders no page of its own here: when the theme
 * does not contribute the view, the visitor is returned to the home.
 *
 * Whether the theme contributes the view is read from the package manifest, which
 * settles as soon as the package loads. The runtime handle only appears after an
 * async mount, so `themeRuntimeReady` is what re-triggers the mount.
 *
 * Mounting is serialized and skips a view that is already mounted: several watchers
 * can fire in the same tick, and concurrent runs would tear down each other's view
 * from the shared host element.
 */
const route = useRoute()
const router = useRouter()
const shell = inject<ReturnType<typeof useHomeShell>>('yin-home-shell')

const host = ref<HTMLElement>()
let mounted: { unmount: () => void | Promise<void> } | undefined
let mountedName = ''
let running = false
let rerun = false

const view = computed(() => String(route.params.view || ''))
const contributes = computed(() => {
  const views = shell?.theme.themeRuntimePackage?.manifest.contributes?.views as readonly string[] | undefined
  return views?.includes(view.value) === true
})
/** The theme decision has settled: it is either rendering or has fallen back. */
const settled = computed(() => shell?.theme.themeRuntimeActive === true || shell?.theme.homeFallbackVisible === true)

async function teardown() {
  const current = mounted
  mounted = undefined
  mountedName = ''
  if (current)
    await Promise.resolve(current.unmount()).catch(() => undefined)
  clearScopeClass()
}

/** Drop the theme's light-DOM scope class so a remount with a new runtime does not
 *  leave the previous theme's styles on this element. */
function clearScopeClass() {
  if (!host.value)
    return
  for (const cls of [...host.value.classList]) {
    if (cls.startsWith('yin-theme-'))
      host.value.classList.remove(cls)
  }
}

async function sync() {
  const name = view.value
  if (!name || !shell)
    return
  // The theme is settled and does not contribute this view: the Core has no page to
  // show, so fall back to the home.
  if (!contributes.value) {
    if (settled.value)
      await router.replace('/')
    return
  }
  // The theme is expected to render it, but it fell back (safe mode, load failure):
  // there is nothing for the Core to show here either.
  if (settled.value && shell.theme.themeRuntimeActive !== true) {
    await router.replace('/')
    return
  }
  // Wait for the runtime handle; `schedule` re-runs when `ready` flips.
  if (!shell.theme.themeRuntimeReady || !host.value || !shell.theme.hasThemeSurface(name))
    return
  if (mounted && mountedName === name)
    return
  await teardown()
  // Scope the theme's own CSS (and its tokens) onto this page, exactly as the home
  // host does, so the theme can style its page with the stylesheet it shipped.
  const scope = shell.theme.themeScopeClass()
  if (scope)
    host.value.classList.add(scope)
  try {
    mounted = await shell.theme.mountThemeSurface(name, host.value)
    mountedName = name
  }
  catch {
    await router.replace('/')
  }
}

/** Run `sync` one at a time; coalesce the watchers that fire in the same tick. */
async function schedule() {
  if (running) {
    rerun = true
    return
  }
  running = true
  try {
    do {
      rerun = false
      await sync()
    } while (rerun)
  }
  finally {
    running = false
  }
}

watch(
  [view, contributes, settled, () => shell?.theme.themeRuntimeReady, () => shell?.theme.themeRuntimeActive, () => shell?.theme.hasThemeSurface(view.value)],
  () => { void schedule() },
  { immediate: true },
)
watch(host, () => { void schedule() })
onMounted(() => { void schedule() })

onBeforeUnmount(() => { void teardown() })
</script>

<template>
  <div class="theme-surface-page" data-testid="theme-surface-page">
    <div ref="host" class="theme-surface-page-host" data-testid="theme-surface-page-host" />
  </div>
</template>

<style scoped>
.theme-surface-page { position: absolute; z-index: 3; inset: 0; overflow: auto; background: var(--yin-canvas, #f4f7f8); }
.theme-surface-page-host { min-height: 100%; }
</style>
