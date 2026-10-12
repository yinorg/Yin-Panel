import { defineStore } from 'pinia'
import { appearancePanelKeys, defaultState, defaultStatePanelConfig, getLocalState, layoutPanelKeys, panelConfigSignature, removeLocalState, setLocalState } from './helper'
import { getUserConfig } from '@/api/panel/userConfig'
import { router } from '@/router'
import type { PanelStateNetworkModeEnum } from '@/enums'
import { useAppStore } from '../app'
import type { Language } from '../app/helper'
export const usePanelState = defineStore('panel', {
  state: (): Panel.State => getLocalState() || defaultState(),

  getters: {
    /** Whether the panel config holds edits the server has not been told about.
     *  Nothing is claimed before the server's own answer arrives. */
    panelConfigDirty(state): boolean {
      if (!state.savedPanelConfig) return false
      return panelConfigSignature(state.panelConfig) !== panelConfigSignature(state.savedPanelConfig)
    },
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
        // What the server just confirmed is the baseline the settings pages
        // compare against, so it is captured before any further edit.
        this.markPanelConfigSaved()
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

    /** Remember the config the server holds, so the settings pages can tell the
     *  user what is still unsaved. */
    markPanelConfigSaved() {
      this.savedPanelConfig = JSON.parse(JSON.stringify(this.panelConfig)) as Panel.panelConfig
    },

    /** Restore one settings page's own keys from the defaults, leaving every
     *  other key — the other page's half, and the wallpaper library choice the
     *  wallpaper centre owns — untouched. */
    resetPanelSection(keys: (keyof Panel.panelConfig)[]) {
      const defaults = defaultStatePanelConfig()
      const next = { ...this.panelConfig }
      for (const key of keys) {
        // Each key indexes this config, so the default's value for it has the
        // same shape as the field it replaces.
        ;(next as Record<keyof Panel.panelConfig, unknown>)[key] = defaults[key]
      }
      this.panelConfig = next
      this.recordState()
    },

    resetAppearanceConfig() {
      this.resetPanelSection(appearancePanelKeys)
    },

    resetLayoutConfig() {
      this.resetPanelSection(layoutPanelKeys)
    },

    async reloadRoute(id?: number) {
      await router.push({ name: 'AppletDialog', params: { aiAppletId: id } })
    },

    recordState() {
      // The saved baseline is a runtime fact about this session's server calls,
      // not a setting: persisting it would let a stale snapshot claim an edit is
      // unsaved after a reload.
      setLocalState({ ...this.$state, savedPanelConfig: null })
    },

    removeState() {
      removeLocalState()
    },
  },
})
