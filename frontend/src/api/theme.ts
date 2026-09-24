import request from '@/utils/request/axios'
import { normalizeThemePackageV2, type ThemePackage, type ThemePackageV2 } from '@/utils/theme'

export interface ThemePreference {
  packageId: string
  mode: 'light' | 'dark' | 'auto'
  revision?: string
}

export interface ThemeRuntimeGrant {
  revision: string
  executionMode: 'sandbox'
  granted: boolean
  permissions: string[]
}

export function getCurrentTheme() {
  return request.get<{ code: number; data: ThemePackageV2 }>('/theme/current').then((res) => {
    return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
  })
}

export function getPreviewTheme(token: string) {
  return request.get<{ code: number; data: ThemePackageV2 }>(`/theme/preview/${encodeURIComponent(token)}`).then((res) => {
    return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
  })
}

export function getMyTheme() {
  return request.get<{ code: number; data: { package: ThemePackageV2; preference: ThemePreference } }>('/theme/mine').then((res) => {
    return { ...res.data, data: { ...res.data.data, package: normalizeThemePackageV2(res.data.data.package) } }
  })
}

export function getInstalledThemes() {
  return request.get<{ code: number; data: Array<{ id: string; name: string; version: string; verified: boolean }> }>('/theme/packages').then(res => res.data)
}

export function getThemePackage(id: string) {
  return request.get<{ code: number; data: ThemePackageV2 }>(`/theme/packages/${encodeURIComponent(id)}`).then((res) => {
    return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
  })
}

export function saveThemePreference(preference: ThemePreference) {
  return request.post<{ code: number; msg: string }>('/theme/preference', preference).then(res => res.data)
}

export function getThemeRuntimeGrant(revision: string) {
  return request.get<{ code: number; data: ThemeRuntimeGrant }>(`/theme/v2/grants/${encodeURIComponent(revision)}`).then(res => res.data)
}

export function setThemeRuntimeGrant(revision: string, permissions: string[]) {
  return request.post<{ code: number; msg: string }>(`/theme/v2/grants/${encodeURIComponent(revision)}`, { permissions }).then(res => res.data)
}

export function revokeThemeRuntimeGrant(revision: string) {
  return request.delete<{ code: number; msg: string }>(`/theme/v2/grants/${encodeURIComponent(revision)}`).then(res => res.data)
}

export function getThemePackages() {
  return request.get<{ code: number; data: { packages: Array<{ id: string; name: string; version: string; verified: boolean }>; defaultPackage: string } }>('/theme/admin/packages').then(res => res.data)
}

export function installThemePackage(file: File, confirmUnverified: boolean) {
  const data = new FormData()
  data.append('package', file)
  data.append('confirmUnverified', String(confirmUnverified))
  return request.post<{ code: number; msg: string; data?: { id: string; revision: string; verified: boolean } }>('/theme/admin/install', data).then(res => res.data)
}

export function previewThemePackage(file: File) {
  const data = new FormData()
  data.append('package', file)
  return request.post<{ code: number; msg: string; data?: { token: string; package: ThemePackageV2 } }>('/theme/admin/preview', data).then((res) => {
    if (!res.data.data) return res.data as { code: number; msg: string; data?: { token: string; package: ThemePackage } }
    return { ...res.data, data: { ...res.data.data, package: normalizeThemePackageV2(res.data.data.package) } }
  })
}

export function setInstanceDefaultTheme(packageId: string, confirmExternalWallpaper = false) {
  return request.post<{ code: number; msg: string }>('/theme/admin/default', { packageId, confirmExternalWallpaper }).then(res => res.data)
}

export function uploadWebWallpaper(file: File) {
  const data = new FormData()
  data.append('package', file)
  return request.post<{ code: number; msg: string; data?: { source: string; poster: string } }>('/theme/wallpaper/web', data).then(res => res.data)
}

export function removeThemePackage(packageId: string) {
  return request.delete<{ code: number; msg: string }>(`/theme/admin/packages/${encodeURIComponent(packageId)}`).then(res => res.data)
}
