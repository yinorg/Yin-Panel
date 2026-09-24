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
  compatibility?: { engine: string; minimum: string; maximum?: string }
  wallpapers?: Record<string, { kind: 'image' | 'video' | 'webBundle' | 'externalUrl' | 'imageUrl'; source: string; poster?: string; overlayOpacity?: number }>
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
  controlHeight: 'dimension', iconSize: 'dimension',
}

const legacyLayoutSlots = new Set(['layoutTemplate', 'homeColumns', 'sidebarWidth', 'contentMaxWidth', 'pageGutter', 'breakpointMobile', 'breakpointTablet'])
const api3Aliases: Record<string, string> = {
  fontBody: 'semantic.typography.body', fontDisplay: 'semantic.typography.display',
  fontBodySize: 'semantic.typography.bodySize', fontSmallSize: 'semantic.typography.smallSize', fontHeadingSize: 'semantic.typography.headingSize',
  fontBodyWeight: 'semantic.typography.bodyWeight', fontHeadingWeight: 'semantic.typography.headingWeight',
  lineHeightBody: 'semantic.typography.bodyLineHeight', lineHeightHeading: 'semantic.typography.headingLineHeight',
  spaceXs: 'spacing.xs', spaceSm: 'spacing.sm', spaceMd: 'spacing.md', spaceLg: 'spacing.lg', spaceXl: 'spacing.xl',
  radiusControl: 'shape.control', radiusCard: 'shape.card', radiusDialog: 'shape.dialog', borderWidth: 'shape.borderWidth',
  shadowCard: 'elevation.card', shadowPopup: 'elevation.popup', controlHeight: 'component.button.height', iconSize: 'component.iconography.size',
}

export function resolvePanelValue<T>(themeDefault: T, storedValue: T | undefined, useThemeDefaults: boolean): T {
  return useThemeDefaults || storedValue === undefined ? themeDefault : storedValue
}

export function selectThemeScheme(schemes: string[], mode: 'light' | 'dark' | 'auto', osTheme: 'light' | 'dark'): string {
  if (schemes.length === 1) return schemes[0]
  return mode === 'auto' ? osTheme : mode
}

export function resolveThemeTokens(pkg: ThemePackage, scheme: string): Record<string, string> {
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
    if (legacyLayoutSlots.has(slot)) continue
    const pointer = pkg.manifest.bindings[slot]
    if (!pointer?.startsWith('/')) throw new Error(`Semantic slot "${slot}" is not bound`)
    const name = pointer.slice(1).split('/').map(part => part.replaceAll('~1', '/').replaceAll('~0', '~')).join('.')
    const token = resolve(name)
    if (token.type !== expectedType) throw new Error(`Semantic slot "${slot}" is not a ${expectedType}`)
    slots[slot] = cssToken(slot, token.type, token.value)
  }
  if (pkg.manifest.apiVersion === '3') {
    for (const name of tokens.keys()) {
      const token = resolve(name)
      const parts = name.split('.')
      if (parts[0] === 'semantic') parts.shift()
      const variable = parts.map(part => part.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()).join('-')
      let value = token.value
      if (token.type === 'asset' && value) {
        const resource = pkg.manifest.resources?.find(item => item.path === value)
        if (!resource?.url) throw new Error(`Asset token "${name}" does not identify a packaged resource`)
        value = `url(${JSON.stringify(resource.url)})`
      }
      slots[variable] = cssToken(variable, token.type, value)
    }
    for (const [alias, name] of Object.entries(api3Aliases)) {
      if (!tokens.has(name)) continue
      const token = resolve(name)
      slots[alias] = cssToken(alias, token.type, token.value)
    }
  }
  return slots
}

export const resolveThemeSlots = resolveThemeTokens

