export interface ThemeManifest {
  id: string
  name: string
  packageVersion: string
  schemes: string[]
  documents: Record<string, string>
  bindings: Record<string, string>
  resources?: Array<{ path: string; sha256: string; mediaType: string; url: string }>
  fonts?: Array<{ family: string; path: string; weight: number; style: string }>
  apiVersion?: string
  wallpapers?: Record<string, { kind: 'image' | 'video' | 'webBundle' | 'externalUrl'; source: string; poster?: string }>
}

export interface ThemePackage {
  manifest: ThemeManifest
  documents: Record<string, Record<string, any>>
  verified: boolean
}

export const semanticSlots = [
  'canvas', 'surface', 'surfaceElevated', 'text', 'textMuted', 'border', 'primary',
  'onPrimary', 'secondary', 'success', 'warning', 'danger', 'focusRing',
] as const

export const designSlotTypes: Record<string, string> = {
  fontBody: 'fontFamily', fontDisplay: 'fontFamily',
  fontBodySize: 'dimension', fontSmallSize: 'dimension', fontHeadingSize: 'dimension',
  fontBodyWeight: 'fontWeight', fontHeadingWeight: 'fontWeight',
  lineHeightBody: 'number', lineHeightHeading: 'number',
  spaceXs: 'dimension', spaceSm: 'dimension', spaceMd: 'dimension',
  spaceLg: 'dimension', spaceXl: 'dimension',
  radiusControl: 'dimension', radiusCard: 'dimension', radiusDialog: 'dimension',
  borderWidth: 'dimension', shadowCard: 'shadow', shadowPopup: 'shadow',
  controlHeight: 'dimension', iconSize: 'dimension', sidebarWidth: 'dimension',
  contentMaxWidth: 'dimension', pageGutter: 'dimension',
  breakpointMobile: 'dimension', breakpointTablet: 'dimension',
  layoutTemplate: 'string', homeColumns: 'number',
}

export function resolvePanelValue<T>(themeDefault: T, storedValue: T | undefined, useThemeDefaults: boolean): T {
  return useThemeDefaults || storedValue === undefined ? themeDefault : storedValue
}

export function selectThemeScheme(schemes: string[], mode: 'light' | 'dark' | 'auto', osTheme: 'light' | 'dark'): string {
  if (schemes.length === 1) return schemes[0]
  return mode === 'auto' ? osTheme : mode
}

export function resolveThemeSlots(pkg: ThemePackage, scheme: string): Record<string, string> {
  const selectedScheme = pkg.manifest.schemes.length === 1 ? pkg.manifest.schemes[0] : scheme
  const document = pkg.documents[selectedScheme]
  if (!document)
    throw new Error(`Theme scheme "${selectedScheme}" is missing`)

  const tokens = new Map<string, { type: string; value: any }>()
  function visit(value: any, prefix: string[] = [], inheritedType = '') {
    if (!value || typeof value !== 'object' || Array.isArray(value))
      return
    const type = typeof value.$type === 'string' ? value.$type : inheritedType
    if ('$value' in value)
      tokens.set(prefix.join('.'), { type, value: value.$value })
    Object.entries(value).forEach(([key, child]) => {
      if (!key.startsWith('$')) visit(child, [...prefix, key], type)
    })
  }
  visit(document)

  function resolve(name: string, visiting = new Set<string>()): { type: string; value: any } {
    const token = tokens.get(name)
    if (!token) throw new Error(`Unknown token "${name}"`)
    if (visiting.has(name)) throw new Error(`Token reference cycle at "${name}"`)
    const chain = new Set(visiting)
    chain.add(name)
    if (typeof token.value === 'string' && /^\{.+\}$/.test(token.value)) {
      const next = token.value.slice(1, -1).replaceAll('/', '.')
      const target = resolve(next, chain)
      if (token.type && token.type !== target.type)
        throw new Error(`Token reference changes type at "${name}"`)
      return { type: token.type || target.type, value: target.value }
    }
    return { type: token.type, value: resolveNested(token.value, chain) }
  }

  function resolveNested(value: any, visiting: Set<string>): any {
    if (typeof value === 'string' && /^\{[^{}]+\}$/.test(value))
      return resolve(value.slice(1, -1).replaceAll('/', '.'), visiting).value
    if (Array.isArray(value))
      return value.map(item => resolveNested(item, visiting))
    if (value && typeof value === 'object') {
      return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, resolveNested(item, visiting)]))
    }
    return value
  }

  const slots: Record<string, string> = {}
  const types: Record<string, string> = Object.fromEntries(semanticSlots.map(slot => [slot, 'color']))
  if (pkg.manifest.apiVersion === '2') Object.assign(types, designSlotTypes)
  for (const [slot, expectedType] of Object.entries(types)) {
    const pointer = pkg.manifest.bindings[slot]
    if (!pointer?.startsWith('/')) throw new Error(`Semantic slot "${slot}" is not bound`)
    const name = pointer.slice(1).split('/').map(part => part.replaceAll('~1', '/').replaceAll('~0', '~')).join('.')
    const token = resolve(name)
    if (token.type !== expectedType) throw new Error(`Semantic slot "${slot}" is not a ${expectedType}`)
    slots[slot] = cssToken(slot, token.type, token.value)
  }
  if (pkg.manifest.apiVersion === '2' && Number.parseFloat(slots.breakpointMobile) >= Number.parseFloat(slots.breakpointTablet))
    throw new Error('Theme breakpoints are not ordered')
  return slots
}

