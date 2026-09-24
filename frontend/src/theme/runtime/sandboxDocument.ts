export function createThemeSandboxDocument(resourceOrigin: string, bootstrap = '') {
  const origin = new URL(resourceOrigin)
  if (!['http:', 'https:'].includes(origin.protocol) || origin.username || origin.password)
    throw new Error('Unsupported theme resource origin')
  const policy = `default-src 'none'; script-src 'unsafe-inline' blob:; style-src 'unsafe-inline' blob:; img-src data: blob: ${origin.origin}; font-src data: blob:; media-src data: blob:; connect-src 'none'; object-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'`
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${policy}"><meta name="viewport" content="width=device-width,initial-scale=1"><style>html,body,#theme-root{box-sizing:border-box;width:100%;height:100%;margin:0}*,*::before,*::after{box-sizing:inherit}body{overflow:auto}</style></head><body><div id="theme-root"></div><script>${bootstrap}</script></body></html>`
}
