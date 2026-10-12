import { useMessage } from 'naive-ui'
import { set as setUserConfig } from '@/api/panel/userConfig'
import { usePanelState } from '@/store'
import { t } from '@/locales'

/**
 * Persist the panel config to the account.
 *
 * Edits land in `panelState.panelConfig` and take effect immediately; this is the
 * explicit step that writes the whole config to the server so it follows the user
 * to another device. The appearance and layout pages own different parts of the
 * same config, and the wallpaper centre writes it too, so they share one save.
 */
export function usePanelConfigSave() {
  const panelState = usePanelState()
  const ms = useMessage()

  function savePanelConfig() {
    setUserConfig({ panel: panelState.panelConfig }).then((res) => {
      if (res.code === 0)
        ms.success(t('apps.baseSettings.configSaved'))
      else
        ms.error(t('apps.baseSettings.configFailed', { message: res.msg }))
    })
  }

  return { savePanelConfig }
}
