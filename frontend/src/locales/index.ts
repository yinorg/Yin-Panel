import type { App } from 'vue'
import { createI18n } from 'vue-i18n'
import enUS from './en-US.json'

export const supportedLocales = ['zh-CN', 'en-US', 'zh-TW', 'ja-JP', 'ko-KR', 'de-DE', 'fr-FR', 'es-ES', 'pt-BR', 'ru-RU', 'id-ID', 'tr-TR', 'pl-PL', 'it-IT', 'nl-NL', 'hi-IN', 'th-TH'] as const
export type SupportedLocale = typeof supportedLocales[number]
const defaultLocale: SupportedLocale = 'en-US'

/**
 * Locale JSON is split into one chunk per language and loaded on demand.
 *
 * Every locale used to be imported eagerly, which folded roughly 730 KB of JSON
 * — 17 languages — into the shared `store` chunk that gates the first paint. On a
 * slow link that alone pushed first paint past six seconds. Only the English
 * fallback stays in the initial graph; the visitor's own language is fetched while
 * the boot layer is still up, and every other language the moment it is selected.
 *
 * The loaders are written out instead of using `import.meta.glob` so each locale
 * is its own chunk keyed by name, and so a missing translation is a type error
 * rather than a runtime one.
 */
const localeLoaders: Record<SupportedLocale, () => Promise<{ default: typeof enUS }>> = {
  'en-US': async () => ({ default: enUS }),
  'zh-CN': () => import('./zh-CN.json'),
  'zh-TW': () => import('./zh-TW.json'),
  'ja-JP': () => import('./ja-JP.json'),
  'ko-KR': () => import('./ko-KR.json'),
  'de-DE': () => import('./de-DE.json'),
  'fr-FR': () => import('./fr-FR.json'),
  'es-ES': () => import('./es-ES.json'),
  'pt-BR': () => import('./pt-BR.json'),
  'ru-RU': () => import('./ru-RU.json'),
  'id-ID': () => import('./id-ID.json'),
  'tr-TR': () => import('./tr-TR.json'),
  'pl-PL': () => import('./pl-PL.json'),
  'it-IT': () => import('./it-IT.json'),
  'nl-NL': () => import('./nl-NL.json'),
  'hi-IN': () => import('./hi-IN.json'),
  'th-TH': () => import('./th-TH.json'),
}

const loadedLocales = new Set<SupportedLocale>([defaultLocale])
const pendingLocales = new Map<SupportedLocale, Promise<void>>()

export function resolveBrowserLocale(): SupportedLocale {
  const browserLocale = typeof navigator === 'undefined' ? '' : navigator.language.toLowerCase()
  if (browserLocale.startsWith('zh-tw') || browserLocale.startsWith('zh-hk') || browserLocale.startsWith('zh-mo')) return 'zh-TW'
  if (browserLocale.startsWith('zh')) return 'zh-CN'
  if (browserLocale.startsWith('ja')) return 'ja-JP'
  if (browserLocale.startsWith('ko')) return 'ko-KR'
  if (browserLocale.startsWith('de')) return 'de-DE'
  if (browserLocale.startsWith('fr')) return 'fr-FR'
  if (browserLocale.startsWith('es')) return 'es-ES'
  if (browserLocale.startsWith('pt')) return 'pt-BR'
  if (browserLocale.startsWith('ru')) return 'ru-RU'
  if (browserLocale.startsWith('id')) return 'id-ID'
  if (browserLocale.startsWith('tr')) return 'tr-TR'
  if (browserLocale.startsWith('pl')) return 'pl-PL'
  if (browserLocale.startsWith('it')) return 'it-IT'
  if (browserLocale.startsWith('nl')) return 'nl-NL'
  if (browserLocale.startsWith('hi')) return 'hi-IN'
  if (browserLocale.startsWith('th')) return 'th-TH'
  return defaultLocale
}

export function resolveSupportedLocale(locale: string): SupportedLocale {
  const resolvedLocale = locale === 'auto' ? resolveBrowserLocale() : locale
  return (supportedLocales as readonly string[]).includes(resolvedLocale)
    ? resolvedLocale as SupportedLocale
    : defaultLocale
}

const i18n = createI18n({
  legacy: false,
  globalInjection: true,
  locale: defaultLocale,
  // Missing translations must never fall back to Simplified Chinese.
  fallbackLocale: 'en-US',
  messages: {
    'en-US': enUS,
  },
})

export const t = i18n.global.t

/**
 * Load a locale's messages into vue-i18n, once.
 *
 * Resolving the promise is what makes the locale switch atomic: the active
 * locale is only moved after its messages are in place, so a language change can
 * never render raw keys. Concurrent callers share a single in-flight request.
 */
export function loadLocale(locale: SupportedLocale): Promise<void> {
  if (loadedLocales.has(locale)) return Promise.resolve()
  const inflight = pendingLocales.get(locale)
  if (inflight) return inflight
  const promise = localeLoaders[locale]().then((module) => {
    i18n.global.setLocaleMessage(locale, module.default)
    loadedLocales.add(locale)
    pendingLocales.delete(locale)
  }).catch((error) => {
    pendingLocales.delete(locale)
    throw error
  })
  pendingLocales.set(locale, promise)
  return promise
}

function applyLocale(locale: SupportedLocale) {
  i18n.global.locale.value = locale
  if (typeof document !== 'undefined')
    document.documentElement.lang = locale
}

/** Synchronous switch for code that only reaches already-loaded locales. */
export function setLocale(locale: string) {
  applyLocale(resolveSupportedLocale(locale))
}

/** Load the locale's chunk (if needed) and only then switch to it. */
export async function setLocaleAsync(locale: string): Promise<void> {
  const resolved = resolveSupportedLocale(locale)
  await loadLocale(resolved)
  applyLocale(resolved)
}

/**
 * Apply the visitor's stored language before mount so the first render is already
 * in the right language rather than flashing the English fallback.
 */
export async function setupI18n(app: App, language: string) {
  await loadLocale(resolveSupportedLocale(language))
  applyLocale(resolveSupportedLocale(language))
  app.use(i18n)
}

export default i18n
