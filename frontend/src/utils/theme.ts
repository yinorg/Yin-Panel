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
  for (const scheme of Object.keys(pkg.manifest.tokens.documents)) {
    const document = pkg.tokens[scheme]
    if (!document) throw new Error(`Theme token document "${scheme}" is missing`)
    validateDTCGDocument202510(document)
  }
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
const DTCG_SCHEMA_202510 = 'https://www.designtokens.org/schemas/2025.10/format.json'
const DTCG_TYPES_202510 = new Set(['color', 'dimension', 'fontFamily', 'fontWeight', 'duration', 'cubicBezier', 'number', 'strokeStyle', 'border', 'transition', 'shadow', 'gradient', 'typography'])
const DTCG_METADATA_202510 = new Set(['$schema', '$type', '$value', '$description', '$extensions', '$deprecated', '$extends', '$ref'])

export function validateDTCGDocument202510(document: Record<string, any>): void {
  if (!document || typeof document !== 'object' || Array.isArray(document) || document.$schema !== DTCG_SCHEMA_202510)
    throw new Error('DTCG document must declare the 2025.10 schema')
  function visit(node: any, inheritedType = '', group = '') {
    if (!node || typeof node !== 'object' || Array.isArray(node))
      throw new Error(`DTCG group "${group}" must be an object`)
    if ('$description' in node && typeof node.$description !== 'string')
      throw new Error(`DTCG group "${group}" has an invalid $description`)
    if ('$deprecated' in node && typeof node.$deprecated !== 'boolean' && typeof node.$deprecated !== 'string')
      throw new Error(`DTCG group "${group}" has an invalid $deprecated`)
    if ('$extensions' in node && (!node.$extensions || typeof node.$extensions !== 'object' || Array.isArray(node.$extensions)))
      throw new Error(`DTCG group "${group}" has an invalid $extensions`)
    const declaredType = node.$type
    if (declaredType !== undefined && typeof declaredType !== 'string')
      throw new Error(`DTCG group "${group}" has an invalid $type`)
    const type = declaredType || inheritedType
    if (type && !DTCG_TYPES_202510.has(type))
      throw new Error(`DTCG group "${group}" has an unknown token type "${type}"`)
    if ('$ref' in node && '$value' in node)
      throw new Error(`DTCG token "${group}" cannot contain both $ref and $value`)
    if ('$value' in node) {
      for (const key of Object.keys(node)) {
        if (!key.startsWith('$') || !DTCG_METADATA_202510.has(key))
          throw new Error(`DTCG token "${group}" has unsupported field "${key}"`)
      }
      return
    }
    if ('$ref' in node) {
      if (typeof node.$ref !== 'string' || !node.$ref.startsWith('#/'))
        throw new Error(`DTCG token "${group}" has an invalid JSON Pointer $ref`)
      return
    }
    for (const [key, child] of Object.entries(node)) {
      if (key === '$root') {
        visit(child, type, group || '$root')
        continue
      }
      if (key.startsWith('$')) {
        if (!DTCG_METADATA_202510.has(key)) throw new Error(`DTCG group "${group}" has unsupported metadata "${key}"`)
        if (key === '$extends' && (typeof child !== 'string' || !/^\{[^{}]+\}$/.test(child)))
          throw new Error(`DTCG group "${group}" has an invalid $extends reference`)
        continue
      }
      if (!key || key.startsWith('$') || /[.{}]/.test(key))
        throw new Error(`DTCG group "${group}" has an invalid token name "${key}"`)
      visit(child, type, group ? `${group}.${key}` : key)
    }
  }
  visit(document)
  resolveThemeDocument({
    manifest: { id: 'validation', name: 'Validation', packageVersion: '0.0.0', schemes: ['validation'], documents: {} },
    documents: { validation: document },
    verified: false,
  }, 'validation', true)
}

export function resolvePanelValue<T>(themeDefault: T, storedValue: T | undefined, useThemeDefaults: boolean): T {
  return useThemeDefaults || storedValue === undefined ? themeDefault : storedValue
}

