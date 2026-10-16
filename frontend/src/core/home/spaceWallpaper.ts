import { ref } from 'vue'

/**
 * The most recent Space-default-wallpaper write, as its writer saw it.
 *
 * The home resolves the active Space's default wallpaper when the Space changes
 * and keeps it in `panelState.spaceWallpaper` for the wallpaper layer to read.
 * The space settings dialog runs on top of that running home, so a save used to
 * leave the cache stale — the artwork only changed on the next reload. Writers
 * announce the value the server now holds, and the home applies it when the
 * write belongs to the Space it is showing.
 */
export interface SpaceWallpaperWrite {
  spaceId: number
  wallpaper: Panel.SpaceWallpaperConfig | null
  at: number
}

export const spaceWallpaperWrite = ref<SpaceWallpaperWrite | null>(null)

export function announceSpaceWallpaperWrite(spaceId: number, wallpaper: Panel.SpaceWallpaperConfig | null) {
  spaceWallpaperWrite.value = { spaceId, wallpaper, at: Date.now() }
}
