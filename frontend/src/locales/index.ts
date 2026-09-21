import type { App } from 'vue'
import { createI18n } from 'vue-i18n'
import enUS from './en-US.json'
import zhCN from './zh-CN.json'
import zhTW from './zh-TW.json'
import jaJP from './ja-JP.json'
import koKR from './ko-KR.json'
import deDE from './de-DE.json'
import frFR from './fr-FR.json'
import esES from './es-ES.json'
import ptBR from './pt-BR.json'
import ruRU from './ru-RU.json'

export const supportedLocales = ['zh-CN', 'en-US', 'zh-TW', 'ja-JP', 'ko-KR', 'de-DE', 'fr-FR', 'es-ES', 'pt-BR', 'ru-RU'] as const
export type SupportedLocale = typeof supportedLocales[number]
const defaultLocale: SupportedLocale = 'en-US'

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
  return 'en-US'
}

const i18n = createI18n({
  locale: defaultLocale,
  // Missing translations must never fall back to Simplified Chinese.
  fallbackLocale: 'en-US',
  allowComposition: true,
  messages: {
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
  },
})

export const t = i18n.global.t

// 避免循环依赖appstore(authstore)language此处暂时先使用any
// 后面有时间调整
export function setLocale(locale: string) {
  const resolvedLocale = locale === 'auto' ? resolveBrowserLocale() : locale
  i18n.global.locale = (supportedLocales as readonly string[]).includes(resolvedLocale) ? resolvedLocale as SupportedLocale : defaultLocale
}

export function setupI18n(app: App) {
  app.use(i18n)
}

export default i18n
