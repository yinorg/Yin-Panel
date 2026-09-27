import type { ThemeItemIcon } from '../../theme/api/v1'

const ICONIFY_API_ORIGIN = 'https://api.iconify.design'
const MAX_ICONIFY_IDENTIFIER_LENGTH = 160
const MAX_ICONIFY_SVG_LENGTH = 128_000
const MAX_ICONIFY_DATA_URI_LENGTH = 256_000
const MAX_CACHED_ICONIFY_RESOURCES = 256
const MAX_PENDING_ICONIFY_RESOURCES = 64

const ALLOWED_ELEMENTS = new Set(['svg', 'g', 'path', 'circle', 'ellipse', 'rect', 'line', 'polyline', 'polygon', 'defs', 'linearGradient', 'radialGradient', 'stop', 'clipPath', 'mask'])
const ALLOWED_ATTRIBUTES = new Set([
  'xmlns', 'viewBox', 'width', 'height', 'fill', 'stroke', 'stroke-width', 'stroke-linecap', 'stroke-linejoin',
  'fill-rule', 'clip-rule', 'd', 'transform', 'cx', 'cy', 'r', 'x', 'y', 'x1', 'y1', 'x2', 'y2', 'points',
  'rx', 'ry', 'opacity', 'fill-opacity', 'stroke-opacity', 'id', 'clip-path', 'mask', 'offset', 'stop-color',
  'stop-opacity', 'gradientUnits', 'gradientTransform',
])

export interface CoreItemIconSource {
  itemType: number
  src?: string
  fileName?: string
  text?: string
  backgroundColor?: string
  resolvedSrc?: string
}

export interface IconifyResourceResolver {
  beginGeneration: () => number
  isCurrentGeneration: (generation: number) => boolean
  resolve: (identifier: string, generation?: number) => Promise<string | undefined>
  cancel: () => void
}