export function resolveWallpaper(pkg: ThemePackage, scheme: string) {
  const selected = pkg.manifest.schemes.length === 1 ? pkg.manifest.schemes[0] : scheme
  const wallpaper = pkg.manifest.wallpapers?.[selected]
  if (!wallpaper) return null
  const asset = (path: string) => pkg.manifest.resources?.find(item => item.path === path)?.url || ''
  const source = wallpaper.kind === 'externalUrl' || wallpaper.kind === 'imageUrl' ? wallpaper.source : asset(wallpaper.source)
  const poster = wallpaper.kind === 'imageUrl' ? wallpaper.source : asset(wallpaper.poster || wallpaper.source)
  if (!source || !poster) return null
  const opacity = resolveThemeTokens(pkg, selected)['background-overlay-opacity']
  const overlayOpacity = wallpaper.overlayOpacity ?? (opacity === undefined ? undefined : Number.parseFloat(opacity))
  const executableWallpaper = wallpaper.kind === 'webBundle' || wallpaper.kind === 'externalUrl'
  return executableWallpaper
    ? { kind: 'image', source: poster, poster, overlayOpacity }
    : { kind: wallpaper.kind === 'imageUrl' ? 'image' : wallpaper.kind, source, poster, overlayOpacity }
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
    if (slot.endsWith('tiltDegrees') && (value < 0 || value > 12)) throw new Error(`Token "${slot}" is out of range`)
    if (slot.toLowerCase().includes('opacity') && (value < 0 || value > 1)) throw new Error(`Token "${slot}" is out of range`)
    if (slot.toLowerCase().includes('scale') && (value < 0.5 || value > 1.5)) throw new Error(`Token "${slot}" is out of range`)
    return String(value)
  }
  if (type === 'string') {
    if (typeof value !== 'string' || value.length > 200 || /[;{}<>\u0000-\u001f]/.test(value)) throw new Error(`Token "${slot}" has an invalid string`)
    return value
  }
  if (type === 'asset') {
    if (typeof value !== 'string' || value.length > 2048 || /[\u0000-\u001f;]/.test(value)) throw new Error(`Token "${slot}" has an invalid asset reference`)
    return value
  }
  if (type === 'boolean') {
    if (typeof value !== 'boolean') throw new Error(`Token "${slot}" has an invalid boolean`)
    return String(value)
  }
  if (type === 'strokeStyle') {
    if (!['solid', 'dashed', 'dotted', 'double', 'groove', 'ridge', 'inset', 'outset'].includes(value)) throw new Error(`Token "${slot}" has an invalid stroke style`)
    return value
  }
  if (type === 'border') {
    const color = cssColor(value?.color)
    const width = cssToken(slot, 'dimension', value?.width)
    const style = cssToken(slot, 'strokeStyle', value?.style)
    if (!color) throw new Error(`Token "${slot}" has an invalid border`)
    return `${width} ${style} ${color}`
  }
  if (type === 'typography') {
    const family = cssToken(slot, 'fontFamily', value?.fontFamily)
    const size = cssToken(slot, 'dimension', value?.fontSize)
    const weight = cssToken(slot, 'fontWeight', value?.fontWeight)
    const lineHeight = value?.lineHeight === undefined ? '' : `/${cssToken(slot, 'number', value.lineHeight)}`
    const style = value?.fontStyle === 'italic' ? 'italic ' : ''
    return `${style}${weight} ${size}${lineHeight} ${family}`
  }
  if (type === 'transition') {
    const property = typeof value?.property === 'string' && /^[\w-]+$/.test(value.property) ? value.property : 'all'
    const duration = cssToken(slot, 'duration', value?.duration)
    const delay = value?.delay ? ` ${cssToken(slot, 'duration', value.delay)}` : ''
    const timing = value?.timingFunction ? ` ${cssToken(slot, 'cubicBezier', value.timingFunction)}` : ''
    return `${property} ${duration}${timing}${delay}`
  }
  if (type === 'gradient') {
    if (!Array.isArray(value) || value.length < 2 || value.length > 16) throw new Error(`Token "${slot}" has an invalid gradient`)
    const stops = value.map((stop: any) => {
      const color = cssColor(stop?.color)
      if (!color || typeof stop?.position !== 'number' || stop.position < 0 || stop.position > 1) throw new Error(`Token "${slot}" has an invalid gradient stop`)
      return `${color} ${stop.position * 100}%`
    })
    return `linear-gradient(180deg, ${stops.join(', ')})`
  }
  if (type === 'duration') {
    if (!value || typeof value.value !== 'number' || value.value < 0 || value.value > 10000 || !['ms', 's'].includes(value.unit)) throw new Error(`Token "${slot}" has an invalid duration`)
    return `${value.value}${value.unit}`
  }
  if (type === 'cubicBezier') {
    if (!Array.isArray(value) || value.length !== 4 || !value.every((point: unknown) => typeof point === 'number' && Number.isFinite(point)) || value[0] < 0 || value[0] > 1 || value[2] < 0 || value[2] > 1) throw new Error(`Token "${slot}" has an invalid cubicBezier`)
    return `cubic-bezier(${value.join(', ')})`
  }
  if (type === 'shadow') {
    const layers = Array.isArray(value) ? value : [value]
    if (!layers.length || layers.length > 3) throw new Error(`Semantic slot "${slot}" has an invalid shadow`)
    return layers.map((layer: any) => {
      const color = cssColor(layer?.color)
      if (!color) throw new Error(`Semantic slot "${slot}" has an invalid shadow color`)
    const fields = ['offsetX', 'offsetY', 'blur', 'spread'].map((field) => {
      const dimension = layer[field]
      if (!dimension || typeof dimension.value !== 'number' || !Number.isFinite(dimension.value) || Math.abs(dimension.value) > 96 || (field === 'blur' && dimension.value < 0) || !['px', 'rem'].includes(dimension.unit)) throw new Error(`Semantic slot "${slot}" has an invalid shadow dimension`)
      return `${dimension.value}${dimension.unit}`
    })
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
