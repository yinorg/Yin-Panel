import { get, post } from '../../utils/request'

export interface Space { id: number; type: 'personal' | 'team'; name: string; ownerUserId: number }
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
