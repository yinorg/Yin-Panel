import { computed } from 'vue'
import { enUS, zhCN } from 'naive-ui'
// import { enUS, koKR, zhCN, zhTW } from 'naive-ui'
import { useAppStore } from '../store'
import { resolveBrowserLocale, setLocale } from '../locales'

export function useLanguage() {
  const appStore = useAppStore()

  const language = computed(() => {
    const resolvedLanguage = appStore.language === 'auto' ? resolveBrowserLocale() : appStore.language
    setLocale(appStore.language)
    switch (resolvedLanguage) {
      case 'en-US':
        setLocale('en-US')
        return enUS
      case 'zh-CN':
        return zhCN
      default:
        return enUS
    }
  })

  return { language }
}
