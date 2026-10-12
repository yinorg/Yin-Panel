// The directory home layout: one tab per root group selects the subtree that is
// rendered, and every group in it becomes a row whose name sits in a leading
// label column with its bookmarks in a compact grid beside it. The switch at the
// right of the tab bar mirrors the paired Yin/Yang spaces. All of the state and
// DOM this needs lives here, which is what keeps the shared shell
// layout-agnostic.
const yinHomeLayout = (() => {
  let rootId
  let nav
  let tabs
  let pagerPrevious
  let pagerNext
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

  // ---- Paired-side switch -------------------------------------------

  // The tab bar is paged, not stepped: the arrows scroll the parent-group strip
  // left and right so more groups come into view, and never change the
  // selection. They are disabled at each end of the scroll range.
  const pageTabs = direction => {
    if (!tabs) return
    const step = Math.max(160, Math.round(tabs.clientWidth * 0.85))
    tabs.scrollBy({ left: direction * step, behavior: 'smooth' })
  }

  const syncPager = () => {
    if (!pagerPrevious || !tabs) return
    const max = Math.max(0, tabs.scrollWidth - tabs.clientWidth)
    pagerPrevious.disabled = tabs.scrollLeft <= 1
    pagerNext.disabled = max <= 1 || tabs.scrollLeft >= max - 1
  }

  // ---- Tab bar ------------------------------------------------------

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
      tabs.append(button)
    }
    for (const [key, button] of nodes) {
      if (!kept.has(key)) {
        button.remove()
        nodes.delete(key)
      }
    }
    // The strip's width is only measurable once it has been laid out.
    requestAnimationFrame(syncPager)
    nav.hidden = roots.length === 0
  }

  // ---- Per-row group menu -------------------------------------------
  //
  // The shell owns the five group actions; this layout only folds them into a
  // single trigger, so their permissions, disabled states and test ids stay
  // exactly as the shell produces them.

  let openMenu = null
  let documentListenersBound = false

  const closeGroupMenu = () => {
    if (!openMenu) return
    openMenu.popup.hidden = true
    openMenu.trigger.setAttribute('aria-expanded', 'false')
    openMenu = null
  }

  const openGroupMenu = entry => {
    closeGroupMenu()
    entry.popup.hidden = false
    entry.trigger.setAttribute('aria-expanded', 'true')
    openMenu = entry
  }

  const bindMenuDocumentListeners = () => {
    if (documentListenersBound) return
    documentListenersBound = true
    document.addEventListener('pointerdown', event => {
      if (!openMenu || openMenu.wrapper.contains(event.target)) return
      closeGroupMenu()
    })
    document.addEventListener('keydown', event => {
      if (event.key === 'Escape') closeGroupMenu()
    })
  }

  const buildGroupMenu = section => {
    if (section._yinMenu) return section._yinMenu
    const heading = section._yin.heading
    const wrapper = element('div', 'yin-group-menu-wrapper')
    const trigger = element('button', 'yin-group-menu', '⋯')
    trigger.type = 'button'
    trigger.hidden = true
    trigger.setAttribute('aria-haspopup', 'true')
    trigger.setAttribute('aria-expanded', 'false')
    trigger.setAttribute('aria-label', 'Group actions')
    trigger.dataset.testid = 'theme-group-menu'
    const popup = element('div', 'yin-group-menu-popup')
    popup.hidden = true
    popup.setAttribute('role', 'menu')
    const actions = [
      section._yin.addItem,
      section._yin.editGroup,
      section._yin.deleteGroup,
      section._yin.moveGroupUp,
      section._yin.moveGroupDown,
    ]
    for (const action of actions) {
      if (!action) continue
      const label = action.getAttribute('aria-label') || ''
      if (label && !action.querySelector('.yin-group-menu-label'))
        action.append(element('span', 'yin-group-menu-label', label))
      popup.append(action)
    }
    const controls = heading.querySelector('.yin-group-controls')
    if (controls) controls.hidden = true
    const entry = { wrapper, trigger, popup, actions: actions.filter(Boolean) }
    trigger.addEventListener('click', event => {
      event.stopPropagation()
      if (popup.hidden) openGroupMenu(entry)
      else closeGroupMenu()
    })
    // Selecting any action closes the menu; the action's own handler still runs.
    popup.addEventListener('click', () => closeGroupMenu())
    wrapper.append(trigger, popup)
    heading.append(wrapper)
    bindMenuDocumentListeners()
    section._yinMenu = entry
    return entry
  }

  return {
    kind: 'directory',
    // The tab bar is the card's top edge, so the collection sits flush under it.
    collectionGap: 0,
    allowDragEdit: false,
    // The row list is a table of groups: a group with no bookmarks is not a row.
    hideEmptyGroups: true,
    createNav: actions => {
      requestRender = actions.requestRender
      nav = element('nav', 'directory-folders')
      nav.setAttribute('aria-label', 'Bookmark groups')
      tabs = element('div', 'directory-tabs')
      // The wallpaper centre is a Core surface, not a theme view: the Core owns
      // the library, the layer behind the panel and the stored choice, so any
      // theme opens the exact same picker.
      const wallpaperButton = element('button', 'directory-wallpaper-button')
      wallpaperButton.type = 'button'
      wallpaperButton.dataset.testid = 'theme-wallpaper-center'
      wallpaperButton.setAttribute('aria-label', '壁纸中心')
      wallpaperButton.setAttribute('title', '壁纸中心')
      wallpaperButton.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path fill="currentColor" d="M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zm1 12h14l-4.6-6.1-3 3.7L9 11z"/><circle cx="8.2" cy="9.2" r="1.6" fill="currentColor"/></svg>'
      wallpaperButton.addEventListener('click', event => {
        event.stopPropagation()
        Promise.resolve(actions.openCoreSurface('wallpaper-center')).catch(() => {})
      })
      pagerPrevious = element('button', 'directory-page', '\u2039')
      pagerPrevious.type = 'button'
      pagerPrevious.dataset.testid = 'theme-group-previous'
      pagerPrevious.setAttribute('aria-label', '向前翻页父分组')
      pagerPrevious.addEventListener('click', () => pageTabs(-1))
      pagerNext = element('button', 'directory-page', '\u203a')
      pagerNext.type = 'button'
      pagerNext.dataset.testid = 'theme-group-next'
      pagerNext.setAttribute('aria-label', '向后翻页父分组')
      pagerNext.addEventListener('click', () => pageTabs(1))
      const pager = element('div', 'directory-pager')
      pager.append(pagerPrevious, pagerNext)
      const actionsBar = element('div', 'directory-actions')
      actionsBar.append(wallpaperButton, pager)
      nav.append(tabs, actionsBar)
      tabs.addEventListener('scroll', () => syncPager(), { passive: true })
      window.addEventListener('resize', () => syncPager())
      syncPager()
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
      // The selected root is already named by the tab, so its own row carries no
      // label; deeper groups keep theirs and indent so the hierarchy reads.
      section._yin.title.hidden = depth === 0
      section._yin.title.style.marginInlineStart = depth > 0 ? `${Math.min(3, depth) * 12}px` : '0px'
      section._yin.toggle.hidden = true
      const menu = buildGroupMenu(section)
      // Rows the viewer may not manage keep no trigger at all.
      menu.trigger.hidden = !menu.actions.some(action => !action.hidden)
    },
    itemChrome: () => ({ hideTitle: false, hideDescription: true, info: false }),
  }
})()
