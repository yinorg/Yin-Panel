import type { App } from 'vue'
import { createI18n } from 'vue-i18n'
import enUS from './en-US.json'
// import koKR from './ko-KR'
import zhCN from './zh-CN.json'
// import ruRU from './ru-RU'

export const supportedLocales = ['zh-CN', 'en-US'] as const
export type SupportedLocale = typeof supportedLocales[number]
const defaultLocale: SupportedLocale = 'zh-CN'

const i18n = createI18n({
  locale: defaultLocale,
  // Missing translations must never fall back to Simplified Chinese.
  fallbackLocale: 'en-US',
  allowComposition: true,
  messages: {
    'en-US': enUS,
    // 'ko-KR': koKR,
    'zh-CN': zhCN,
    // 'zh-TW': zhTW,
    // 'ru-RU': ruRU,
  },
})

export const t = i18n.global.t

// 避免循环依赖appstore(authstore)language此处暂时先使用any
// 后面有时间调整
export function setLocale(locale: string) {
  i18n.global.locale = (supportedLocales as readonly string[]).includes(locale) ? locale : defaultLocale
}

export function setupI18n(app: App) {
  app.use(i18n)
}

export default i18n
