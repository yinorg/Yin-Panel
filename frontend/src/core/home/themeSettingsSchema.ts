import type { ThemeManifest } from '@/utils/theme'
import { sha256Hex } from '../../utils/sha256.js'

export interface ThemeSettingsPackage {
  manifest: Pick<ThemeManifest, 'resources' | 'settings'>
  revision?: string
}

type Validator = ((value: Record<string, unknown>) => boolean) & { errors?: unknown }

export function createThemeSettingsSchemaValidator(options: {
  origin: string
  fetcher?: typeof fetch
  loadAjv?: () => Promise<typeof import('ajv/dist/2020.js')>
}) {
  const fetcher = options.fetcher || fetch
  const validators = new Map<string, Validator>()

  return async function validateThemeSettings(theme: ThemeSettingsPackage | undefined, settings: Record<string, unknown>): Promise<void> {
    const declaration = theme?.manifest.settings
    if (!declaration) return
    if (!Number.isSafeInteger(declaration.schemaVersion) || declaration.schemaVersion < 1)
      throw apiError('INVALID_ARGUMENT', 'Theme settings schema version is invalid')

    const resource = theme?.manifest.resources?.find(candidate => candidate.path === declaration.schema && candidate.mediaType === 'application/schema+json')
    if (!resource?.url) throw apiError('INVALID_ARGUMENT', 'Theme settings schema resource is unavailable')
    const url = new URL(resource.url, options.origin)
    const themeAssetRoute = url.pathname.startsWith('/api/theme/v2/assets/') || url.pathname.startsWith('/api/theme/v2/preview/') && url.pathname.includes('/assets/')
    if (url.origin !== options.origin || !themeAssetRoute || url.search || url.hash || url.username || url.password)
      throw apiError('PERMISSION_DENIED', 'Theme settings schema must be a same-origin Theme resource')

    const response = await fetcher(url, { credentials: 'omit', redirect: 'error', cache: 'force-cache' })
    if (!response.ok) throw apiError('INVALID_ARGUMENT', 'Theme settings schema could not be loaded')
    const bytes = new Uint8Array(await response.arrayBuffer())
    if (bytes.byteLength > 256 * 1024) throw apiError('INVALID_ARGUMENT', 'Theme settings schema exceeds 256 KiB')
    const actual = await sha256Hex(bytes)
    if (actual !== resource.sha256.toLowerCase()) throw apiError('INVALID_ARGUMENT', 'Theme settings schema digest does not match')

    const cacheKey = `${theme?.revision}:${actual}`
    let validate = validators.get(cacheKey)
    if (!validate) {
      const schema = JSON.parse(new TextDecoder().decode(bytes))
      rejectExternalSchemaRefs(schema)
      const { default: Ajv } = await (options.loadAjv || (() => import('ajv/dist/2020.js')))()
      const ajv = new Ajv({ allErrors: true, strict: true, validateSchema: true })
      validate = ajv.compile(schema) as Validator
      validators.set(cacheKey, validate)
    }
    if (!validate(settings))
      throw apiError('INVALID_ARGUMENT', `Theme settings do not match the package schema: ${JSON.stringify(validate.errors || [])}`)
  }
}

function rejectExternalSchemaRefs(value: unknown): void {
  if (Array.isArray(value)) {
    value.forEach(rejectExternalSchemaRefs)
    return
  }
  if (!value || typeof value !== 'object') return
  for (const [key, child] of Object.entries(value)) {
    if (key === '$ref' && typeof child === 'string' && !child.startsWith('#'))
      throw apiError('INVALID_ARGUMENT', 'Theme settings schema cannot reference external documents')
    rejectExternalSchemaRefs(child)
  }
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
