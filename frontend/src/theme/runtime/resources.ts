export interface ThemeCssResource {
  path: string
  url?: string
  mediaType: string
}

export interface ThemeCssScope {
  /**
   * Rewrite `:root`/`html`/`body` to `:host` for a shadow-root mount.
   */
  shadow?: boolean
  /**
   * Scope every selector to a container selector (for a light-DOM mount), so a
   * theme's CSS cannot reach the Core chrome outside its own container. `:root`,
   * `html`, `body`, and `:host` are rewritten to the scope itself.
   */
  prefix?: string
}

export async function rewriteThemeStylesheet(
  source: string,
  stylesheetURL: string,
  resources: readonly ThemeCssResource[],
  assetURLs: Readonly<Record<string, string>>,
  baseURL?: string,
  scope: ThemeCssScope = {},
): Promise<string> {
  const { default: cssTree } = await import('css-tree')
  const stylesheet = new URL(stylesheetURL, baseURL)
  const tree = cssTree.parse(source, { positions: true, parseValue: true })

  cssTree.walk(tree, (node) => {
    if (scope.shadow && node.type === 'PseudoClassSelector' && node.name?.toLowerCase() === 'root')
      node.name = 'host'
    if (scope.shadow && node.type === 'TypeSelector' && ['html', 'body'].includes(node.name?.toLowerCase() || '')) {
      Object.assign(node, { type: 'PseudoClassSelector', name: 'host', children: null })
    }
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

  if (scope.prefix)
    prefixSelectors(cssTree, tree, scope.prefix)

  return cssTree.generate(tree)
}

/**
 * Prefix every selector with the scope selector. `:root`, `:host`, `html`, and
 * `body` collapse to the scope itself; anything else becomes a descendant of it.
 */
function prefixSelectors(cssTree: any, tree: any, prefix: string) {
  const lists: any[] = []
  cssTree.walk(tree, (node: any) => {
    if (node.type === 'SelectorList')
      lists.push(node)
  })
  for (const list of lists) {
    const rewritten = list.children.toArray()
      .map((selector: any) => scopeSelector(cssTree.generate(selector).trim(), prefix))
      .filter(Boolean)
    if (!rewritten.length) continue
    const rebuilt = cssTree.parse(rewritten.join(','), { context: 'selectorList' })
    list.children = rebuilt.children
  }
}

function scopeSelector(selector: string, prefix: string): string {
  const trimmed = selector.trim()
  if (!trimmed) return ''
  const root = trimmed.match(/^(?::root|:host|html|body)(?=$|[\s>+~.:#[(*])/i)
  if (root) {
    const rest = trimmed.slice(root[0].length).trim()
    return rest ? `${prefix} ${rest}` : prefix
  }
  return `${prefix} ${trimmed}`
}

function isThemeMediaResource(mediaType: string): boolean {
  return mediaType.startsWith('image/') || mediaType === 'font/woff2' || mediaType.startsWith('video/')
}
