/**
 * The text colour that stays readable on a background, or `undefined` when the
 * value cannot be parsed.
 *
 * An item icon may carry a user-chosen background and then paints a letter or a
 * glyph on it. A fixed scheme colour put a light letter on a light tile (知乎,
 * 微博 and 唯品会 all default to the translucent `#2a2a2a6b`, which renders pale
 * over the panel), so the surface decides the text instead of the page scheme.
 *
 * A translucent colour has no luminance of its own: it is first flattened over
 * `base`, the surface it is painted on. Without a base there is nothing to
 * flatten against, so the caller keeps its own default.
 */
export function readableTextColor(background?: string | null, base?: string | null): '#101317' | '#ffffff' | undefined {
  const colour = parseColor(background)
  if (!colour)
    return undefined
  let channels = colour.channels
  if (colour.alpha < 1) {
    const backdrop = parseColor(base)
    if (!backdrop || backdrop.alpha < 1)
      return undefined
    channels = colour.channels.map((value, index) =>
      Math.round(value * colour.alpha + backdrop.channels[index] * (1 - colour.alpha))) as [number, number, number]
  }
  const [r, g, b] = channels.map((value) => {
    const channel = value / 255
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
  })
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b
  // Contrast against black and white (WCAG 2.x), so the darker of the two wins on
  // a light background and the lighter one wins on a dark background.
  const againstBlack = (luminance + 0.05) / 0.05
  const againstWhite = 1.05 / (luminance + 0.05)
  return againstWhite >= againstBlack ? '#ffffff' : '#101317'
}

/**
 * The tile colour the editor's first version wrote as its own default. It is not a
 * choice: a translucent near-black renders differently on every wallpaper, and the
 * tile an "auto" icon gets is the panel's own surface. Treat it as "no colour" so a
 * text icon sits on the same surface as auto, while a colour the user picked still
 * wins.
 */
export const LEGACY_DEFAULT_ICON_BACKGROUND = '#2a2a2a6b'

export function normalizeIconBackground(background?: string | null): string | undefined {
  if (typeof background !== 'string')
    return undefined
  const value = background.trim()
  if (!value || value.toLowerCase() === LEGACY_DEFAULT_ICON_BACKGROUND)
    return undefined
  return value
}

function parseColor(value?: string | null): { channels: [number, number, number]; alpha: number } | undefined {
  if (typeof value !== 'string')
    return undefined
  const input = value.trim().toLowerCase()
  const hex = /^#([0-9a-f]+)$/.exec(input)
  if (hex) {
    const digits = hex[1]
    if (digits.length === 3 || digits.length === 4)
      return {
        channels: [0, 1, 2].map(index => Number.parseInt(digits[index] + digits[index], 16)) as [number, number, number],
        alpha: digits.length === 4 ? Number.parseInt(digits[3] + digits[3], 16) / 255 : 1,
      }
    if (digits.length === 6 || digits.length === 8)
      return {
        channels: [0, 1, 2].map(index => Number.parseInt(digits.slice(index * 2, index * 2 + 2), 16)) as [number, number, number],
        alpha: digits.length === 8 ? Number.parseInt(digits.slice(6, 8), 16) / 255 : 1,
      }
    return undefined
  }
  const rgb = /^rgba?\(\s*(\d{1,3})\s*[,\s]\s*(\d{1,3})\s*[,\s]\s*(\d{1,3})\s*(?:[,/]\s*([\d.]+)\s*)?\)$/.exec(input)
  if (!rgb)
    return undefined
  const alpha = rgb[4] === undefined ? 1 : Number.parseFloat(rgb[4])
  if (!Number.isFinite(alpha) || alpha < 0 || alpha > 1)
    return undefined
  const channels = [rgb[1], rgb[2], rgb[3]].map(Number) as [number, number, number]
  return channels.some(channel => channel < 0 || channel > 255) ? undefined : { channels, alpha }
}
