export interface ThemeManifest {
  id: string
  name: string
  packageVersion: string
  schemes: string[]
  documents: Record<string, string>
  bindings: Record<string, string>
  resources?: Array<{ path: string; sha256: string; mediaType: string; url: string }>
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
    if (typeof token.value === 'string' && /^\{.+\}$/.test(token.value)) {
      const next = token.value.slice(1, -1).replaceAll('/', '.')
      const chain = new Set(visiting)
      chain.add(name)
      const target = resolve(next, chain)
      if (token.type && token.type !== target.type)
        throw new Error(`Token reference changes type at "${name}"`)
      return { type: token.type || target.type, value: target.value }
    }
    return { type: token.type, value: resolveNested(token.value, visiting) }
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
  for (const slot of semanticSlots) {
    const pointer = pkg.manifest.bindings[slot]
    if (!pointer?.startsWith('/')) throw new Error(`Semantic slot "${slot}" is not bound`)
    const name = pointer.slice(1).split('/').map(part => part.replaceAll('~1', '/').replaceAll('~0', '~')).join('.')
    const token = resolve(name)
    if (token.type !== 'color') throw new Error(`Semantic slot "${slot}" is not a color`)
    const color = cssColor(token.value)
    if (!/^#[0-9a-f]{6}$/i.test(color)) throw new Error(`Semantic slot "${slot}" has an unsupported color`)
    slots[slot] = color
  }
  return slots
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
