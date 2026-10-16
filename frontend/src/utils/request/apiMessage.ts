import type { ConfigProviderProps, MessageOptions } from 'naive-ui'
import { createDiscreteApi, darkTheme, lightTheme, useOsTheme } from 'naive-ui'
import { computed, ref } from 'vue'
import type { Response } from './index'
import { t } from '../../locales'
import { useAppStore } from '../../store'

const themeRef = ref<'light' | 'dark'>('light')
const configProviderPropsRef = computed<ConfigProviderProps>(() => ({
  theme: themeRef.value === 'light' ? lightTheme : darkTheme,
}))
const { message: naiveMessage } = createDiscreteApi(['message'], {
  configProviderProps: configProviderPropsRef,
  // The home's space selector sits at the top centre, and a top-centre toast covers
  // it. The discrete API is a second provider next to the mounted one, so it needs
  // the same placement as `NaiveProvider` rather than Naive's top-centre default.
  messageProviderProps: { placement: 'top-right', max: 3 },
})

/**
 * One outage fires a failure per in-flight request — the home loads its data and
 * its monitor snapshot together — which stacked the same error twice. Keep one
 * copy of an identical error on screen and let the next one in after it leaves.
 */
const liveErrors = new Set<string>()

function errorOnce(content: string, options?: MessageOptions) {
  if (liveErrors.has(content))
    return
  liveErrors.add(content)
  const { onLeave } = options || {}
  return naiveMessage.error(content, {
    ...options,
    onLeave() {
      liveErrors.delete(content)
      onLeave?.()
    },
  })
}

export const message = { ...naiveMessage, error: errorOnce }

export function apiRespErrMsg(res: Response): boolean {
  const appStore = useAppStore()
  const osTheme = useOsTheme()
  if (appStore.theme === 'auto')
    themeRef.value = osTheme.value as 'dark' | 'light'
  else
    themeRef.value = appStore.theme as 'dark' | 'light'

  const apiErrorCodeName = `apiErrorCode.${res.code}`
  const getI18nValue = t(apiErrorCodeName)
  if (apiErrorCodeName === getI18nValue) {
    return false
  }
  else {
    message.error(t(`apiErrorCode.${res.code}`))
    return true
  }
}
