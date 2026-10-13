/**
 * The Chrome bookmark format, parsed and written here rather than on the server.
 *
 * It is a browser interchange format, and the import cannot leave the browser
 * anyway: the icons a bookmark lands with come from the extension when it is
 * installed, then from a favicon lookup, so the file is read where that is
 * possible. Keeping both directions in one module keeps them honest — the
 * round-trip test in tests/unit/bookmark-format.test.mjs fails if either drifts
 * from the other.
 */

export interface BookmarkLink {
  title: string
  url: string
}

export interface BookmarkNode {
  title: string
  items: BookmarkLink[]
  children: BookmarkNode[]
}

/** A folder a bookmark file lists before its first one, so its links still land somewhere. */
export const ROOT_FOLDER_FALLBACK = 'Untitled'

interface Token {
  kind: 'tag' | 'text'
  value: string
}

/**
 * Split the document into tags and the text between them.
 *
 * A tag ends at the first `>` *outside quotes*: Chrome writes icons as
 * `ICON="data:image/svg+xml,<svg …>"`, and matching `<[^>]*>` would cut that tag
 * in half and lose the link. Comments are read to `-->` for the same reason.
 */
function tokenize(html: string): Token[] {
  const tokens: Token[] = []
  let i = 0
  while (i < html.length) {
    if (html[i] !== '<') {
      const next = html.indexOf('<', i)
      const end = next === -1 ? html.length : next
      tokens.push({ kind: 'text', value: html.slice(i, end) })
      i = end
      continue
    }
    if (html.startsWith('<!--', i)) {
      const close = html.indexOf('-->', i + 4)
      i = close === -1 ? html.length : close + 3
      continue
    }
    let j = i + 1
    let quote = ''
    while (j < html.length) {
      const ch = html[j]
      if (quote) {
        if (ch === quote) quote = ''
      }
      else if (ch === '"' || ch === '\'') {
        quote = ch
      }
      else if (ch === '>') {
        break
      }
      j++
    }
    if (j >= html.length) {
      tokens.push({ kind: 'text', value: html.slice(i) })
      break
    }
    tokens.push({ kind: 'tag', value: html.slice(i + 1, j) })
    i = j + 1
  }
  return tokens
}

function tagName(raw: string): { name: string; closing: boolean } {
  const trimmed = raw.trim()
  const closing = trimmed.startsWith('/')
  const body = closing ? trimmed.slice(1) : trimmed
  const name = (body.split(/[\s/>]/, 1)[0] || '').toLowerCase()
  return { name, closing }
}

function extractHref(raw: string): string {
  const matched = raw.match(/\bHREF\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))/i)
  if (!matched)
    return ''
  return matched[1] ?? matched[2] ?? matched[3] ?? ''
}

/**
 * Collect the text of an element that starts just after its opening tag.
 *
 * The format closes `<H3>` and `<A>` itself, but variants exist that do not, so
 * an element without its closing tag ends at whatever tag comes next instead of
 * swallowing the rest of the document.
 */
function collectText(tokens: Token[], from: number, closing: string): { text: string; end: number } {
  let text = ''
  for (let i = from; i < tokens.length; i++) {
    const token = tokens[i]
    if (token.kind === 'text') {
      text += token.value
      continue
    }
    const { name, closing: isClosing } = tagName(token.value)
    if (isClosing && name === closing)
      return { text, end: i }
    return { text, end: i - 1 }
  }
  return { text, end: tokens.length - 1 }
}

