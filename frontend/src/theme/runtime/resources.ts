export interface ThemeCssResource {
  path: string
  url?: string
  mediaType: string
}

export async function rewriteThemeStylesheet(
  source: string,
  stylesheetURL: string,
  resources: readonly ThemeCssResource[],
  assetURLs: Readonly<Record<string, string>>,
): Promise<string> {
  const { default: cssTree } = await import('css-tree')
  const stylesheet = new URL(stylesheetURL)
  const tree = cssTree.parse(source, { positions: true, parseValue: true })

  cssTree.walk(tree, (node) => {
    if (node.type === 'Atrule' && (node.name?.toLowerCase() === 'import' || node.name?.includes('\\')))
      throw new Error('Theme stylesheets cannot use @import or escaped at-rule names')
    if (node.type === 'Function' && (node.name?.includes('\\') || ['image-set', '-webkit-image-set'].includes(node.name?.toLowerCase() || '')))
      throw new Error('Theme stylesheets cannot use escaped functions or image-set string fetches')
    if (node.type !== 'Url') return

    const value = node.value?.value.trim()
    const rawURL = value && node.value?.type === 'String' ? value.slice(1, -1) : value
    if (!rawURL || rawURL.includes('\\'))
      throw new Error('Theme stylesheet contains an invalid asset URL')

    let resolved: URL
    try {
      resolved = new URL(rawURL, stylesheet)
    }
    catch {
      throw new Error('Theme stylesheet contains an invalid asset URL')
    }
    const resource = resources.find((item) => {
      if (!item.url || !isThemeMediaResource(item.mediaType)) return false
      try { return new URL(item.url, stylesheet).href === resolved.href }
      catch { return false }
    })
    if (!resource || !assetURLs[resource.path])
      throw new Error(`Theme stylesheet references an undeclared asset: ${rawURL}`)

    node.value!.value = resource.mediaType === 'image/svg+xml'
      ? new URL(resource.url!, stylesheet).href
      : assetURLs[resource.path]
  })

  return cssTree.generate(tree)
}

function isThemeMediaResource(mediaType: string): boolean {
  return mediaType.startsWith('image/') || mediaType === 'font/woff2' || mediaType.startsWith('video/')
}
