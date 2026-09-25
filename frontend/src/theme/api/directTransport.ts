import type { ThemeApiRequest } from '../../../packages/theme-sdk/src/index'
import type { ThemeApiResponse } from './dispatcher'

export type ThemeApiDispatch = (value: unknown) => Promise<ThemeApiResponse | null>

export function createThemeApiDirectTransport(dispatch: ThemeApiDispatch) {
  return async (request: ThemeApiRequest): Promise<unknown> => {
    const response = await dispatch(request)
    if (!response || response.requestId !== request.requestId || response.contextVersion !== request.contextVersion)
      throw apiError('RUNTIME_UNAVAILABLE', 'Theme API dispatcher returned an invalid response')
    if (!response.ok)
      throw apiError(response.error?.code || 'RUNTIME_UNAVAILABLE', response.error?.message || 'Theme API request failed')
    return response.result
  }
}

function apiError(code: string, message: string) {
  return Object.assign(new Error(message), { code })
}
