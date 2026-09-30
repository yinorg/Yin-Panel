import type { AxiosProgressEvent, AxiosResponse, GenericAbortSignal } from 'axios'
import request, { parsePublicCodeFromPath } from './axios'
import { apiRespErrMsg, message } from './apiMessage'
import { t } from '../../locales'
import { useAppStore, useAuthStore } from '../../store'
import { router } from '../../router'

let loginMessageShow = false
export interface HttpOption {
  url: string
  data?: any
  method?: string
  headers?: any
  onDownloadProgress?: (progressEvent: AxiosProgressEvent) => void
  signal?: GenericAbortSignal
  beforeRequest?: () => void
  afterRequest?: () => void
}

export interface Response<T = any> {
  data: T
  msg: string
  code: number
}

function http<T = any>(
  { url, data, method, headers, onDownloadProgress, signal, beforeRequest, afterRequest }: HttpOption,
) {
  const authStore = useAuthStore()
  const appStore = useAppStore()
  const successHandler = (res: AxiosResponse<Response<T>>) => {
    if (res.data.code === 0)
      return res.data

    // The console is the one channel that cannot be missing. The message api is
    // mounted as a separate app, so a toast can fail to appear for reasons this
    // layer cannot see — no container, a provider that never mounted, or an empty
    // localised string — and a refused request would then leave no trace at all.
    // Only the code and the server's own message are logged; the body may carry
    // user data and is deliberately left out.
    const failure = `[api] ${method} ${url} -> code ${res.data.code}${res.data.msg ? `: ${res.data.msg}` : ''}`
    switch (res.data.code) {
      case 1001:
      case 1000:
      case 1005:
      case -1:
        // Expected refusals, handled below rather than surfaced as faults.
        console.warn(failure)
        break
      default:
        console.error(failure)
    }

    if (res.data.code === 1001 && !window.location.pathname.match(/^\/[a-z][a-z0-9-]{4,28}[a-z0-9]\/?$/)) {
      // 避免重复弹窗
      if (!loginMessageShow) {
        loginMessageShow = true
        message.warning(t('api.loginExpires'), {
          onLeave() {
            loginMessageShow = false
          },
        })
      }

      router.push({ path: '/login' })
      authStore.removeStorage()
      return res.data
    }

    if (res.data.code === 1000 && !window.location.pathname.match(/^\/[a-z][a-z0-9-]{4,28}[a-z0-9]\/?$/)) {
      router.push({ path: '/login' })
      authStore.removeStorage()
      return res.data
    }

    if (res.data.code === 1005) {
      // A public link is not a signed-in session: every JWT-only endpoint refuses
      // it with 1005, which is an expected state rather than something the visitor
      // can act on. Announcing it puts a warning toast over the home on every open.
      if (!parsePublicCodeFromPath())
        message.warning(res.data.msg)
      return res.data
    }

    if (res.data.code === -1) {
      return res.data
    }

    if (!apiRespErrMsg(res.data))
      return Promise.reject(res.data)
    else
      return res.data
  }

  const failHandler = (error: Response<Error>) => {
    afterRequest?.()
    if (signal?.aborted)
      throw error
    console.error(`[api] ${method} ${url} -> transport failure`, error?.msg || error)
    message.error(t('common.networkError'), {
      duration: 50000,
      closable: true,
    })
    throw new Error(error?.msg || 'Error')
  }

  beforeRequest?.()

  method = method || 'GET'

  const params = Object.assign(typeof data === 'function' ? data() : data ?? {}, {})
  if (!headers)
    headers = {}

  headers.lang = appStore.language
  return method === 'GET'
    ? request.get(url, { params, signal, onDownloadProgress }).then(successHandler, failHandler)
    : request.post(url, params, { headers, signal, onDownloadProgress }).then(successHandler, failHandler)
}

export function get<T = any>(
  { url, data, method = 'GET', onDownloadProgress, signal, beforeRequest, afterRequest }: HttpOption,
): Promise<Response<T>> {
  return http<T>({
    url,
    method,
    data,
    onDownloadProgress,
    signal,
    beforeRequest,
    afterRequest,
  })
}

export function post<T = any>(
  { url, data, method = 'POST', headers, onDownloadProgress, signal, beforeRequest, afterRequest }: HttpOption,
): Promise<Response<T>> {
  return http<T>({
    url,
    method,
    data,
    headers,
    onDownloadProgress,
    signal,
    beforeRequest,
    afterRequest,
  })
}

export default post
