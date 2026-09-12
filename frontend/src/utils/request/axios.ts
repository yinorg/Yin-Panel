import axios, { type AxiosResponse } from 'axios'
import { useAuthStore } from '../../store'

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
})

service.interceptors.request.use(
  (config) => {
    const authStore = useAuthStore()
    const token = authStore.token
    
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
    else
      config.headers.Authorization = `Bearer ${token}`
    
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
