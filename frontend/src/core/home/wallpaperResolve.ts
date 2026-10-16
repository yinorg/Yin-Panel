/**
 * Which wallpaper the home paints, as a pure decision so it can be tested
 * without a browser.
 *
 * Precedence, highest first:
 *   1. `previewThemeDefaults` (`?themePreview`) — the theme's own artwork.
 *   2. The visitor's own choice. Someone who has picked a wallpaper must see it:
 *      a Space's default is a default, so it never displaces a choice the visitor
 *      made for themselves, and their picker stays instant.
 *   3. The active Space's default wallpaper — the Space's look, standing for
 *      everyone who has not named one of their own (unset, or `theme`).
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

/** A library id resolved against the pack, or undefined when it is not carried. */
function libraryChoice(entry: WallpaperLibraryEntry | undefined): WallpaperChoice | undefined {
  if (!entry)
    return undefined
  return { kind: entry.kind, source: entry.source, poster: entry.poster || entry.source, overlayOpacity: 0 }
}

export function resolveWallpaper(input: WallpaperResolveInput): WallpaperChoice | null {
  const { panelConfig, spaceWallpaper, findLibraryEntry, themeWallpaper, previewThemeDefaults } = input

  if (previewThemeDefaults)
    return themeWallpaper || FALLBACK

  // The visitor's own wallpaper first: someone who picked one sees it, whatever
  // the Space they are in says.
  if (panelConfig.wallpaperMode === 'none')
    return null
  if (panelConfig.wallpaperMode === 'library') {
    const choice = libraryChoice(findLibraryEntry(panelConfig.wallpaperLibraryEntry))
    if (choice)
      return choice
  }
  if (panelConfig.wallpaperMode === 'custom' || (!panelConfig.wallpaperMode && panelConfig.backgroundImageSrc))
    return customChoice(panelConfig.wallpaperKind || 'image', panelConfig.wallpaperSource || panelConfig.backgroundImageSrc || FALLBACK.source, panelConfig.wallpaperPoster)

  // Then the Space's default: the Space's look, for anyone who has not named a
  // wallpaper of their own. `theme` means exactly that, so it falls through here.
  if (spaceWallpaper && spaceWallpaper.mode && spaceWallpaper.mode !== 'theme') {
    if (spaceWallpaper.mode === 'library') {
      const choice = libraryChoice(findLibraryEntry(spaceWallpaper.libraryEntry))
      // An entry the library no longer carries falls through to the theme's own
      // artwork rather than leaving the page bare.
      if (choice)
        return choice
    }
    if (spaceWallpaper.mode === 'custom' && spaceWallpaper.source)
      return customChoice(spaceWallpaper.kind || 'image', spaceWallpaper.source, spaceWallpaper.poster)
  }

  return themeWallpaper || FALLBACK
}
