import { get, post } from '../../utils/request'
import { t } from '../../locales'

export interface Space { id: number; type: 'personal' | 'team' | 'shared'; name: string; ownerUserId: number; publicEnabled?: boolean; publicId?: string; publicMode?: 'direct' | 'code' }
export interface PublicConfig { enabled: boolean; publicId: string; mode: 'direct' | 'code'; accessCode?: string }
export interface SpaceMember { id: number; userId: number; email?: string; role: 'admin' | 'editor' | 'viewer'; source?: string }
export function spaceDisplayName(space: Space, spaces: Space[], currentUserId?: number, memberView = false) {
  if (space.type === 'personal' && !memberView && space.ownerUserId === currentUserId)
    return t('spaceManage.mySpace')
  const sameName = spaces.filter(item => item.name === space.name)
  return sameName.length > 1 ? `${space.name} ${space.id}` : space.name
}
export function sortSpaces(spaces: Space[], currentUserId?: number) {
  return [...spaces].sort((a, b) => {
    const aMine = a.type === 'personal' && a.ownerUserId === currentUserId ? 0 : 1
    const bMine = b.type === 'personal' && b.ownerUserId === currentUserId ? 0 : 1
    return aMine - bMine || a.name.localeCompare(b.name) || a.id - b.id
  })
}
export function getSpaces<T>() { return get<T>({ url: '/spaces' }) }
export function getPublicConfig<T>(spaceId: number) { return get<T>({ url: `/spaces/${spaceId}/public` }) }
export function setPublicConfig<T>(spaceId: number, data: PublicConfig) { return post<T>({ url: `/spaces/${spaceId}/public`, data }) }
export function importBookmarks<T>(spaceId: number, data: any) { return post<T>({ url: `/spaces/${spaceId}/bookmarks/import`, data }) }
export function clearSpace<T>(spaceId: number) { return post<T>({ url: `/spaces/${spaceId}/clear` }) }
export function createTeam<T>(name: string) { return post<T>({ url: '/spaces/teams', data: { name } }) }
export function getGroups<T>(spaceId: number) { return get<T>({ url: `/spaces/${spaceId}/groups` }) }
export function getItems<T>(spaceId: number, groupId?: number, page = 1, pageSize = 50) { return get<T>({ url: `/spaces/${spaceId}/items`, data: { groupId, page, pageSize } }) }
export function createItem<T>(spaceId: number, data: any) { return post<T>({ url: `/spaces/${spaceId}/items`, data }) }
export function updateItem<T>(spaceId: number, id: number, data: any) { return post<T>({ url: `/spaces/${spaceId}/items/${id}/update`, data }) }
export function deleteItem<T>(spaceId: number, id: number) { return post<T>({ url: `/spaces/${spaceId}/items/${id}/delete` }) }
export function sortItems<T>(spaceId: number, groupId: number, sortItems: any[]) { return post<T>({ url: `/spaces/${spaceId}/items/sort`, data: { groupId, sortItems } }) }
export function createGroup<T>(spaceId: number, title: string, icon = '', parentId?: number | null) { return post<T>({ url: `/spaces/${spaceId}/groups`, data: { title, icon, parentId: parentId || null } }) }
export function updateGroup<T>(spaceId: number, groupId: number, title: string, icon = '', parentId?: number | null) { return post<T>({ url: `/spaces/${spaceId}/groups/${groupId}/update`, data: { title, icon, parentId: parentId || null } }) }
export function deleteGroup<T>(spaceId: number, groupId: number) { return post<T>({ url: `/spaces/${spaceId}/groups/${groupId}/delete` }) }
export function renameSpace<T>(spaceId: number, name: string) { return post<T>({ url: `/spaces/${spaceId}`, data: { name } }) }
export function copySpace<T>(spaceId: number, name?: string) { return post<T>({ url: `/spaces/${spaceId}/copy`, data: name ? { name } : {} }) }
export function transferSpace<T>(spaceId: number, email: string) { return post<T>({ url: `/spaces/${spaceId}/transfer`, data: { email } }) }
export function getMembers<T>(spaceId: number) { return get<T>({ url: `/spaces/${spaceId}/members` }) }
export function addMember<T>(spaceId: number, email: string, role: string) { return post<T>({ url: `/spaces/${spaceId}/members`, data: { email, role } }) }
export function updateMember<T>(spaceId: number, userId: number, role: string) { return post<T>({ url: `/spaces/${spaceId}/members/${userId}`, data: { role } }) }
export function removeMember<T>(spaceId: number, userId: number) { return post<T>({ url: `/spaces/${spaceId}/members/${userId}` }) }
export function getOIDCGroups<T>(spaceId: number) { return get<T>({ url: `/spaces/${spaceId}/oidc-groups` }) }
export function addOIDCGroup<T>(spaceId: number, provider: string, groupName: string, role: string) { return post<T>({ url: `/spaces/${spaceId}/oidc-groups`, data: { provider, groupName, role } }) }
export function deleteOIDCGroup<T>(spaceId: number, ruleId: number) { return post<T>({ url: `/spaces/${spaceId}/oidc-groups/${ruleId}` }) }