export function resolveWallpaper(pkg: ThemePackage, scheme: string) {
  const selected = pkg.manifest.schemes.length === 1 ? pkg.manifest.schemes[0] : scheme
  const wallpaper = pkg.manifest.wallpapers?.[selected]
  if (!wallpaper) return null
  const asset = (path: string) => pkg.manifest.resources?.find(item => item.path === path)?.url || ''
  const source = wallpaper.kind === 'externalUrl' ? wallpaper.source : asset(wallpaper.source)
  const poster = asset(wallpaper.poster || wallpaper.source)
  if (!source || !poster) return null
  return { kind: wallpaper.kind, source, poster }
}

function cssToken(slot: string, type: string, value: any): string {
  if (type === 'color') {
    const color = cssColor(value)
    if (!/^#[0-9a-f]{6}$/i.test(color)) throw new Error(`Semantic slot "${slot}" has an unsupported color`)
    return color
  }
  if (type === 'dimension') {
    if (!value || typeof value.value !== 'number' || !Number.isFinite(value.value) || !['px', 'rem'].includes(value.unit) || value.value < 0)
      throw new Error(`Semantic slot "${slot}" has an invalid dimension`)
    return `${value.value}${value.unit}`
  }
  if (type === 'fontFamily') {
    const names = Array.isArray(value) ? value : [value]
    if (!names.length || !names.every(name => typeof name === 'string' && /^[\p{L}\p{N} _.-]{1,80}$/u.test(name)))
      throw new Error(`Semantic slot "${slot}" has an invalid font family`)
    return names.map(name => name.includes(' ') ? `"${name}"` : name).join(', ')
  }
  if (type === 'fontWeight') {
    if ((typeof value !== 'number' || !Number.isFinite(value) || value < 100 || value > 900) && typeof value !== 'string')
      throw new Error(`Semantic slot "${slot}" has an invalid font weight`)
    return String(value)
  }
  if (type === 'number') {
    if (typeof value !== 'number' || !Number.isFinite(value)) throw new Error(`Semantic slot "${slot}" has an invalid number`)
    return String(value)
  }
  if (type === 'string') {
    if (!['centered', 'split'].includes(value)) throw new Error(`Semantic slot "${slot}" has an invalid layout`)
    return value
  }
  if (type === 'shadow') {
    const layers = Array.isArray(value) ? value : [value]
    if (!layers.length || layers.length > 3) throw new Error(`Semantic slot "${slot}" has an invalid shadow`)
    return layers.map((layer: any) => {
      const color = cssColor(layer?.color)
      if (!color) throw new Error(`Semantic slot "${slot}" has an invalid shadow color`)
      const fields = ['offsetX', 'offsetY', 'blur', 'spread'].map(field => cssToken(slot, 'dimension', layer[field]))
      return `${layer.inset ? 'inset ' : ''}${fields.join(' ')} ${color}`
    }).join(', ')
  }
  throw new Error(`Semantic slot "${slot}" has an unsupported type`)
}

function cssColor(value: any): string {
  if (typeof value === 'string')
    return value
  if (value?.colorSpace !== 'srgb' || !Array.isArray(value.components) || value.components.length !== 3 || (value.alpha !== undefined && value.alpha !== 1))
    return ''
  const bytes = value.components.map((component: unknown) => {
    if (typeof component !== 'number' || component < 0 || component > 1) return -1
    return Math.round(component * 255)
  })
  if (bytes.some((component: number) => component < 0)) return ''
  return `#${bytes.map((component: number) => component.toString(16).padStart(2, '0')).join('')}`
}
