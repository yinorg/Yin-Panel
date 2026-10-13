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
  let editing = false
  let tree = null
  let activeIds = new Set()
  let hoverTimer = null
  const nodes = new Map()
  const sections = new Map()

  // ---- Editing keeps the folder view ---------------------------------
  //
  // The shell serializes group order from the content DOM, so every root has to
  // be in it while a drag is staged. Only one root is ever visible, though: the
  // page keeps the shape it has while browsing, and a hover opens a folder
  // instead of replacing the view.

  const subtreeIds = key => {
    const ids = new Set()
    if (!tree || !key) return ids
    const root = tree.roots.find(group => String(group.id) === String(key))
    if (!root) return ids
    const walk = group => {
      ids.add(String(group.id))
      for (const child of tree.children.get(String(group.id)) || []) walk(child)
    }
    walk(root)
    return ids
  }

  const applyRootVisibility = () => {
    for (const [id, section] of sections) {
      if (!editing) { section.hidden = false; continue }
      section.hidden = !activeIds.has(id)
    }
  }

  const activateRoot = key => {
    if (!key || String(key) === rootId) return
    rootId = String(key)
    activeIds = subtreeIds(rootId)
    for (const [id, button] of nodes) {
      const active = id === rootId
      button.classList.toggle('active', active)
      button.setAttribute('aria-pressed', active ? 'true' : 'false')
    }
    applyRootVisibility()
  }

  // Hovering a folder opens it, so a dragged item can be carried into a folder
  // that is not the one on screen. The shell's drag listens on the document and
  // never captures the pointer, so these events still arrive mid-drag.
  const openDelay = 600
  const scheduleOpenRoot = key => {
    if (!editing || !key || String(key) === rootId) return
    clearTimeout(hoverTimer)
    hoverTimer = setTimeout(() => { hoverTimer = null; activateRoot(key) }, openDelay)
  }
  const cancelOpenRoot = () => { clearTimeout(hoverTimer); hoverTimer = null }

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
          // In edit mode the active tab is already the one on screen, so clicking
          // it renames that root instead of switching to it.
          if (editing) {
            if (String(button.dataset.groupId) === String(rootId)) {
              const section = sections.get(String(button.dataset.groupId))
              const rename = section && section._yin && section._yin.editGroup
              if (rename) { rename.click(); return }
            }
            activateRoot(button.dataset.groupId)
            return
          }
          rootId = button.dataset.groupId
          requestRender()
        })
        button.addEventListener('pointerdown', event => {
          if (event.button === 0) beginTabDrag(event, button)
        })
        button.addEventListener('pointerenter', () => scheduleOpenRoot(key))
        button.addEventListener('pointermove', () => scheduleOpenRoot(key))
        button.addEventListener('pointerleave', cancelOpenRoot)
        nodes.set(key, button)
      }
      button.dataset.groupId = key
      button.textContent = group.title || 'Untitled group'
      button.classList.toggle('active', key === rootId)
      button.setAttribute('aria-pressed', key === rootId ? 'true' : 'false')
      // The active tab is the one a user reaches for, and in edit mode it is the
      // only tab that renames its root, so it is the only one that says so. The
      // wording comes off the shell's rename button, which is already localised.
      const section = sections.get(key)
      const rename = section && section._yin && section._yin.editGroup
      const renameLabel = rename && rename.getAttribute('aria-label')
      if (editing && key === rootId && renameLabel) button.title = renameLabel
      else button.removeAttribute('title')
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

  // ---- Reordering folders from the strip -----------------------------
  //
  // Root order lives in the tab strip, not in the content area: the content
  // shows one root at a time, so its DOM cannot hold every root sibling. Moving
  // the tab and its section together keeps the shell's DOM-ordered
  // serialization correct without the shell knowing anything about tabs.

  let tabDrag = null

  const tabDropTarget = () => {
    if (!tabDrag || !tabs) return null
    for (const button of tabs.querySelectorAll('.directory-folder-button')) {
      if (button === tabDrag.button || !button.offsetParent) continue
      const rect = button.getBoundingClientRect()
      if (tabDrag.lastX < rect.left + rect.width / 2) return button
    }
    return null
  }

  const onTabDragMove = event => {
    if (!tabDrag) return
    tabDrag.lastX = event.clientX
    if (!tabDrag.moved) {
      if (Math.abs(event.clientX - tabDrag.originX) < 6) return
      tabDrag.moved = true
      tabDrag.button.classList.add('is-dragging')
      document.body.append(tabDrag.ghost)
    }
    event.preventDefault()
    tabDrag.ghost.style.left = `${event.clientX - tabDrag.offsetX}px`
    tabDrag.ghost.style.top = `${event.clientY - tabDrag.offsetY}px`
    const before = tabDropTarget()
    if (before) tabs.insertBefore(tabDrag.button, before)
    else tabs.append(tabDrag.button)
  }

  // The shell reads group order from the content DOM, so the sections follow the
  // strip. Re-appending only the roots is enough: a group's siblings all share
  // its parent, and the shell indexes siblings per parent.
  const syncSectionsToTabs = () => {
    const ordered = [...tabs.querySelectorAll('.directory-folder-button')]
      .map(button => sections.get(button.dataset.groupId))
      .filter(Boolean)
    if (!ordered.length) return
    const collection = ordered[0].parentElement
    if (!collection) return
    for (const section of ordered) collection.append(section)
  }

  const onTabDragEnd = () => {
    document.removeEventListener('pointermove', onTabDragMove)
    document.removeEventListener('pointerup', onTabDragEnd)
    document.removeEventListener('pointercancel', onTabDragEnd)
    if (!tabDrag) return
    const { button, ghost, moved } = tabDrag
    button.classList.remove('is-dragging')
    ghost.remove()
    tabDrag = null
    if (moved) { syncSectionsToTabs(); syncPager() }
  }

  const beginTabDrag = (event, button) => {
    if (tabDrag || !editing) return
    const rect = button.getBoundingClientRect()
    const ghost = button.cloneNode(true)
    ghost.classList.add('yin-drag-ghost')
    ghost.style.width = `${rect.width}px`
    ghost.style.height = `${rect.height}px`
    tabDrag = { button, ghost, offsetX: event.clientX - rect.left, offsetY: event.clientY - rect.top, lastX: event.clientX, originX: event.clientX, moved: false }
    document.addEventListener('pointermove', onTabDragMove, { passive: false })
    document.addEventListener('pointerup', onTabDragEnd)
    document.addEventListener('pointercancel', onTabDragEnd)
  }

  return {
    kind: 'directory',
    // The tab bar is the card's top edge, so the collection sits flush under it.
    collectionGap: 0,
    allowDragEdit: true,
    // Editing has to see an empty group to be able to drop into it.
    hideEmptyGroups: () => !editing,
    onEditingChange: value => {
      editing = value === true
      if (!editing) cancelOpenRoot()
      activeIds = editing ? subtreeIds(rootId) : new Set()
    },
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
      tree = groupTree(groups)
      refreshNav(tree.roots)
      if (!rootId || !tree.roots.some(group => String(group.id) === rootId)) {
        rootId = tree.roots.length ? String(tree.roots[0].id) : undefined
        refreshNav(tree.roots)
      }
      const visible = []
      const walk = (group, depth) => {
        visible.push({ group, depth })
        for (const child of tree.children.get(String(group.id)) || []) walk(child, depth + 1)
      }
      // While a drag is staged the whole tree is rendered so the shell can
      // serialize group order from it; only the open root is ever visible.
      if (editing) {
        for (const root of tree.roots) walk(root, 0)
        activeIds = subtreeIds(rootId)
        return visible
      }
      const selectedRoot = tree.roots.find(group => String(group.id) === rootId)
      if (selectedRoot) walk(selectedRoot, 0)
      return visible
    },
    decorateGroup: (section, group, depth) => {
      // The selected root is already named by the tab, so its own row carries no
      // label; deeper groups keep theirs and indent so the hierarchy reads.
      section._yin.title.hidden = depth === 0
      section._yin.title.style.marginInlineStart = depth > 0 ? `${Math.min(3, depth) * 12}px` : '0px'
      section._yin.toggle.hidden = true
      sections.set(String(group.id), section)
      // Only the open folder is shown while editing; the rest stay in the DOM so
      // the shell can serialize the complete group order.
      if (editing) section.hidden = !activeIds.has(String(group.id))
      // A root has no rendered siblings here, and the content area cannot place
      // it against siblings that are not in the DOM, so it is reordered by
      // dragging its tab instead.
      if (depth === 0 && editing) section.dataset.noGroupDrag = '1'
      else delete section.dataset.noGroupDrag
      const menu = buildGroupMenu(section)
      // Rows the viewer may not manage keep no trigger at all.
      menu.trigger.hidden = !menu.actions.some(action => !action.hidden)
    },
    itemChrome: () => ({ hideTitle: false, hideDescription: true, info: false }),
    // What this layout browses is a folder, not a space, so its own wording
    // applies here rather than the Core's single word for both layouts.
    emptyKey: 'collection.emptyFolder',
  }
})()
