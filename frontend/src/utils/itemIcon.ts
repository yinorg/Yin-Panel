import { getSiteFavicon } from '@/api/panel/itemIcon'

export async function getIconByUrl(url: string): Promise<Panel.ItemIcon | null> {
  try {
    const { code, data } = await getSiteFavicon<{ iconUrl: string; fileName: string }>(url)
    if (code !== 0 || !data?.iconUrl)
      return null

    return {
      itemType: 2,
      src: data.iconUrl,
      fileName: data.fileName,
    }
  }
  catch {
    return null
  }
}

export async function getAutomaticIconFileByUrl(url: string): Promise<File | null> {
  try {
    if (await isIconExtensionAvailable()) {
      const results = await fetchIconsFromExtension([url])
      const file = await iconFileFromBytes(results?.[0])
      if (file) return file.file
    }
  }
  catch { /* Extension unavailable. */ }
  return null
}

export async function getAutomaticIconByUrl(url: string): Promise<Panel.ItemIcon | null> {
  return getIconByUrl(url)
}

export async function fetchIconFile(url: string): Promise<{ file: File; hash: string; ext: string } | null> {
  try {
    const page = new URL(url)
    const response = await fetch(new URL('/favicon.ico', page.origin), { mode: 'cors' })
    if (!response.ok) return null
    const blob = await response.blob(); if (!blob.type.startsWith('image/')) return null
    const buffer = await blob.arrayBuffer(); const digest = await crypto.subtle.digest('SHA-256', buffer)
    const hash = [...new Uint8Array(digest)].map(v => v.toString(16).padStart(2, '0')).join('')
    const ext = ({ 'image/png': '.png', 'image/jpeg': '.jpg', 'image/gif': '.gif', 'image/webp': '.webp', 'image/svg+xml': '.svg', 'image/x-icon': '.ico' } as Record<string, string>)[blob.type] || '.ico'
    return { file: new File([blob], `${hash}${ext}`, { type: blob.type }), hash, ext }
  } catch { return null }
}

interface ExtensionIconResult { url: string; data?: string; bytes?: ArrayBuffer; mimeType?: string }

function createRequestId() {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID()
  if (typeof crypto.getRandomValues === 'function') {
    const bytes = new Uint8Array(16); crypto.getRandomValues(bytes)
    return [...bytes].map(value => value.toString(16).padStart(2, '0')).join('')
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

export function fetchIconsFromExtension(urls: string[], timeout = 30000): Promise<ExtensionIconResult[] | null> {
  if (typeof window === 'undefined' || !urls.length) return Promise.resolve([])
  const requestId = createRequestId()
  return new Promise(resolve => {
    let settled = false
    const finish = (value: ExtensionIconResult[] | null) => {
      if (settled) return
      settled = true
      window.removeEventListener('message', onMessage)
      window.clearTimeout(timer)
      resolve(value)
    }
    const onMessage = (event: MessageEvent) => {
      const data = event.data
      if (event.source !== window || data?.source !== 'yin-panel-extension' || data.requestId !== requestId) return
      finish(Array.isArray(data.items) ? data.items : null)
    }
    const timer = window.setTimeout(() => finish(null), timeout)
    window.addEventListener('message', onMessage)
    window.postMessage({ source: 'yin-panel', type: 'fetch-icons', requestId, urls }, '*')
  })
}

export function isIconExtensionAvailable(timeout = 5000): Promise<boolean> {
  if (typeof window === 'undefined') return Promise.resolve(false)
  const requestId = createRequestId()
  return new Promise(resolve => {
    const onMessage = (event: MessageEvent) => {
      if (event.source === window && event.data?.source === 'yin-panel-extension' && event.data.requestId === requestId) {
        window.removeEventListener('message', onMessage); window.clearTimeout(timer); resolve(true)
      }
    }
    const timer = window.setTimeout(() => { window.removeEventListener('message', onMessage); resolve(false) }, timeout)
    window.addEventListener('message', onMessage)
    window.postMessage({ source: 'yin-panel', type: 'ping', requestId }, '*')
  })
}

export async function iconFileFromBytes(item?: ExtensionIconResult): Promise<{ file: File; hash: string; ext: string } | null> {
  if ((!item?.data && !item?.bytes) || !item.mimeType) return null
  const bytes = item.data
    ? Uint8Array.from(atob(item.data), character => character.charCodeAt(0))
    : new Uint8Array(item.bytes!)
  if (bytes.byteLength === 0) return null
  const blob = new Blob([bytes], { type: item.mimeType })
  if (!blob.type.startsWith('image/')) return null
  const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer())
  const hash = [...new Uint8Array(digest)].map(v => v.toString(16).padStart(2, '0')).join('')
  const ext = ({ 'image/png': '.png', 'image/jpeg': '.jpg', 'image/gif': '.gif', 'image/webp': '.webp', 'image/svg+xml': '.svg', 'image/x-icon': '.ico', 'image/vnd.microsoft.icon': '.ico' } as Record<string, string>)[blob.type] || '.ico'
  return { file: new File([blob], `${hash}${ext}`, { type: blob.type }), hash, ext }
}
