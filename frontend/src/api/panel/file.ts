import request from '@/utils/request/axios'

export interface UploadedImage {
  imageUrl: string
  fileName: string
}

export function uploadImage(file: File) {
  const data = new FormData()
  data.append('imgfile', file, file.name)
  return request.post<{ code: number; msg?: string; data?: UploadedImage }>('/file/uploadImg', data).then(response => response.data)
}
