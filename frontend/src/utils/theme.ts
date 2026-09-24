export interface ThemeManifest {
  id: string
  name: string
  packageVersion: string
  schemes: string[]
  documents: Record<string, string>
  resources?: Array<{ path: string; sha256: string; mediaType: string; url?: string }>
  fonts?: Array<{ family: string; path: string; weight: number; style: string }>
  themeApi?: string
  core?: string
  entrypoints?: { script?: string; styles?: string[] }
  runtime?: { supportedModes?: string[] }
  contributes?: { views?: string[]; regions?: string[]; components?: string[] }
  permissions?: { required?: Array<{ name: string; origins?: string[] }>; optional?: Array<{ name: string; origins?: string[] }> }
  wallpapers?: Record<string, { kind: 'image' | 'video' | 'webBundle' | 'externalUrl' | 'imageUrl'; source: string; poster?: string; overlayOpacity?: number }>
}

export interface ThemePackage {
  manifest: ThemeManifest
  documents: Record<string, Record<string, any>>
  revision?: string
  verified: boolean
}

export interface ThemePackageV2 {
  manifest: {
    id: string
    name: string
    version: string
    defaultScheme: string
    themeApi?: string
    core?: string
    entrypoints?: { script?: string; styles?: string[] }
    runtime?: { supportedModes?: string[] }
    contributes?: { views?: string[]; regions?: string[]; components?: string[] }
    permissions?: { required?: Array<{ name: string; origins?: string[] }>; optional?: Array<{ name: string; origins?: string[] }> }
    tokens: { format: 'DTCG'; version: string; documents: Record<string, string> }
    resources: Array<{ path: string; sha256: string; mediaType: string; url?: string }>
  }
  tokens: Record<string, Record<string, any>>
  revision: string
  verified: boolean
}

export function normalizeThemePackageV2(pkg: ThemePackageV2): ThemePackage {
  return {
    manifest: {
      id: pkg.manifest.id,
      name: pkg.manifest.name,
      packageVersion: pkg.manifest.version,
      themeApi: pkg.manifest.themeApi,
      core: pkg.manifest.core,
      entrypoints: pkg.manifest.entrypoints,
      runtime: pkg.manifest.runtime,
      contributes: pkg.manifest.contributes,
      permissions: pkg.manifest.permissions,
      schemes: Object.keys(pkg.manifest.tokens.documents),
      documents: pkg.manifest.tokens.documents,
      resources: pkg.manifest.resources || [],
    },
    documents: pkg.tokens,
    revision: pkg.revision,
    verified: pkg.verified,
  }
}

export const semanticSlots = [
  'canvas', 'surface', 'surfaceElevated', 'text', 'textMuted', 'border', 'primary',
  'onPrimary', 'secondary', 'success', 'warning', 'danger', 'focusRing',
] as const

