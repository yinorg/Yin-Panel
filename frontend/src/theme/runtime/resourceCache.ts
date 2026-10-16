/**
 * One fetch per theme resource URL per session.
 *
 * A cold home load reads the same entry script twice: `useTheme` warms it as
 * soon as the package resolves (so the download overlaps the grant round trip
 * that has to finish before the host may mount anything), and `ThemeHost` then
 * reads the same bytes to verify their sha256 and mount them. Both go through
 * here, so the second reader awaits the first reader's promise instead of asking
 * the browser for a copy it already has.
 *
 * Only successful reads are kept, and only the most recent few: the entries are
 * a theme's text resources, not its media, so the cache stays small while still
 * covering a visit that switches between a couple of Spaces.
 */
const MAX_CACHED_ENTRIES = 8
const cache = new Map<string, Promise<ArrayBuffer>>()

export function readThemeResource(url: string): Promise<ArrayBuffer> {
  const key = new URL(url, window.location.origin).href
  const cached = cache.get(key)
  if (cached) {
    // Re-inserting keeps the map ordered by use, so the eviction below drops the
    // resource that has gone longest without a reader.
    cache.delete(key)
    cache.set(key, cached)
    return cached
  }

  // The response is served `immutable` and its URL carries the package revision,
  // so a cached copy is correct by construction. `no-store` is deliberately not
  // set: it would make the theme unloadable offline, and freshness does not
  // depend on the cache — the host still checks the bytes against the manifest's
  // sha256 before running anything.
  const pending = fetch(key, { credentials: 'omit', redirect: 'error' }).then((response) => {
    if (!response.ok) throw new Error(`Theme resource request failed: ${key}`)
    return response.arrayBuffer()
  })
  cache.set(key, pending)
  void pending.catch(() => {
    if (cache.get(key) === pending) cache.delete(key)
  })
  while (cache.size > MAX_CACHED_ENTRIES) {
    const oldest = cache.keys().next().value
    if (oldest === undefined) break
    cache.delete(oldest)
  }
  return pending
}

/** Fire-and-forget warm-up; a failure costs nothing but the warm-up. */
export function prefetchThemeResource(url: string) {
  void readThemeResource(url).catch(() => {})
}
