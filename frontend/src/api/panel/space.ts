import { get, post } from '../../utils/request'

export interface Space { id: number; type: 'personal' | 'team' | 'shared'; name: string; ownerUserId: number }
export interface SpaceMember { id: number; userId: number; email?: string; role: 'admin' | 'editor' | 'viewer'; source?: string }
export function spaceDisplayName(space: Space, spaces: Space[], memberView = false) {
  if (space.type === 'personal' && !memberView)
    return '我的空间'
  const sameName = spaces.filter(item => item.name === space.name)
  return sameName.length > 1 ? `${space.name} ${space.id}` : space.name
}
export function getSpaces<T>() { return get<T>({ url: '/spaces' }) }
export function createTeam<T>(name: string) { return post<T>({ url: '/spaces/teams', data: { name } }) }
export function getGroups<T>(spaceId: number) { return get<T>({ url: `/spaces/${spaceId}/groups` }) }
export function getItems<T>(spaceId: number, groupId?: number) { return get<T>({ url: `/spaces/${spaceId}/items`, params: { groupId } }) }
export function createItem<T>(spaceId: number, data: any) { return post<T>({ url: `/spaces/${spaceId}/items`, data }) }
export function updateItem<T>(spaceId: number, id: number, data: any) { return post<T>({ url: `/spaces/${spaceId}/items/${id}/update`, data }) }
export function deleteItem<T>(spaceId: number, id: number) { return post<T>({ url: `/spaces/${spaceId}/items/${id}/delete` }) }
export function sortItems<T>(spaceId: number, groupId: number, sortItems: any[]) { return post<T>({ url: `/spaces/${spaceId}/items/sort`, data: { groupId, sortItems } }) }
export function createGroup<T>(spaceId: number, title: string, icon = '') { return post<T>({ url: `/spaces/${spaceId}/groups`, data: { title, icon } }) }
export function updateGroup<T>(spaceId: number, groupId: number, title: string, icon = '') { return post<T>({ url: `/spaces/${spaceId}/groups/${groupId}/update`, data: { title, icon } }) }
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
