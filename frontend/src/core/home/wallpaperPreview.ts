import { shallowRef } from 'vue'

/**
 * The wallpaper the picker is previewing but has not saved yet.
 *
 * The wallpaper layer reads this on top of the stored panel config, so choosing a
 * tile paints the home immediately while the user config stays untouched until the
 * save button commits it. The picker clears it on unmount, so closing the dialog
 * without saving puts the stored wallpaper back.
 */
export interface WallpaperPreview {
  mode: 'theme' | 'library'
  entry: string
}

const wallpaperPreview = shallowRef<WallpaperPreview | null>(null)

export function setWallpaperPreview(value: WallpaperPreview | null) {
  wallpaperPreview.value = value
}

export function useWallpaperPreview() {
  return wallpaperPreview
}
