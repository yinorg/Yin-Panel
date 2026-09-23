import request from '@/utils/request/axios'
import type { ThemePackage } from '@/utils/theme'

export interface ThemePreference {
  packageId: string
  mode: 'light' | 'dark' | 'auto'
}

export function getCurrentTheme() {
  return request.get<{ code: number; data: ThemePackage }>('/theme/current').then(res => res.data)
}

export function getMyTheme() {
  return request.get<{ code: number; data: { package: ThemePackage; preference: ThemePreference } }>('/theme/mine').then(res => res.data)
}

export function getInstalledThemes() {
  return request.get<{ code: number; data: Array<{ id: string; name: string; version: string; verified: boolean }> }>('/theme/packages').then(res => res.data)
}

export function saveThemePreference(preference: ThemePreference) {
  return request.post<{ code: number; msg: string }>('/theme/preference', preference).then(res => res.data)
}

export function getThemePackages() {
  return request.get<{ code: number; data: { packages: Array<{ id: string; name: string; version: string; verified: boolean }>; defaultPackage: string } }>('/theme/admin/packages').then(res => res.data)
}

export function installThemePackage(file: File, confirmUnverified: boolean) {
  const data = new FormData()
  data.append('package', file)
  data.append('confirmUnverified', String(confirmUnverified))
  return request.post<{ code: number; msg: string; data?: { id: string; verified: boolean } }>('/theme/admin/install', data).then(res => res.data)
}

export function setInstanceDefaultTheme(packageId: string) {
  return request.post<{ code: number; msg: string }>('/theme/admin/default', { packageId }).then(res => res.data)
}

export function removeThemePackage(packageId: string) {
  return request.delete<{ code: number; msg: string }>(`/theme/admin/packages/${encodeURIComponent(packageId)}`).then(res => res.data)
}
