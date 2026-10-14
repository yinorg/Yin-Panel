import { ref } from 'vue'

/**
 * How light the artwork behind the home is.
 *
 * The Core owns the wallpaper, so it is the only place that can read it: the
 * value is derived here once and handed to the theme through the snapshot. The
 * theme decides what to do with it — the suggestions below are what the Core
 * would use, not a rule the theme has to follow.
 */
export type WallpaperTone = 'light' | 'dark'

export interface WallpaperForeground {
  /** A foreground that reads against artwork of this tone. */
  onWallpaper: string
  /** The same, for secondary text. */
  onWallpaperMuted: string
}

/** The luminance at or below which artwork counts as dark. */
export const DARK_LUMA = 140

const SUGGESTIONS: Record<WallpaperTone, WallpaperForeground> = {
  dark: { onWallpaper: '#ffffff', onWallpaperMuted: 'rgb(255 255 255 / 82%)' },
  light: { onWallpaper: '#101418', onWallpaperMuted: 'rgb(16 20 24 / 72%)' },
}

/** The tone an average luminance describes. */
export function toneForLuma(luma: number): WallpaperTone {
  return luma < DARK_LUMA ? 'dark' : 'light'
}

/** The foreground the Core suggests for a tone. */
export function suggestedForeground(tone: WallpaperTone): WallpaperForeground {
  return SUGGESTIONS[tone]
}

/**
 * The tone of the wallpaper currently painted, or `null`. Paired with
 * `wallpaperPainted`, `null` means the artwork is there but cannot be read —
 * a sandboxed page, a cross-origin frame, a picture that taints the canvas.
 */
export const wallpaperTone = ref<WallpaperTone | null>(null)

/** Whether any artwork is painted behind the home at all. */
export const wallpaperPainted = ref(false)

/**
 * Average luminance of an image's opaque pixels, or `null` when it cannot be
 * read. Anchored to the top when asked: the masthead sits over the upper band,
 * and that is the part that has to carry text.
 */
export function measureImage(source: string, size = 32, topAnchored = true): Promise<number | null> {
  return new Promise(resolve => {
    if (typeof document === 'undefined') { resolve(null); return }
    const probe = new Image()
    probe.referrerPolicy = 'no-referrer'
    probe.onload = () => {
      try {
        const canvas = document.createElement('canvas')
        canvas.width = size
        canvas.height = size
        const context = canvas.getContext('2d', { willReadFrequently: true })
        if (!context) { resolve(null); return }
        const scale = Math.max(size / probe.width, size / probe.height)
        const width = probe.width * scale
        const height = probe.height * scale
        context.drawImage(probe, (size - width) / 2, topAnchored ? 0 : (size - height) / 2, width, height)
        const { data } = context.getImageData(0, 0, size, size)
        let luma = 0
        let weight = 0
        for (let index = 0; index < data.length; index += 4) {
          const alpha = data[index + 3] / 255
          if (alpha < 0.1) continue
          luma += (0.2126 * data[index] + 0.7152 * data[index + 1] + 0.0722 * data[index + 2]) * alpha
          weight += alpha
        }
        resolve(weight ? luma / weight : null)
      }
      catch {
        // A cross-origin picture taints the canvas and `getImageData` throws.
        resolve(null)
      }
    }
    probe.onerror = () => resolve(null)
    probe.src = source
  })
}

/** Derive the tone for one painted source and publish it. */
export async function publishWallpaperTone(source: string | null): Promise<void> {
  if (!source) {
    wallpaperPainted.value = false
    wallpaperTone.value = null
    return
  }
  wallpaperPainted.value = true
  const luma = await measureImage(source)
  // Artwork that cannot be read reports no tone rather than a guessed one; the
  // theme decides what to draw on it.
  wallpaperTone.value = luma === null ? null : toneForLuma(luma)
}
