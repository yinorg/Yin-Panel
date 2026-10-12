import { get } from '@/utils/request'

/** One wallpaper the Core's library offers. An `image` is painted like any
 *  photo; a `page` is an HTML document the layer mounts in a sandboxed frame. */
export interface WallpaperLibraryEntry {
  id: string
  name: string
  kind: 'image' | 'page'
  source: string
  poster?: string
}

export interface WallpaperLibraryCategory {
  id: string
  name: string
  items: WallpaperLibraryEntry[]
}

export interface WallpaperLibrary {
  version: string
  categories: WallpaperLibraryCategory[]
}

export function getWallpaperLibrary<T = WallpaperLibrary>(signal?: AbortSignal) {
  return get<T>({
    url: '/theme/v2/wallpaper/library',
    signal,
  })
}
