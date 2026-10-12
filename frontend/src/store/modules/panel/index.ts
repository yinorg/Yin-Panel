import { defineStore } from 'pinia'
import { defaultState, defaultStatePanelConfig, getLocalState, removeLocalState, setLocalState } from './helper'
import { getUserConfig } from '@/api/panel/userConfig'
import { router } from '@/router'
import type { PanelStateNetworkModeEnum } from '@/enums'
import { useAppStore } from '../app'
import type { Language } from '../app/helper'
export const usePanelState = defineStore('panel', {
  state: (): Panel.State => getLocalState() || defaultState(),

  getters: {

  },

  actions: {
    setLeftSiderCollapsed(Collapsed: boolean) {
      this.leftSiderCollapsed = Collapsed
      // this.recordState()
    },

    setRightSiderCollapsed(Collapsed: boolean) {
      this.rightSiderCollapsed = Collapsed
      // this.recordState()
    },

    setNetworkMode(mode: PanelStateNetworkModeEnum) {
      this.networkMode = mode
      this.recordState()
    },

    // 获取云端（搭建的服务器）的面板配置
    async updatePanelConfigByCloud(signal?: AbortSignal) {
      try {
        const res = await getUserConfig<Panel.userConfig>(signal)
        if (signal?.aborted) return false
        if (res.code !== 0) return false
        const saved = res.data.panel
        this.panelConfig = { ...defaultStatePanelConfig(), ...saved }
        if (!saved?.wallpaperMode && saved?.backgroundImageSrc)
          this.panelConfig.wallpaperMode = 'custom'
        this.recordState()
        // The language the user chose belongs to the account, so it arrives with
        // the rest of the config and outranks the browser and this device's local
        // copy. An absent value means they never chose one, and the local
        // resolution stands instead.
        if (res.data.language)
          useAppStore().setLanguage(res.data.language as Language)
        return true
      }
      catch {
        return false
      }
    },

    resetPanelConfig() {
      this.panelConfig = defaultStatePanelConfig()
    },

    async reloadRoute(id?: number) {
      await router.push({ name: 'AppletDialog', params: { aiAppletId: id } })
    },

    recordState() {
      setLocalState(this.$state)
    },

    removeState() {
      removeLocalState()
    },
  },
})