const cssColorProfiles = new Set(['srgb', 'srgb-linear', 'display-p3', 'a98-rgb', 'prophoto-rgb', 'rec2020', 'xyz-d50', 'xyz-d65'])
const cssPolarColorSpaces = new Set(['hsl', 'hwb', 'lab', 'lch', 'oklab', 'oklch'])

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

  const tokens = new Map<string, { type: string; value: any; group: string }>()
  const groups = new Map<string, { type: string; extends?: string }>()
  function visit(value: any, prefix: string[] = [], inheritedType = '', inheritedGroup = '') {
    if (!value || typeof value !== 'object' || Array.isArray(value))
      return
    const type = typeof value.$type === 'string' ? value.$type : inheritedType
    const name = prefix.join('.')
    if ('$value' in value) {
      tokens.set(name, { type, value: value.$value, group: inheritedGroup })
      return
    }
    if (name) {
      const reference = value.$extends
      if (reference !== undefined && (typeof reference !== 'string' || !/^\{[^{}]+\}$/.test(reference)))
        throw new Error(`DTCG group "${name}" has an invalid $extends reference`)
      groups.set(name, { type, extends: typeof reference === 'string' ? reference.slice(1, -1).replaceAll('/', '.') : undefined })
    }
    const group = name || inheritedGroup
    for (const [key, child] of Object.entries(value)) {
      if (key === '$root') visit(child, prefix, type, group)
      else if (!key.startsWith('$')) visit(child, [...prefix, key], type, group)
    }
  }
  visit(document)

  const expandedGroups = new Map<string, Map<string, { type: string; value: any; group: string }>>()
  const extending = new Set<string>()
  function inheritedGroupType(name: string, visited = new Set<string>()): string {
    if (visited.has(name)) throw new Error(`DTCG group extension cycle at "${name}"`)
    visited.add(name)
    const group = groups.get(name)
    if (!group) return ''
    if (group.type) return group.type
    return group.extends ? inheritedGroupType(group.extends, visited) : ''
  }
  function expandGroup(name: string) {
    const cached = expandedGroups.get(name)
    if (cached) return cached
    if (extending.has(name)) throw new Error(`DTCG group extension cycle at "${name}"`)
    const group = groups.get(name)
    if (!group) throw new Error(`DTCG group "${name}" extends an unknown group`)
    extending.add(name)
    const expanded = new Map<string, { type: string; value: any; group: string }>()
    if (group.extends) {
      for (const [relative, token] of expandGroup(group.extends))
        expanded.set(relative, { ...token, type: group.type || token.type, group: name })
    }
    for (const [tokenName, token] of tokens) {
      if (token.group !== name && !token.group.startsWith(`${name}.`)) continue
      const relative = tokenName === name ? '$root' : tokenName.slice(name.length + 1)
      expanded.set(relative, { ...token, type: token.type || inheritedGroupType(name) })
    }
    extending.delete(name)
    expandedGroups.set(name, expanded)
    return expanded
  }
  for (const [name, group] of groups) {
    if (!group.extends) continue
    for (const [relative, token] of expandGroup(name)) {
      const tokenName = relative === '$root' ? name : `${name}.${relative}`
      const existing = tokens.get(tokenName)
      if (existing) {
        if (!existing.type && token.type) tokens.set(tokenName, { ...existing, type: token.type })
      }
      else {
        tokens.set(tokenName, { ...token, group: token.group || name })
      }
    }
  }

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
  for (const name of tokens.keys()) {
    const token = resolve(name)
    const parts = name.split('.')
    if (parts[0] === 'semantic') parts.shift()
    const variable = parts.map(part => part.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()).join('-')
    slots[variable] = cssToken(variable, token.type, token.value)
  }
  const semanticAliases: Record<string, string[]> = {
    canvas: ['semantic.canvas', 'semantic.color.canvas'],
    surface: ['semantic.surface', 'semantic.color.surface'],
    surfaceElevated: ['semantic.surfaceElevated', 'semantic.color.surfaceElevated', 'semantic.surface', 'semantic.color.surface'],
    text: ['semantic.text', 'semantic.color.text'],
    textMuted: ['semantic.muted', 'semantic.textMuted', 'semantic.color.muted', 'semantic.color.textMuted'],
    border: ['semantic.border', 'semantic.color.border'],
    primary: ['semantic.primary', 'semantic.color.primary'],
    onPrimary: ['semantic.onPrimary', 'semantic.color.onPrimary'],
    secondary: ['semantic.secondary', 'semantic.color.secondary'],
    success: ['semantic.success', 'semantic.color.success'],
    warning: ['semantic.warning', 'semantic.color.warning'],
    danger: ['semantic.danger', 'semantic.color.danger'],
    focusRing: ['semantic.focusRing', 'semantic.color.focusRing', 'semantic.primary', 'semantic.color.primary'],
    fontBody: ['semantic.typography.body'],
    fontDisplay: ['semantic.typography.display'],
    fontBodySize: ['semantic.typography.bodySize'],
    fontSmallSize: ['semantic.typography.smallSize'],
    fontHeadingSize: ['semantic.typography.headingSize'],
    fontBodyWeight: ['semantic.typography.bodyWeight'],
    fontHeadingWeight: ['semantic.typography.headingWeight'],
    lineHeightBody: ['semantic.typography.bodyLineHeight'],
    lineHeightHeading: ['semantic.typography.headingLineHeight'],
    spaceMd: ['semantic.spacing.component', 'semantic.spacing.md'],
    radiusCard: ['semantic.shape.cardRadius'],
    radiusControl: ['semantic.shape.controlRadius'],
    shadowCard: ['semantic.elevation.card'],
  }
  for (const [alias, names] of Object.entries(semanticAliases)) {
    const name = names.find(candidate => tokens.has(candidate))
    if (!name) continue
    const token = resolve(name)
    slots[alias] = cssToken(alias, token.type, token.value)
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
    if (!color) throw new Error(`Semantic slot "${slot}" has an unsupported color`)
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
    return /^#[0-9a-f]{6}$/i.test(value) ? value : ''
  const colorSpace = value?.colorSpace
  if ((!cssColorProfiles.has(colorSpace) && !cssPolarColorSpaces.has(colorSpace)) || !Array.isArray(value.components) || value.components.length !== 3)
    return ''
  const components = value.components.map((component: unknown) => {
    if (component === 'none') return 'none'
    if (typeof component !== 'number' || !Number.isFinite(component)) return ''
    return String(component)
  })
  if (components.some((component: string) => component === '')) return ''
  const alpha = value.alpha ?? 1
  if (typeof alpha !== 'number' || !Number.isFinite(alpha) || alpha < 0 || alpha > 1) return ''
  if (colorSpace === 'srgb' && alpha === 1 && components.every((component: string) => component !== 'none' && Number(component) >= 0 && Number(component) <= 1)) {
    const bytes = components.map((component: string) => Math.round(Number(component) * 255))
    return `#${bytes.map((component: number) => component.toString(16).padStart(2, '0')).join('')}`
  }
  const alphaValue = alpha === 1 ? '' : ` / ${alpha}`
  if (colorSpace === 'hsl' || colorSpace === 'hwb') {
    const [hue, first, second] = components
    return `${colorSpace}(${hue} ${first === 'none' ? 'none' : `${Number(first) * 100}%`} ${second === 'none' ? 'none' : `${Number(second) * 100}%`}${alphaValue})`
  }
  if (cssPolarColorSpaces.has(colorSpace))
    return `${colorSpace}(${components.join(' ')}${alphaValue})`
  return `color(${colorSpace} ${components.join(' ')}${alphaValue})`
}