export function selectThemeScheme(schemes: string[], mode: 'light' | 'dark' | 'auto', osTheme: 'light' | 'dark'): string {
  if (schemes.length === 1) return schemes[0]
  return mode === 'auto' ? osTheme : mode
}

export function resolveThemeTokens(pkg: ThemePackage, scheme: string): Record<string, string> {
  return resolveThemeDocument(pkg, scheme, false) as Record<string, string>
}

function resolveThemeDocument(pkg: ThemePackage, scheme: string, validateOnly: true): void
function resolveThemeDocument(pkg: ThemePackage, scheme: string, validateOnly: false): Record<string, string>
function resolveThemeDocument(pkg: ThemePackage, scheme: string, validateOnly: boolean): Record<string, string> | void {
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
    if ('$ref' in value && '$value' in value)
      throw new Error(`DTCG token "${name}" cannot contain both $ref and $value`)
    if ('$value' in value) {
      tokens.set(name, { type, value: value.$value, group: inheritedGroup })
      return
    }
    if ('$ref' in value) {
      if (typeof value.$ref !== 'string' || !value.$ref.startsWith('#/'))
        throw new Error(`DTCG token "${name}" has an invalid JSON Pointer $ref`)
      tokens.set(name, { type, value: { $ref: value.$ref }, group: inheritedGroup })
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
    if (group.extends) return inheritedGroupType(group.extends, visited)
    const parent = name.includes('.') ? name.slice(0, name.lastIndexOf('.')) : ''
    return parent ? inheritedGroupType(parent, visited) : ''
  }
  function expandGroup(name: string) {
    const cached = expandedGroups.get(name)
    if (cached) return cached
    if (extending.has(name)) throw new Error(`DTCG group extension cycle at "${name}"`)
    const group = groups.get(name)
    if (!group) throw new Error(`DTCG group "${name}" extends an unknown group`)
    extending.add(name)
    const expanded = new Map<string, { type: string; value: any; group: string }>()
    const inheritedType = inheritedGroupType(name)
    if (group.extends) {
      for (const [relative, token] of expandGroup(group.extends))
        expanded.set(relative, { ...token, type: group.type || token.type || inheritedType, group: name })
    }
    for (const [tokenName, token] of tokens) {
      if (token.group !== name && !token.group.startsWith(`${name}.`)) continue
      const relative = tokenName === name ? '$root' : tokenName.slice(name.length + 1)
      expanded.set(relative, { ...token, type: token.type || inheritedType })
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

  function pointer(reference: string, visiting = new Set<string>()): any {
    if (!reference.startsWith('#/')) throw new Error(`Invalid JSON Pointer "${reference}"`)
    if (visiting.has(reference)) throw new Error(`JSON Pointer cycle at "${reference}"`)
    visiting.add(reference)
    try {
      let value: any = document
      for (const raw of reference.slice(2).split('/')) {
        if (/~(?![01])/.test(raw)) throw new Error(`Malformed JSON Pointer "${reference}"`)
        const part = raw.replaceAll('~1', '/').replaceAll('~0', '~')
        if (Array.isArray(value)) {
          if (!/^(0|[1-9]\d*)$/.test(part) || Number(part) >= value.length) throw new Error(`Invalid JSON Pointer "${reference}"`)
          value = value[Number(part)]
        } else if (value && typeof value === 'object' && part in value) value = value[part]
        else throw new Error(`Invalid JSON Pointer "${reference}"`)
      }
      if (value && typeof value === 'object' && !Array.isArray(value) && typeof value.$ref === 'string')
        return pointer(value.$ref, visiting)
      return value
    } finally { visiting.delete(reference) }
  }

  function resolve(name: string, visiting = new Set<string>()): { type: string; value: any } {
    const token = tokens.get(name)
    if (!token) throw new Error(`Unknown token "${name}"`)
    if (visiting.has(name)) throw new Error(`Token reference cycle at "${name}"`)
    const chain = new Set(visiting)
    chain.add(name)
    if (token.value && typeof token.value === 'object' && !Array.isArray(token.value) && typeof token.value.$ref === 'string')
      return { type: token.type, value: pointer(token.value.$ref) }
    if (typeof token.value === 'string' && /^\{.+\}$/.test(token.value)) {
      const next = token.value.slice(1, -1).replaceAll('/', '.')
      const target = resolve(next, chain)
      if (token.type && token.type !== target.type)
        throw new Error(`Token reference changes type at "${name}"`)
      return { type: token.type || target.type, value: target.value }
    }
    return { type: token.type, value: resolveNested(token.value, chain, token.type) }
  }

  function resolveNested(value: any, visiting: Set<string>, type: string): any {
    if (value && typeof value === 'object' && !Array.isArray(value) && typeof value.$ref === 'string')
      return pointer(value.$ref)
    if (typeof value === 'string' && /^\{[^{}]+\}$/.test(value)) {
      if (!['border', 'transition', 'shadow', 'gradient', 'typography', 'strokeStyle'].includes(type))
        throw new Error('Curly brace references are only valid in composite token values')
      return resolve(value.slice(1, -1).replaceAll('/', '.'), visiting).value
    }
    if (Array.isArray(value)) return value.map(item => resolveNested(item, visiting, type))
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, resolveNested(item, visiting, type)]))
    return value
  }

  const slots: Record<string, string> = {}
  for (const name of tokens.keys()) {
    const token = resolve(name)
    if (validateOnly) {
      validateDTCGValue202510(name, token.type, token.value)
      continue
    }
    const parts = name.split('.')
    if (parts[0] === 'semantic') parts.shift()
    const variable = parts.map(part => part.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()).join('-')
    slots[variable] = cssToken(variable, token.type, token.value)
  }
  if (validateOnly) return
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

