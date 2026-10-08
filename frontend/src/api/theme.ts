import request from '@/utils/request/axios'
import { normalizeThemePackageV2, type ThemePackage, type ThemePackageV2 } from '@/utils/theme'

export interface ThemePreference {
  packageId: string
  mode: 'light' | 'dark' | 'auto'
  revision?: string
  /** 'custom' uses the user's own theme; 'follow-space' inherits the space's. */
  themeMode?: 'custom' | 'follow-space'
}

export interface ThemeRuntimeGrant {
  revision: string
  executionMode: 'sandbox' | 'trusted'
  available: boolean
  granted: boolean
  permissions: string[]
}

export interface ThemeRevisionSummary {
  id: string
  packageId: string
  version: string
  verified: boolean
  createdAt: string
}

export interface ThemeAdminPackageState {
  packages: Array<{ id: string; name: string; version: string; revision: string; verified: boolean }>
  revisions: ThemeRevisionSummary[]
  defaultPackage: string
  activeRevision: string
  lastGoodRevision: string
  pendingRevision: string
  trialStartedAt?: string | null
}

export function getCurrentTheme() {
  return request.get<{ code: number; data: ThemePackageV2 }>('/theme/v2/current').then((res) => {
    return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
  })
}

export function getPreviewTheme(token: string) {
  return request.get<{ code: number; data: ThemePackageV2 }>(`/theme/v2/preview/${encodeURIComponent(token)}`).then((res) => {
    return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
  })
}

export function getMyTheme() {
  return request.get<{ code: number; data: { package: ThemePackageV2; preference: ThemePreference } }>('/theme/v2/mine').then((res) => {
    return { ...res.data, data: { ...res.data.data, package: normalizeThemePackageV2(res.data.data.package) } }
  })
}

export type EffectiveThemeSource = 'user-space' | 'user' | 'space' | 'system' | 'builtin'

/**
 * The server-side resolution for a space. Anonymous visitors and signed-in users
 * share it; only the precedence chain differs. `spaceId` is optional so the
 * initial load (before a space is known) still resolves.
 */
export function getEffectiveTheme(spaceId?: number) {
  const query = spaceId ? `?spaceId=${encodeURIComponent(spaceId)}` : ''
  return request.get<{ code: number; data: { package: ThemePackageV2; revision: string; source: EffectiveThemeSource; mode: ThemePreference['mode'] } }>(`/theme/v2/effective${query}`).then((res) => {
    return { ...res.data, data: { ...res.data.data, package: normalizeThemePackageV2(res.data.data.package) } }
  })
}

export function getThemePreference() {
  return request.get<{ code: number; data: { mode: ThemePreference['mode'] } }>('/theme/v2/preference').then(res => res.data)
}

export function getInstalledThemes() {
  return request.get<{ code: number; data: Array<{ id: string; name: string; version: string; verified: boolean }> }>('/theme/v2/packages').then(res => res.data)
}

export function getThemePackage(id: string) {
  return request.get<{ code: number; data: ThemePackageV2 }>(`/theme/v2/package/${encodeURIComponent(id)}`).then((res) => {
    return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
  })
}

export function getThemeRevision(revision: string) {
	return request.get<{ code: number; data: ThemePackageV2 }>(`/theme/v2/revisions/${encodeURIComponent(revision)}`).then((res) => {
		return { ...res.data, data: normalizeThemePackageV2(res.data.data) }
	})
}

export function getSpaceTheme(spaceId: number) {
  return request.get<{ code: number; data: { spaceId: number; theme: { packageId: string; revision: string } | null; override: { packageId: string; revision: string } | null } }>(`/theme/v2/space/${spaceId}`).then(res => res.data)
}

export function setSpaceTheme(spaceId: number, selection: { packageId?: string; revision?: string }) {
  return request.put<{ code: number; msg: string }>(`/theme/v2/space/${spaceId}`, selection).then(res => res.data)
}

export function clearSpaceTheme(spaceId: number) {
  return request.delete<{ code: number; msg: string }>(`/theme/v2/space/${spaceId}`).then(res => res.data)
}