export function unescapeHtml(value: string): string {
  return value
    .replace(/&#x([0-9a-f]+);/gi, (_, hex: string) => String.fromCodePoint(Number.parseInt(hex, 16)))
    .replace(/&#(\d+);/g, (_, dec: string) => String.fromCodePoint(Number.parseInt(dec, 10)))
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, '\'')
    // Last, so `&amp;lt;` still reads as the literal text `&lt;`.
    .replace(/&amp;/g, '&')
}

export function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

interface Frame {
  /** The folder this level belongs to; the root level has none. */
  owner: BookmarkNode | null
  nodes: BookmarkNode[]
}

/**
 * Read a Netscape bookmark file into the tree the Core stores.
 *
 * A folder is `<DT><H3>title</H3>` optionally followed by its `<DL>`; a link is
 * `<DT><A HREF="…">title</A>`. Two folders of the same name under different
 * parents stay distinct, which is the point of the exercise.
 */
export function parseBookmarkHtml(html: string, rootTitle: string = ROOT_FOLDER_FALLBACK): BookmarkNode[] {
  const tokens = tokenize(html)
  const stack: Frame[] = [{ owner: null, nodes: [] }]
  const rootLinks: BookmarkLink[] = []
  let pendingFolder: BookmarkNode | null = null

  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i]
    if (token.kind === 'text')
      continue
    const { name, closing } = tagName(token.value)

    if (name === 'dl') {
      if (closing) {
        if (stack.length > 1)
          stack.pop()
      }
      else if (pendingFolder) {
        // The preceding folder owns this level.
        stack.push({ owner: pendingFolder, nodes: pendingFolder.children })
        pendingFolder = null
      }
      // A level with no folder to own it carries nothing, so it is skipped.
      continue
    }

    if (name === 'dt') {
      // A `DT` opens the next entry, so the previous folder had no children.
      pendingFolder = null
      continue
    }

    if (name === 'h3') {
      const collected = collectText(tokens, i + 1, 'h3')
      i = collected.end
      const title = unescapeHtml(collected.text).trim()
      if (!title)
        continue
      const node: BookmarkNode = { title, items: [], children: [] }
      stack[stack.length - 1].nodes.push(node)
      pendingFolder = node
      continue
    }

    if (name === 'a') {
      const href = unescapeHtml(extractHref(token.value)).trim()
      const collected = collectText(tokens, i + 1, 'a')
      i = collected.end
      const title = unescapeHtml(collected.text).trim()
      if (!title || !href)
        continue
      const frame = stack[stack.length - 1]
      if (frame.owner)
        frame.owner.items.push({ title, url: href })
      else
        rootLinks.push({ title, url: href })
    }
  }

  const nodes = stack[0].nodes
  if (rootLinks.length)
    nodes.push({ title: rootTitle, items: rootLinks, children: [] })
  return nodes
}

const INDENT = '    '

function renderNodes(nodes: BookmarkNode[], depth: number, stamp: number): string {
  let out = ''
  const pad = INDENT.repeat(depth)
  const childPad = INDENT.repeat(depth + 1)
  for (const node of nodes) {
    out += `${pad}<DT><H3 ADD_DATE="${stamp}" LAST_MODIFIED="${stamp}">${escapeHtml(node.title)}</H3>\n`
    // A folder's own entries live inside its `DL`, which is the only place the
    // format puts them: written after it they parse back onto the parent level.
    if (node.items.length || node.children.length) {
      out += `${pad}<DL><p>\n`
      for (const link of node.items)
        out += `${childPad}<DT><A HREF="${escapeHtml(link.url)}" ADD_DATE="${stamp}">${escapeHtml(link.title)}</A>\n`
      out += renderNodes(node.children, depth + 1, stamp)
      out += `${pad}</DL><p>\n`
    }
  }
  return out
}

/** Write the tree back out in the format Chrome imports. */
export function buildBookmarkHtml(nodes: BookmarkNode[]): string {
  const stamp = Math.floor(Date.now() / 1000)
  let out = '<!DOCTYPE NETSCAPE-Bookmark-file-1>\n'
  out += '<!-- This is an automatically generated file.\n     It will be read and overwritten.\n     DO NOT EDIT! -->\n'
  out += '<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">\n'
  out += '<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n'
  out += renderNodes(nodes, 1, stamp)
  out += '</DL><p>\n'
  return out
}