export function createIconifyResourceResolver(options: {
  fetch?: typeof fetch
  parseSvg?: (source: string) => Document
} = {}): IconifyResourceResolver {
  const fetchResource = options.fetch || globalThis.fetch?.bind(globalThis)
  const parseSvg = options.parseSvg || defaultParseSvg
  const resources = new Map<string, string>()
  const pending = new Map<string, { generation: number; controller: AbortController; promise: Promise<string | undefined> }>()
  let generation = 0

  function beginGeneration() {
    generation++
    abortPending()
    return generation
  }

  function isCurrentGeneration(value: number) {
    return value === generation
  }

  function cancel() {
    generation++
    abortPending()
  }

  function abortPending() {
    for (const request of pending.values()) request.controller.abort()
    pending.clear()
  }

  async function resolve(identifier: string, requestedGeneration = generation): Promise<string | undefined> {
    if (!isValidIconifyIdentifier(identifier) || !isCurrentGeneration(requestedGeneration) || !fetchResource)
      return undefined
    const cached = resources.get(identifier)
    if (cached) return cached
    const existing = pending.get(identifier)
    if (existing?.generation === requestedGeneration) return existing.promise
    if (pending.size >= MAX_PENDING_ICONIFY_RESOURCES) return undefined

    const controller = new AbortController()
    const promise = Promise.resolve().then(() => fetchResource(buildIconifyEndpoint(identifier), {
      method: 'GET',
      mode: 'cors',
      credentials: 'omit',
      redirect: 'error',
      referrerPolicy: 'no-referrer',
      signal: controller.signal,
      headers: { Accept: 'image/svg+xml' },
    })).then(async (response) => {
      if (!response.ok || !isCurrentGeneration(requestedGeneration)) return undefined
      const contentType = response.headers.get('content-type')?.split(';', 1)[0].trim().toLowerCase()
      if (contentType !== 'image/svg+xml') return undefined
      const source = await response.text()
      if (source.length > MAX_ICONIFY_SVG_LENGTH || !isCurrentGeneration(requestedGeneration)) return undefined
      const safeSvg = sanitizeIconifySvg(source, parseSvg)
      if (!safeSvg) return undefined
      const dataUri = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(safeSvg)}`
      if (dataUri.length > MAX_ICONIFY_DATA_URI_LENGTH || !isCurrentGeneration(requestedGeneration)) return undefined
      if (resources.size >= MAX_CACHED_ICONIFY_RESOURCES)
        resources.delete(resources.keys().next().value as string)
      resources.set(identifier, dataUri)
      return dataUri
    }).catch(() => undefined).finally(() => {
      if (pending.get(identifier)?.promise === promise) pending.delete(identifier)
    })
    pending.set(identifier, { generation: requestedGeneration, controller, promise })
    return promise
  }

  return { beginGeneration, isCurrentGeneration, resolve, cancel }
}

export function isValidIconifyIdentifier(identifier: unknown): identifier is string {
  return typeof identifier === 'string'
    && identifier.length <= MAX_ICONIFY_IDENTIFIER_LENGTH
    && /^[a-z0-9]+(?:-[a-z0-9]+)*:[a-z0-9]+(?:-[a-z0-9]+)*$/.test(identifier)
}

export function isSafeIconifyDataUri(value: unknown): value is string {
  return typeof value === 'string'
    && value.length <= MAX_ICONIFY_DATA_URI_LENGTH
    && value.startsWith('data:image/svg+xml;charset=utf-8,')
}

export function toThemeItemIcon(icon: CoreItemIconSource | null | undefined): ThemeItemIcon | undefined {
  if (!icon) return undefined
  if (icon.itemType === 1)
    return { itemType: 1, text: icon.text, backgroundColor: icon.backgroundColor }
  if (icon.itemType === 2)
    return { itemType: 2, src: icon.src, fileName: icon.fileName, backgroundColor: icon.backgroundColor }
  if (icon.itemType === 3) {
    const resource = isSafeIconifyDataUri(icon.resolvedSrc) ? icon.resolvedSrc : icon.src
    return isSafeIconifyDataUri(resource)
      ? { itemType: 3, src: resource, backgroundColor: icon.backgroundColor }
      : { itemType: 4, backgroundColor: icon.backgroundColor }
  }
  return { itemType: 4, backgroundColor: icon.backgroundColor }
}

function buildIconifyEndpoint(identifier: string): string {
  const [prefix, name] = identifier.split(':')
  return `${ICONIFY_API_ORIGIN}/${prefix}/${name}.svg`
}

function defaultParseSvg(source: string): Document {
  if (typeof DOMParser === 'undefined') throw new Error('SVG parsing is unavailable')
  return new DOMParser().parseFromString(source, 'image/svg+xml')
}

function sanitizeIconifySvg(source: string, parse: (source: string) => Document): string | undefined {
  try {
    const document = parse(source)
    const root = document.documentElement
    if (!root || root.localName !== 'svg' || root.namespaceURI !== 'http://www.w3.org/2000/svg' || document.doctype)
      return undefined
    const body = serializeAllowedNode(root, true)
    if (!body || !/<path\b|<(?:circle|ellipse|rect|line|polyline|polygon)\b/.test(body)) return undefined
    return body
  }
  catch {
    return undefined
  }
}

function serializeAllowedNode(element: Element, isRoot = false): string | undefined {
  if (element.namespaceURI !== 'http://www.w3.org/2000/svg' || !ALLOWED_ELEMENTS.has(element.localName)) return undefined
  const attributes: string[] = []
  for (const attribute of Array.from(element.attributes)) {
    const { name, value } = attribute
    if (!ALLOWED_ATTRIBUTES.has(name) || name.toLowerCase().startsWith('on') || !isSafeAttribute(name, value))
      return undefined
    attributes.push(`${name}="${escapeXmlAttribute(value)}"`)
  }
  if (isRoot && !attributes.some(attribute => attribute.startsWith('xmlns=')))
    attributes.push('xmlns="http://www.w3.org/2000/svg"')
  let content = ''
  for (const child of Array.from(element.childNodes)) {
    if (child.nodeType === 1) {
      const serialized = serializeAllowedNode(child as Element)
      if (!serialized) return undefined
      content += serialized
    }
    else if (child.nodeType === 3 && child.textContent?.trim()) {
      return undefined
    }
    else if (child.nodeType !== 3 && child.nodeType !== 8) {
      return undefined
    }
  }
  return `<${element.localName}${attributes.length ? ` ${attributes.join(' ')}` : ''}>${content}</${element.localName}>`
}

function isSafeAttribute(name: string, value: string): boolean {
  if (value.length > 32_000 || /[<>"'`]/.test(value) || /url\s*\(/i.test(value)) return false
  if (name === 'xmlns') return value === 'http://www.w3.org/2000/svg'
  if (name === 'width' || name === 'height') return /^(?:\d+(?:\.\d+)?|\.\d+)(?:px|em|rem|%)?$/.test(value)
  if (name === 'd') return /^[0-9eE.,+\-\sMmLlHhVvCcSsQqTtAaZz]*$/.test(value)
  if (name === 'viewBox' || name === 'points' || name === 'transform' || name === 'gradientTransform')
    return /^[0-9eE.,+\-()\s]*$/.test(value)
  if (name === 'clip-path' || name === 'mask') return /^url\(#[a-zA-Z][\w.-]*\)$/.test(value)
  if (['fill', 'stroke', 'stop-color'].includes(name))
    return /^(?:currentColor|none|#[0-9a-fA-F]{3,8}|[a-zA-Z]+)$/.test(value)
  if (name === 'id') return /^[a-zA-Z][\w.-]*$/.test(value)
  if (name === 'xmlns' || name === 'gradientUnits') return value === 'http://www.w3.org/2000/svg' || value === 'userSpaceOnUse' || value === 'objectBoundingBox'
  return /^[0-9eE.,+%\-\s]*$/.test(value)
}

function escapeXmlAttribute(value: string): string {
  return value.replace(/&/g, '&amp;').replace(/"/g, '&quot;')
}
