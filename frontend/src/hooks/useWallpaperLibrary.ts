import { computed, ref } from 'vue'
import { activeThemePackage } from '@/hooks/useTheme'
import i18n from '@/locales'
import { getWallpaperLibrary, type WallpaperLibrary, type WallpaperLibraryCategory, type WallpaperLibraryEntry } from '@/api/panel/wallpaperLibrary'

/**
 * The wallpaper library the picker draws and the wallpaper layer resolves
 * against: the Core's packaged pack plus, when the active theme declares one, the
 * theme's own catalogue.
 *
 * A stored selection is only an id, so the layer must resolve it without the
 * picker ever being opened — which is why both read from here and the request is
 * shared. A Core entry keeps its id; a theme entry is namespaced with its package
 * id, because two themes may reuse a name and the id is what gets persisted.
 */
const coreLibrary = ref<WallpaperLibrary>({ version: '', categories: [] })
const themeCategories = ref<WallpaperLibraryCategory[]>([])
const loading = ref(false)
let pending: Promise<void> | null = null

/** Resolve a catalogue-relative path against the theme's own asset URL. */
function resolveAgainst(value: unknown, base: string): string | null {
  if (typeof value !== 'string' || !value)
    return null
  try {
    return new URL(value, base).href
  }
  catch {
    return null
  }
}

function sameOrigin(url: string): boolean {
  try {
    return new URL(url).origin === window.location.origin
  }
  catch {
    return false
  }
}

/**
 * A catalogue names its own entries, but the name is authored in one language.
 * The locale files carry a `wallpaperCatalogue` map keyed by the same ids, so a
 * name resolves per locale and falls back to the catalogue's own one — which is
 * also what a theme's catalogue gets, since its ids are not in these files.
 */
function localisedName(scope: 'categories' | 'items', id: string, fallback: string): string {
  const key = `wallpaperCatalogue.${scope}.${id}`
  return i18n.global.te(key) ? i18n.global.t(key) : fallback
}

/**
 * One catalogue entry, validated and resolved. A catalogue is theme content, so
 * anything malformed, of an unknown kind, or pointing off-origin is dropped: the
 * layer only ever paints the app's own assets.
 */
function normaliseEntry(raw: unknown, base: string, packageId: string): WallpaperLibraryEntry | null {
  const value = raw as Partial<WallpaperLibraryEntry> | null
  if (!value || typeof value !== 'object')
    return null
  const kind = value.kind === 'page' || value.kind === 'image' ? value.kind : null
  const source = resolveAgainst(value.source, base)
  if (!kind || typeof value.id !== 'string' || !value.id || typeof value.name !== 'string' || !value.name || !source || !sameOrigin(source))
    return null
  const poster = resolveAgainst(value.poster, base)
  return {
    id: packageId ? `${packageId}:${value.id}` : value.id,
    name: localisedName('items', value.id, value.name),
    kind,
    source,
    poster: poster && sameOrigin(poster) ? poster : undefined,
  }
}

function normaliseCategories(raw: unknown, base: string, packageId: string): WallpaperLibraryCategory[] {
  const document = raw as { categories?: unknown } | null
  const categories = Array.isArray(document?.categories) ? document?.categories : []
  const result: WallpaperLibraryCategory[] = []
  for (const candidate of categories as Array<{ id?: unknown; name?: unknown; items?: unknown }>) {
    if (typeof candidate?.id !== 'string' || typeof candidate?.name !== 'string' || !Array.isArray(candidate.items))
      continue
    const items = (candidate.items as unknown[])
      .map(item => normaliseEntry(item, base, packageId))
      .filter((item): item is WallpaperLibraryEntry => !!item)
    if (items.length)
      result.push({ id: candidate.id, name: localisedName('categories', candidate.id, candidate.name), items })
  }
  return result
}

/** The catalogue the active theme declares, if it declares one. */
async function loadThemeCatalogue() {
  themeCategories.value = []
  const pkg = activeThemePackage.value
  const path = pkg?.manifest?.wallpaperCatalogue
  if (!pkg || !path)
    return
  const resource = pkg.manifest.resources?.find(item => item.path === path)
  if (!resource?.url)
    return
  try {
    const response = await fetch(resource.url, { credentials: 'omit' })
    if (!response.ok)
      return
    themeCategories.value = normaliseCategories(await response.json(), resource.url, pkg.manifest.id)
  }
  catch {
    // A theme catalogue that cannot be read simply does not appear: the Core
    // library must stay usable.
    themeCategories.value = []
  }
}

export function useWallpaperLibrary() {
  function load(force = false): Promise<void> {
    if (pending && !force)
      return pending
    if (coreLibrary.value.categories.length && !force) {
      void loadThemeCatalogue()
      return Promise.resolve()
    }
    loading.value = true
    pending = getWallpaperLibrary<WallpaperLibrary>()
      .then((res) => {
        const value = res.code === 0 ? res.data : null
        coreLibrary.value = value?.categories ? value : { version: '', categories: [] }
      })
      .catch(() => {
        // An unreachable library is not fatal: the picker shows its empty state
        // and a stored selection falls back to the theme's own wallpaper.
        if (!coreLibrary.value.categories.length)
          coreLibrary.value = { version: '', categories: [] }
      })
      .then(loadThemeCatalogue)
      .finally(() => {
        loading.value = false
        pending = null
      })
    return pending
  }

  const library = computed<WallpaperLibrary>(() => ({
    version: coreLibrary.value.version,
    categories: [...coreLibrary.value.categories, ...themeCategories.value],
  }))

  function find(id?: string): WallpaperLibraryEntry | null {
    if (!id)
      return null
    for (const category of library.value.categories) {
      for (const entry of category.items) {
        if (entry.id === id)
          return entry
      }
    }
    return null
  }

  return { library, loading, load, find }
}
