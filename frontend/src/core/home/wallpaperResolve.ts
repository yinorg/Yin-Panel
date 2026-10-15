/**
 * Which wallpaper the home paints, as a pure decision so it can be tested
 * without a browser.
 *
 * Precedence, highest first:
 *   1. `previewThemeDefaults` (`?themePreview`) — the theme's own artwork.
 *   2. The active Space's default wallpaper. A Space's look is the Space's, the
 *      same way its theme is, so it outranks the visitor's global choice.
 *   3. The visitor's own wallpaper (library, custom, or an explicit `none`).
 *   4. The active theme's own wallpaper, then the packaged fallback.
 */

export interface WallpaperChoice {
  kind: string
  source: string
  poster: string
  overlayOpacity?: number
}

export interface WallpaperLibraryEntry {
  kind: string
  source: string
  poster?: string
}

export interface PanelWallpaperConfig {
  wallpaperMode?: string
  wallpaperKind?: string
  wallpaperSource?: string
  wallpaperPoster?: string
  wallpaperLibraryEntry?: string
  backgroundImageSrc?: string
}

export interface SpaceWallpaperConfig {
  mode?: string
  kind?: string
  source?: string
  poster?: string
  libraryEntry?: string
}

export interface WallpaperResolveInput {
  panelConfig: PanelWallpaperConfig
  /** The active Space's default, already read from the server. */
  spaceWallpaper?: SpaceWallpaperConfig | null
  /** Resolves a library id, or undefined when the pack no longer carries it. */
  findLibraryEntry: (id?: string) => WallpaperLibraryEntry | undefined
  /** The active theme's own wallpaper, when it declares one. */
  themeWallpaper?: WallpaperChoice | null
  previewThemeDefaults?: boolean
}

const FALLBACK: WallpaperChoice = { kind: 'image', source: '/assets/bg-forest.webp', poster: '/assets/bg-forest.webp', overlayOpacity: 0 }

/** A custom source, with the poster rule the pre-theme build used. */
function customChoice(kind: string, source: string, poster?: string): WallpaperChoice {
  const isImage = kind === 'image'
  return {
    kind,
    source,
    poster: poster || (isImage && !/\.gif(?:[?#]|$)/i.test(source) ? source : FALLBACK.poster),
    overlayOpacity: undefined,
  }
}

export function resolveWallpaper(input: WallpaperResolveInput): WallpaperChoice | null {
  const { panelConfig, spaceWallpaper, findLibraryEntry, themeWallpaper, previewThemeDefaults } = input

  if (!previewThemeDefaults && spaceWallpaper && spaceWallpaper.mode && spaceWallpaper.mode !== 'theme') {
    if (spaceWallpaper.mode === 'library') {
      const entry = findLibraryEntry(spaceWallpaper.libraryEntry)
      // An entry the library no longer carries falls through to the visitor's
      // own choice rather than leaving the page bare.
      if (entry)
        return { kind: entry.kind, source: entry.source, poster: entry.poster || entry.source, overlayOpacity: 0 }
    }
    if (spaceWallpaper.mode === 'custom' && spaceWallpaper.source)
      return customChoice(spaceWallpaper.kind || 'image', spaceWallpaper.source, spaceWallpaper.poster)
  }

  if (!previewThemeDefaults && panelConfig.wallpaperMode === 'none')
    return null
  if (!previewThemeDefaults && panelConfig.wallpaperMode === 'library') {
    const entry = findLibraryEntry(panelConfig.wallpaperLibraryEntry)
    if (entry)
      return { kind: entry.kind, source: entry.source, poster: entry.poster || entry.source, overlayOpacity: 0 }
  }
  if (!previewThemeDefaults && (panelConfig.wallpaperMode === 'custom' || (!panelConfig.wallpaperMode && panelConfig.backgroundImageSrc)))
    return customChoice(panelConfig.wallpaperKind || 'image', panelConfig.wallpaperSource || panelConfig.backgroundImageSrc || FALLBACK.source, panelConfig.wallpaperPoster)

  return themeWallpaper || FALLBACK
}
