import axios, { type AxiosResponse } from 'axios'

export function parsePublicCodeFromPath(): string {
  let publiccode = ''
  const pathSegments = window.location.pathname.split('/')
  if (pathSegments.length > 1 && pathSegments[1] !== '') {
    // Check if code format is valid (only letters and numbers, length of 10)
    if (/^[a-z][a-z0-9-]{4,28}[a-z0-9]$/.test(pathSegments[1])) {
      publiccode = pathSegments[1]
    }
  }
  return publiccode
}

function getPublicAccessCode(): string {
  const key = `yin-panel-public-access:${parsePublicCodeFromPath()}`
  return sessionStorage.getItem(key) || ''
}

const service = axios.create({
  baseURL: import.meta.env.VITE_GLOB_API_URL,
  // The session token now travels in an httpOnly cookie, so requests must send
  // credentials. The API is same-origin, so this only makes that explicit.
  withCredentials: true,
})

service.interceptors.request.use(
  (config) => {
    // 从 URL 路径中获取 public code
    const publiccode = parsePublicCodeFromPath()

    // 添加 publiccode 到请求头（如果存在）
    if (publiccode)
      config.headers.publiccode = publiccode
    if (publiccode) {
      const accessCode = getPublicAccessCode()
      if (accessCode)
        config.headers['Public-Access-Code'] = accessCode
    }
    // Signed-in requests carry no Authorization header: the httpOnly session
    // cookie is attached by the browser and cannot be read or forged by script.

    return config
  },
  (error) => {
    return Promise.reject(error.response)
  },
)

service.interceptors.response.use(
  (response: AxiosResponse): AxiosResponse => {
    if (response.status === 200)
      return response

    throw new Error(response.status.toString())
  },
  (error) => {
    return Promise.reject(error)
  },
)

export default service
