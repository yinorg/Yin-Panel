import { computed, watch } from 'vue'
import type { NLocale } from 'naive-ui'
import {
  deDE,
  enUS,
  esES,
  frFR,
  hiIN,
  idID,
  itIT,
  jaJP,
  koKR,
  nlNL,
  plPL,
  ptBR,
  ruRU,
  thTH,
  trTR,
  zhCN,
  zhTW,
} from 'naive-ui'
import { useAppStore } from '../store'
import { resolveBrowserLocale, setLocaleAsync } from '../locales'

const naiveLocales: Record<string, NLocale> = {
  'en-US': enUS,
  'zh-CN': zhCN,
  'zh-TW': zhTW,
  'ja-JP': jaJP,
  'ko-KR': koKR,
  'de-DE': deDE,
  'fr-FR': frFR,
  'es-ES': esES,
  'pt-BR': ptBR,
  'ru-RU': ruRU,
  'id-ID': idID,
  'tr-TR': trTR,
  'pl-PL': plPL,
  'it-IT': itIT,
  'nl-NL': nlNL,
  'hi-IN': hiIN,
  'th-TH': thTH,
}

export function useLanguage() {
  const appStore = useAppStore()

  // Fetch the locale chunk before switching, so a language change never renders
  // raw keys. The boot path already loaded the stored language, so this is a
  // no-op on first render and only does work when the user actually changes it.
  watch(
    () => appStore.language,
    language => { void setLocaleAsync(language) },
    { immediate: true },
  )

  const language = computed(() => {
    const resolvedLanguage = appStore.language === 'auto' ? resolveBrowserLocale() : appStore.language
    return naiveLocales[resolvedLanguage] ?? enUS
  })

  return { language }
}