export function setUserSpaceTheme(spaceId: number, selection: { packageId?: string; revision?: string }) {
  return request.put<{ code: number; msg: string }>(`/theme/v2/space/${spaceId}/mine`, selection).then(res => res.data)
}

export function clearUserSpaceTheme(spaceId: number) {
  return request.delete<{ code: number; msg: string }>(`/theme/v2/space/${spaceId}/mine`).then(res => res.data)
}

export function saveThemePreference(preference: ThemePreference) {
  return request.post<{ code: number; msg: string }>('/theme/v2/preference', preference).then(res => res.data)
}

export function getThemeRuntimeGrant(revision: string, executionMode: ThemeRuntimeGrant['executionMode'] = 'sandbox') {
  return request.get<{ code: number; data: ThemeRuntimeGrant }>(`/theme/v2/grants/${encodeURIComponent(revision)}?executionMode=${executionMode}`).then(res => res.data)
}

export function setThemeRuntimeGrant(revision: string, permissions: string[], executionMode: ThemeRuntimeGrant['executionMode'] = 'sandbox') {
  return request.post<{ code: number; msg: string }>(`/theme/v2/grants/${encodeURIComponent(revision)}`, { executionMode, permissions }).then(res => res.data)
}

export function revokeThemeRuntimeGrant(revision: string, executionMode: ThemeRuntimeGrant['executionMode'] = 'sandbox') {
  return request.delete<{ code: number; msg: string }>(`/theme/v2/grants/${encodeURIComponent(revision)}?executionMode=${executionMode}`).then(res => res.data)
}

export function getTrustedThemeRuntimePolicy(revision: string) {
  return request.get<{ code: number; data: { revision: string; enabled: boolean } }>(`/theme/v2/admin/trusted/${encodeURIComponent(revision)}`).then(res => res.data)
}

export function setTrustedThemeRuntimePolicy(revision: string, enabled: boolean) {
  return request.put<{ code: number; msg: string }>(`/theme/v2/admin/trusted/${encodeURIComponent(revision)}`, { enabled }).then(res => res.data)
}

export function getThemePackages() {
	return request.get<{ code: number; data: ThemeAdminPackageState }>('/theme/v2/admin/packages').then(res => res.data)
}

export function beginThemeTrial(revision: string) {
	return request.post<{ code: number; msg: string; data?: { revision: string; expiresInSeconds: number } }>('/theme/v2/admin/trial', { revision }).then(res => res.data)
}

export function confirmThemeTrial(revision: string) {
	return request.post<{ code: number; msg: string }>('/theme/v2/admin/confirm', { revision }).then(res => res.data)
}

export function rollbackThemeTrial() {
	return request.post<{ code: number; msg: string }>('/theme/v2/admin/rollback').then(res => res.data)
}

export function installThemePackage(file: File, confirmUnverified: boolean) {
  const data = new FormData()
  data.append('package', file)
  data.append('confirmUnverified', String(confirmUnverified))
  return request.post<{ code: number; msg: string; data?: { id: string; revision: string; verified: boolean } }>('/theme/v2/admin/install', data).then(res => res.data)
}

export function previewThemePackage(file: File) {
  const data = new FormData()
  data.append('package', file)
  return request.post<{ code: number; msg: string; data?: { token: string; package: ThemePackageV2 } }>('/theme/v2/admin/preview', data).then((res) => {
    if (!res.data.data) return res.data as { code: number; msg: string; data?: { token: string; package: ThemePackage } }
    return { ...res.data, data: { ...res.data.data, package: normalizeThemePackageV2(res.data.data.package) } }
  })
}

export function setInstanceDefaultTheme(packageId: string, confirmExternalWallpaper = false) {
  return request.post<{ code: number; msg: string }>('/theme/v2/admin/default', { packageId, confirmExternalWallpaper }).then(res => res.data)
}

export function uploadWebWallpaper(file: File) {
  const data = new FormData()
  data.append('package', file)
  return request.post<{ code: number; msg: string; data?: { source: string; poster: string } }>('/theme/v2/wallpaper/web', data).then(res => res.data)
}

export function removeThemePackage(packageId: string) {
  return request.delete<{ code: number; msg: string }>(`/theme/v2/admin/packages/${encodeURIComponent(packageId)}`).then(res => res.data)
}
