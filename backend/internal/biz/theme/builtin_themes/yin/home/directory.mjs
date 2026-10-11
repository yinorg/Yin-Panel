// The directory home layout: a folder picker selects one root group and only its
// subtree is rendered, as rows rather than tiles. All of the state and DOM this
// needs lives here, which is what keeps the shared shell layout-agnostic.
const yinHomeLayout = (() => {
  let rootId
  let nav
  let requestRender = () => {}
  const nodes = new Map()

  const groupTree = groups => {
    const byId = new Map((groups || []).map(group => [String(group.id), group]))
    const children = new Map()
    for (const group of byId.values()) {
      const parent = group.parentId == null ? '' : String(group.parentId)
      const siblings = children.get(parent) || []
      siblings.push(group)
      children.set(parent, siblings)
    }
    return { byId, children, roots: children.get('') || [] }
  }

  const refreshNav = roots => {
    if (!nav) return
    const kept = new Set()
    for (const group of roots) {
      const key = String(group.id)
      kept.add(key)
      let button = nodes.get(key)
      if (!button) {
        button = element('button', 'directory-folder-button')
        button.type = 'button'
        button.addEventListener('click', () => {
          rootId = button.dataset.groupId
          requestRender()
        })
        nodes.set(key, button)
      }
      button.dataset.groupId = key
      button.textContent = group.title || 'Untitled group'
      button.classList.toggle('active', key === rootId)
      button.setAttribute('aria-pressed', key === rootId ? 'true' : 'false')
      nav.append(button)
    }
    for (const [key, button] of nodes) {
      if (!kept.has(key)) {
        button.remove()
        nodes.delete(key)
      }
    }
    nav.hidden = roots.length === 0
  }

  return {
    kind: 'directory',
    collectionGap: 18,
    allowDragEdit: false,
    createNav: render => {
      requestRender = render
      nav = element('nav', 'directory-folders')
      nav.setAttribute('aria-label', 'Bookmark groups')
      return nav
    },
    refreshNav,
    visibleGroups: groups => {
      const tree = groupTree(groups)
      refreshNav(tree.roots)
      if (!rootId || !tree.roots.some(group => String(group.id) === rootId)) {
        rootId = tree.roots.length ? String(tree.roots[0].id) : undefined
        refreshNav(tree.roots)
      }
      const selectedRoot = tree.roots.find(group => String(group.id) === rootId)
      const visible = []
      const walk = (group, depth) => {
        visible.push({ group, depth })
        for (const child of tree.children.get(String(group.id)) || []) walk(child, depth + 1)
      }
      if (selectedRoot) walk(selectedRoot, 0)
      return visible
    },
    decorateGroup: (section, _group, depth) => {
      section._yin.title.hidden = depth === 0
      section._yin.title.style.marginInlineStart = `${Math.min(8, depth) * 18}px`
    },
    itemChrome: () => ({ hideTitle: false, hideDescription: true, info: false }),
  }
})()
