import CryptoJS from 'crypto-js'

function bytesToWordArray(bytes) {
  const words = []
  for (let index = 0; index < bytes.length; index += 1)
    words[index >>> 2] = (words[index >>> 2] || 0) | bytes[index] << (24 - (index % 4) * 8)
  return CryptoJS.lib.WordArray.create(words, bytes.length)
}

export async function sha256Hex(bytes) {
  const view = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes)
  const subtle = globalThis.crypto?.subtle
  if (subtle) {
    const digest = await subtle.digest('SHA-256', view.slice().buffer)
    return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('')
  }
  return CryptoJS.SHA256(bytesToWordArray(view)).toString(CryptoJS.enc.Hex)
}