function validateDTCGValue202510(name: string, type: string, value: any) {
  const invalid = () => { throw new Error(`DTCG token "${name}" has an invalid ${type} value`) }
  const dimension = (item: any, units: string[]) => item && typeof item === 'object' && typeof item.value === 'number' && Number.isFinite(item.value) && units.includes(item.unit)
  const color = (item: any) => {
    const spaces: Record<string, number> = {
      srgb: 3, 'srgb-linear': 3, 'display-p3': 3, 'a98-rgb': 3, 'prophoto-rgb': 3,
      rec2020: 3, 'xyz-d50': 3, 'xyz-d65': 3, hsl: 3, hwb: 3, lab: 3, lch: 3, oklab: 3, oklch: 3,
    }
    const componentIsValid = (space: string, index: number, component: any) => {
      if (component === 'none') return true
      if (typeof component !== 'number' || !Number.isFinite(component)) return false
      if (['srgb', 'srgb-linear', 'display-p3', 'a98-rgb', 'prophoto-rgb', 'rec2020', 'xyz-d50', 'xyz-d65'].includes(space)) return component >= 0 && component <= 1
      if (space === 'hsl' || space === 'hwb') return index === 0 ? component >= 0 && component < 360 : component >= 0 && component <= 100
      if (space === 'lab') return index === 0 ? component >= 0 && component <= 100 : true
      if (space === 'lch') return index === 0 ? component >= 0 && component <= 100 : index === 1 ? component >= 0 : component >= 0 && component < 360
      if (space === 'oklab') return index === 0 ? component >= 0 && component <= 1 : true
      if (space === 'oklch') return index === 0 ? component >= 0 && component <= 1 : index === 1 ? component >= 0 : component >= 0 && component < 360
      return false
    }
    return !!item && typeof item === 'object' && Array.isArray(item.components) && spaces[item.colorSpace] === item.components.length
      && item.components.every((component: any, index: number) => componentIsValid(item.colorSpace, index, component))
      && (item.hex === undefined || typeof item.hex === 'string' && /^#[0-9a-fA-F]{6}$/.test(item.hex))
      && (item.alpha === undefined || typeof item.alpha === 'number' && Number.isFinite(item.alpha) && item.alpha >= 0 && item.alpha <= 1)
  }
  const fontFamily = (item: any) => {
    const names = Array.isArray(item) ? item : [item]
    return names.length > 0 && names.every(name => typeof name === 'string' && name.trim())
  }
  const fontWeight = (item: any) => {
    const names = new Set(['thin', 'hairline', 'extra-light', 'ultra-light', 'light', 'normal', 'medium', 'semi-bold', 'demi-bold', 'bold', 'extra-bold', 'ultra-bold', 'black', 'heavy', 'extra-black', 'ultra-black'])
    return typeof item === 'number' && Number.isFinite(item) && item >= 1 && item <= 1000
      || typeof item === 'string' && names.has(item)
  }
  const strokeStyle = (item: any) => {
    const names = new Set(['solid', 'dashed', 'dotted', 'double', 'groove', 'ridge', 'inset', 'outset'])
    return typeof item === 'string' ? names.has(item)
      : Array.isArray(item?.dashArray) && item.dashArray.length > 0 && item.dashArray.every((part: any) => dimension(part, ['px', 'rem']))
        && ['butt', 'round', 'square'].includes(item.lineCap)
  }
  const cubicBezier = (item: any) => Array.isArray(item) && item.length === 4
    && item.every((part: any) => typeof part === 'number' && Number.isFinite(part))
    && item[0] >= 0 && item[0] <= 1 && item[2] >= 0 && item[2] <= 1
  switch (type) {
    case 'color': if (!color(value)) invalid(); return
    case 'dimension': if (!dimension(value, ['px', 'rem'])) invalid(); return
    case 'duration': if (!dimension(value, ['ms', 's'])) invalid(); return
    case 'fontFamily': if (!fontFamily(value)) invalid(); return
    case 'fontWeight': if (!fontWeight(value)) invalid(); return
    case 'number': if (typeof value !== 'number' || !Number.isFinite(value)) invalid(); return
    case 'cubicBezier': if (!cubicBezier(value)) invalid(); return
    case 'strokeStyle': if (!strokeStyle(value)) invalid(); return
    case 'border':
      if (!value || !color(value.color) || !dimension(value.width, ['px', 'rem']) || !strokeStyle(value.style)) invalid()
      return
    case 'transition':
      if (!value || !dimension(value.duration, ['ms', 's']) || !dimension(value.delay, ['ms', 's']) || !cubicBezier(value.timingFunction)) invalid()
      return
    case 'shadow': {
      const layers = Array.isArray(value) ? value : [value]
      if (!layers.length || !layers.every((layer: any) => layer && color(layer.color) && ['offsetX', 'offsetY', 'blur', 'spread'].every(key => dimension(layer[key], ['px', 'rem'])) && (layer.inset === undefined || typeof layer.inset === 'boolean'))) invalid()
      return
    }
    case 'gradient':
      if (!Array.isArray(value) || value.length < 1 || !value.every((stop: any) => stop && color(stop.color) && typeof stop.position === 'number' && Number.isFinite(stop.position))) invalid()
      return
    case 'typography':
      if (!value || !fontFamily(value.fontFamily) || !dimension(value.fontSize, ['px', 'rem']) || !fontWeight(value.fontWeight) || !dimension(value.letterSpacing, ['px', 'rem']) || typeof value.lineHeight !== 'number' || !Number.isFinite(value.lineHeight)) invalid()
      return
    default: invalid()
  }
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
    if (['solid', 'dashed', 'dotted', 'double', 'groove', 'ridge', 'inset', 'outset'].includes(value)) return value
    if (!Array.isArray(value?.dashArray) || !value.dashArray.length || !value.dashArray.every((part: any) => part && typeof part.value === 'number' && Number.isFinite(part.value) && ['px', 'rem'].includes(part.unit)))
      throw new Error(`Token "${slot}" has an invalid stroke style`)
    return value.dashArray.map((part: any) => `${part.value}${part.unit}`).join(' ')
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
