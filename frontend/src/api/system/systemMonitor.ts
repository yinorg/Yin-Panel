import { post } from '../../utils/request'

export function getAll<T>() {
  return post<T>({
    url: '/system/monitor/getAll',
  })
}

export function getDiskStateByPath<T>(path: string) {
  return post<T>({
    url: '/system/monitor/getDiskStateByPath',
    data: { path },
  })
}

export function getDiskMountpoints<T>() {
  return post<T>({
    url: '/system/monitor/getDiskMountpoints',
  })
}

export function getEnableStatus<T>() {
  return post<T>({
    url: '/system/monitor/getEnableStatus',
  })
}

export function getSnapshot<T>() {
  return post<T>({ url: '/system/monitor/getSnapshot' })
}
