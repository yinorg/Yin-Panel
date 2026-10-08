import { defineStore } from 'pinia'
import { getStorage, removeStorage as hRemoveStorage, setStorage } from './helper'

export interface AuthState {
  userInfo: User.Info | null
  /**
   * Whether a session is believed to exist. The token itself lives in an
   * httpOnly cookie the frontend cannot read, so this flag only drives UI and
   * guards; a stale flag is corrected by the API's login-expired handler.
   */
  loggedIn: boolean
}

const defaultState: AuthState = {
  userInfo: null,
  loggedIn: false,
}

export const useAuthStore = defineStore('auth-store', {
  state: (): AuthState => {
    const saved = getStorage() as (AuthState & { token?: string }) | null
    if (!saved)
      return { ...defaultState }
    // Legacy session: the token used to be JS-readable here. Drop it and force a
    // fresh cookie login so it never lives in script again.
    if (saved.token) {
      hRemoveStorage()
      return { ...defaultState }
    }
    return { userInfo: saved.userInfo ?? null, loggedIn: !!saved.loggedIn }
  },

  actions: {
    setLoggedIn(loggedIn: boolean) {
      this.loggedIn = loggedIn
    },

    setUserInfo(userInfo: User.Info) {
      // Defense in depth: never persist a token carried on the login payload.
      if (userInfo && typeof userInfo === 'object' && 'token' in userInfo) {
        const { token, ...rest } = userInfo as User.Info & { token?: string }
        void token
        this.userInfo = rest as User.Info
        return
      }
      this.userInfo = userInfo
    },

    saveStorage() {
      setStorage(this.$state)
    },

    removeStorage() {
      this.$state = { ...defaultState }
      hRemoveStorage()
    },
  },

})
