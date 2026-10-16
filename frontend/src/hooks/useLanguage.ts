import type { NLocale } from 'naive-ui'
import { computed, shallowRef, watch } from 'vue'
import { enUS } from 'naive-ui'
import { useAppStore } from '../store'
import { resolveSupportedLocale, setLocaleAsync, type SupportedLocale } from '../locales'

/**
 * Naive's own control copy — pagination, file pickers, empty states, popconfirm
 * buttons — is per-language and adds up fast. Seventeen static imports put every
 * one of them in the entry graph, which the login page then paid for to show a
 * single form. Each locale is fetched when its language is used instead, the
 * same way the app's own translations already are, and the boot path preloads
 * the visitor's before mount so the first render is already in the right one.
 */
const naiveLocaleLoaders: Record<SupportedLocale, () => Promise<{ default: NLocale }>> = {
  'en-US': async () => ({ default: enUS }),
  'zh-CN': () => import('naive-ui/es/locales/common/zhCN'),
  'zh-TW': () => import('naive-ui/es/locales/common/zhTW'),
  'ja-JP': () => import('naive-ui/es/locales/common/jaJP'),
  'ko-KR': () => import('naive-ui/es/locales/common/koKR'),
  'de-DE': () => import('naive-ui/es/locales/common/deDE'),
  'fr-FR': () => import('naive-ui/es/locales/common/frFR'),
  'es-ES': () => import('naive-ui/es/locales/common/esES'),
  'pt-BR': () => import('naive-ui/es/locales/common/ptBR'),
  'ru-RU': () => import('naive-ui/es/locales/common/ruRU'),
  'id-ID': () => import('naive-ui/es/locales/common/idID'),
  'tr-TR': () => import('naive-ui/es/locales/common/trTR'),
  'pl-PL': () => import('naive-ui/es/locales/common/plPL'),
  'it-IT': () => import('naive-ui/es/locales/common/itIT'),
  'nl-NL': () => import('naive-ui/es/locales/common/nlNL'),
  'hi-IN': () => import('naive-ui/es/locales/common/hiIN'),
  'th-TH': () => import('naive-ui/es/locales/common/thTH'),
}

/** The active locale, read by the application-level `NConfigProvider`. */
const naiveLocale = shallowRef<NLocale>(enUS)
const loadedNaiveLocales = new Map<SupportedLocale, NLocale>([['en-US', enUS]])
const pendingNaiveLocales = new Map<SupportedLocale, Promise<void>>()

/**
 * Fetch Naive's control copy for a language and make it the active one, once.
 *
 * Concurrent callers share one in-flight request, and a language already loaded
 * resolves from the cache, so switching back and forth never refetches.
 */
export function loadNaiveLocale(language: string): Promise<void> {
  const locale = resolveSupportedLocale(language)
  const cached = loadedNaiveLocales.get(locale)
  if (cached) {
    naiveLocale.value = cached
    return Promise.resolve()
  }
  const inflight = pendingNaiveLocales.get(locale)
  if (inflight) return inflight
  const promise = naiveLocaleLoaders[locale]().then((module) => {
    loadedNaiveLocales.set(locale, module.default)
    naiveLocale.value = module.default
    pendingNaiveLocales.delete(locale)
  }).catch((error) => {
    pendingNaiveLocales.delete(locale)
    throw error
  })
  pendingNaiveLocales.set(locale, promise)
  return promise
}

export function useLanguage() {
  const appStore = useAppStore()

  // Fetch both halves of a language change — the app's own messages and Naive's
  // control copy — before switching, so a change never renders raw keys or mixed
  // languages. The boot path already loaded the stored language, so this is a
  // cache hit on first render and only does work when the user changes it.
  watch(
    () => appStore.language,
    language => { void switchLanguage(language) },
    { immediate: true },
  )

  const language = computed(() => naiveLocale.value)

  return { language }
}

async function switchLanguage(language: string) {
  await Promise.all([setLocaleAsync(language), loadNaiveLocale(language)])
}
