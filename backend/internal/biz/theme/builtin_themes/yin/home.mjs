const element = (tag, className, text) => {
  const value = document.createElement(tag)
  if (className) value.className = className
  if (text !== undefined) value.textContent = text
  return value
}

const textValue = value => typeof value === 'string' ? value : ''

// 诊断开关来自 Core 快照的 `diagnostics.showClickTrace`。
//
// 它不能从 URL 读。主题文档是 `srcdoc`，实测三条路都不通：
//   location.href   = "about:srcdoc"（没有查询串）
//   document.referrer = ""（空）
//   parent.location  = SecurityError（沙箱禁止）
// 所以面板 URL 上的 `?yinDiag=1` 无法到达主题，只能由 Core 在组装快照时告诉它。
//
// 它存在的理由：主题跑在一个 opaque 的沙箱 iframe 里，父文档和外部工具都读不到它内部
// 的状态——从外面 `frame.contentWindow.go()` 是 SecurityError，在顶层 Console 包
// `window.open` 也看不到框内的调用（那是另一个 window）。于是「点了没反应」这件事
// 从外面完全不可观测，只能靠猜。把结果直接渲染在主题自己的文档里，它就出现在屏幕上。
let diagOn = false

// 面板保留全部记录，不覆盖：一次点击会产生多条（事件、手势、开窗结果），
// 互相覆盖会让真正需要的那条恰好消失——这正是它第一版没给出答案的原因。
const diag = lines => {
  if (!diagOn) return
  let box = document.querySelector('.yin-diag')
  if (!box) {
    box = element('pre', 'yin-diag')
    document.body.append(box)
  }
  if (box.textContent) box.textContent = `${box.textContent}\n${'─'.repeat(24)}\n${lines.join('\n')}`
  else box.textContent = lines.join('\n')
  box.scrollTop = box.scrollHeight
}

// 记录一次点击到底发生了什么。每一项都是可直接读的事实，不含推断。
const diagClick = (label, detail) => {
  if (!diagOn) return
  const activation = (() => {
    try {
      const ua = navigator.userActivation
      return `active=${ua.isActive} hasBeenActive=${ua.hasBeenActive}`
    } catch { return 'userActivation=unavailable' }
  })()
  diag([
    `点击: ${label}`,
    `时间: ${new Date().toISOString()}`,
    `视口: ${innerWidth}x${innerHeight}`,
    `手势(userActivation): ${activation}`,
    ...detail,
  ])
}

export default {
  apiVersion: '1.0.0',
  setup(api) {
    let root
    let snapshot
    // Theme UI text is owned by the Core: the theme holds no locale bundle and
    // falls back to its own English when a label is missing (older Core).
    const envLabel = (key, fallback) => api.environment?.get?.()?.labels?.[key] || fallback
    let sideSwitching = false
    let query = ''
    let directoryRootId
    let clockTimer
    let collectionLayoutFrame
    let groupDialog
    let groupDialogTitle
    let groupDialogIcon
    let groupDialogHeading
    let groupDialogSubmit
    let groupDialogError
    let contextMenu
    let actionBar
    let addGroupButton
    let commandCenterButton
    let styleButton
    let pageButton
    let networkMode = 'wan'
    let reportedSearchBottom = -1
    let groupDialogValue
    let groupDialogOpener
    let contextItemTrigger
    const inFlight = new Set()
    const collapsedGroups = new Set()
    const groupNodes = new Map()
    const itemNodes = new Map()
    const directoryNodes = new Map()

    // Drag edit mode (see enterEditMode): while editing, the DOM is the source of
    // truth, so renderCollection is frozen and the saved layout is diffed against
    // the baseline captured on entry. Order is stored on the shared records, so a
    // save reorders the space for every user.
    let editMode = false
    let editBaseline = null
    let dragState = null
    let autoScrollFrame
    let saveButton
    let cancelButton
    let lastEditToken = Number(api.environment?.get?.()?.editToken) || 0

    const run = async action => {
      try { await action() }
      catch (error) { if (status) status.textContent = error?.message || 'Action failed' }
    }

    // Deliberately excludes snapshot.permissions: that is authorization (may the
    // theme ever write?) while these sources are space-scoped (may it write
    // *here*?). Mixing them would re-expose write affordances on a read-only
    // Space just because the user granted write scope elsewhere.
    const globalCapabilitySources = () => [
      snapshot?.capabilities,
      snapshot?.activeSpaceCapabilities,
      snapshot?.presentation?.capabilities,
      ...(snapshot?.groups || []).map(group => group.capabilities),
    ].filter(Array.isArray)

    const hasGlobalCapability = (...names) => globalCapabilitySources().some(capabilities =>
      capabilities.some(capability => names.includes(capability)))

    // Authorization for Core-owned surfaces. A theme mounted without a grant has
    // an empty set, so these entries hide instead of failing on click.
    const hasPermission = name => (snapshot?.permissions || []).includes(name)

    const canCreateItems = () => hasGlobalCapability('items.write', 'item.create')
      || (snapshot?.items || []).some(item => (item.capabilities || []).some(capability => ['item.update', 'item.delete'].includes(capability)))
    const canEditItem = item => hasGlobalCapability('items.write') || (item?.capabilities || []).includes('item.update')
    const canDeleteItem = item => hasGlobalCapability('items.write') || (item?.capabilities || []).includes('item.delete')
    const canReorderItems = group => hasGlobalCapability('items.write', 'items.reorder')
      || (group?.capabilities || []).includes('items.reorder')
    const canMutateGroup = action => hasGlobalCapability('groups.write', `group.${action}`)
      || (action === 'reorder' && hasGlobalCapability('groups.reorder'))

    const mutate = async (key, button, command, payload) => {
      if (inFlight.has(key)) return false
      inFlight.add(key)
      if (button) button.disabled = true
      if (status) status.textContent = ''
      try {
        await api.commands.execute(command, payload)
        return true
      } catch (error) {
        if (status) status.textContent = error?.message || 'Action failed'
        return false
      } finally {
        inFlight.delete(key)
        if (button?.isConnected) button.disabled = false
      }
    }

    const openEditor = (key, payload, button) => mutate(key, button, 'editor.open', payload)

    const addActionButton = (container, text, testId, label, action, disabled = false) => {
      const button = element('button', 'yin-action-button', text)
      button.type = 'button'
      if (container === contextMenu) button.setAttribute('role', 'menuitem')
      button.dataset.testid = testId
      button.setAttribute('aria-label', label)
      button.disabled = disabled
      button.addEventListener('click', () => action(button))
      container.append(button)
      return button
    }

    const showGroupDialog = (mode, group) => {
      groupDialogOpener = document.activeElement
      groupDialogValue = { mode, group }
      groupDialogHeading.textContent = mode === 'edit' ? envLabel('dialog.editGroup', 'Edit group') : envLabel('dialog.addGroup', 'Add group')
      groupDialogSubmit.textContent = mode === 'edit' ? envLabel('dialog.save', 'Save group') : envLabel('dialog.create', 'Create group')
      groupDialogTitle.value = group?.title || ''
      groupDialogIcon.value = group?.icon || ''
      groupDialogError.textContent = ''
      groupDialog.hidden = false
      groupDialogTitle.focus()
      if (mode === 'edit') groupDialogTitle.select()
    }

    const closeGroupDialog = () => {
      groupDialog.hidden = true
      groupDialogValue = undefined
      groupDialogError.textContent = ''
      if (groupDialogOpener?.isConnected) groupDialogOpener.focus()
      groupDialogOpener = undefined
    }

    const submitGroupDialog = async () => {
      if (!groupDialogValue || groupDialogSubmit.disabled) return
      const title = groupDialogTitle.value.trim()
      if (!title) {
        groupDialogError.textContent = envLabel('dialog.nameRequired', 'Enter a group name.')
        groupDialogTitle.focus()
        return
      }
      const { mode, group } = groupDialogValue
      groupDialogSubmit.disabled = true
      const payload = mode === 'edit'
        ? { groupId: String(group.id), title, icon: groupDialogIcon.value.trim() }
        : { title, icon: groupDialogIcon.value.trim(), parentId: null }
      const success = await mutate(`group-${mode}-${group?.id || 'new'}`, groupDialogSubmit, mode === 'edit' ? 'group.update' : 'group.create', payload)
      groupDialogSubmit.disabled = false
      if (success) closeGroupDialog()
      else groupDialogError.textContent = status.textContent || 'Could not save the group.'
    }

    const reorderItem = (group, item, direction, button) => {
      if (!canReorderItems(group) || inFlight.has(`items-reorder-${group.id}`)) return
      const itemIds = (group.itemIds || []).map(String)
      const index = itemIds.indexOf(String(item.id))
      const target = index + direction
      if (index < 0 || target < 0 || target >= itemIds.length) return
      ;[itemIds[index], itemIds[target]] = [itemIds[target], itemIds[index]]
      void mutate(`items-reorder-${group.id}`, button, 'items.reorder', { groupId: String(group.id), itemIds })
    }

    const reorderGroup = (group, direction, button) => {
      if (!canMutateGroup('reorder') || inFlight.has(`groups-reorder-${group.parentId ?? 'root'}`)) return
      const parentId = group.parentId ?? null
      const siblings = (snapshot.groups || []).filter(candidate => (candidate.parentId ?? null) === parentId)
      const groupIds = siblings.map(candidate => String(candidate.id))
      const index = groupIds.indexOf(String(group.id))
      const target = index + direction
      if (index < 0 || target < 0 || target >= groupIds.length) return
      ;[groupIds[index], groupIds[target]] = [groupIds[target], groupIds[index]]
      void mutate(`groups-reorder-${parentId ?? 'root'}`, button, 'groups.reorder', { parentId, groupIds })
    }

    const openItemMenu = (event, item, group) => {
      event.preventDefault()
      event.stopPropagation()
      contextItemTrigger = itemNodes.get(String(item.id))
      contextMenu.replaceChildren()
      // Localized menu labels come from the Core environment; the English text
      // stays as a fallback so a Core that predates a key still renders.
      const labels = api.environment?.get?.()?.labels || {}
      const actions = []
      if (canEditItem(item)) {
        const label = labels['item.edit'] || 'Edit item'
        actions.push(addActionButton(contextMenu, label, 'theme-edit-item', `${label} ${item.title}`, button => {
          contextMenu.hidden = true
          void openEditor(`editor-item-${item.id}`, { itemId: String(item.id) }, button)
        }))
      }
      if (canDeleteItem(item)) {
        const label = labels['item.delete'] || 'Delete item'
        actions.push(addActionButton(contextMenu, label, 'theme-delete-item', `${label} ${item.title}`, button => {
          contextMenu.hidden = true
          void mutate(`item-delete-${item.id}`, button, 'item.delete', { itemId: String(item.id) })
        }))
      }
      if (canReorderItems(group)) {
        const index = (group.itemIds || []).map(String).indexOf(String(item.id))
        if (index > 0) {
          const label = labels['item.moveUp'] || 'Move item up'
          actions.push(addActionButton(contextMenu, label, 'theme-reorder-item-up', `${label} ${item.title}`, button => {
            contextMenu.hidden = true
            reorderItem(group, item, -1, button)
          }))
        }
        if (index >= 0 && index < (group.itemIds || []).length - 1) {
          const label = labels['item.moveDown'] || 'Move item down'
          actions.push(addActionButton(contextMenu, label, 'theme-reorder-item-down', `${label} ${item.title}`, button => {
            contextMenu.hidden = true
            reorderItem(group, item, 1, button)
          }))
        }
      }
      if (!actions.length) return
      contextMenu.hidden = false
      const bounds = page.getBoundingClientRect()
      contextMenu.style.left = `${Math.max(8, Math.min(event.clientX, bounds.right - 200))}px`
      contextMenu.style.top = `${Math.max(8, Math.min(event.clientY, window.innerHeight - 200))}px`
      actions[0].focus()
    }

    let status
    let page
    let logo
    let logoImage
    let logoText
    let logoDivider
    let clock
    let clockTime
    let clockDate
    let searchSection
    let searchContainer
    let searchClearButton
    let searchInput
    let engineMenu
    let engineButton
    let selectedEngineId = ''
    let searchButton
    let directoryNav
    let collection
    let emptyState
    let footer

    const updateClock = () => {
      if (!clock) return
      const presentation = snapshot?.presentation
      if (!presentation?.clock?.visible) {
        clock.hidden = true
        return
      }
      clock.hidden = false
      const now = new Date()
      // Follow the panel language, not the OS locale: passing `undefined` made
      // Intl use the browser locale, which is how the clock kept showing English
      // weekdays (and a hard-coded month-day order) under a Chinese UI.
      const locale = api.environment?.get?.()?.language || undefined
      const formatter = new Intl.DateTimeFormat(locale, {
        hour: '2-digit', minute: '2-digit', second: presentation.clock.showSeconds ? '2-digit' : undefined,
        hourCycle: 'h23',
      })
      clockTime.textContent = formatter.format(now)
      clockDate.textContent = new Intl.DateTimeFormat(locale, { month: 'long', day: 'numeric', weekday: 'long' }).format(now)
      clock.title = new Intl.DateTimeFormat(locale, { dateStyle: 'full' }).format(now)
      if (presentation.clock.color) clock.style.color = presentation.clock.color
      else clock.style.removeProperty('color')
    }

    const canToggleSide = () => (snapshot?.activeSpaceCapabilities || []).includes('space.toggleSide')

    // The frame is sized to this document's content, so `vh` inside it resolves
    // against the content height. Spacing must therefore use the real viewport
    // reported by the Core environment, otherwise the frame height feeds back
    // into its own padding and the layout grows without bound.
    const viewportHeight = () => {
      const reported = Number(api.environment?.get?.()?.viewport?.height)
      return Number.isFinite(reported) && reported > 0 ? reported : window.innerHeight
    }
    // The pre-theme build placed the content with `margin-top: 10%`, and CSS
    // resolves a percentage margin against the containing block's *width*. So
    // the vertical offset tracks the viewport width, not the height.
    const viewportWidth = () => {
      const reported = Number(api.environment?.get?.()?.viewport?.width)
      return Number.isFinite(reported) && reported > 0 ? reported : window.innerWidth
    }

    const togglePanelSide = async () => {
      if (sideSwitching || !canToggleSide()) return
      sideSwitching = true
      updateLogo(snapshot?.presentation)
      try {
        await api.commands.execute('space.toggleSide')
      } catch (error) {
        if (status) status.textContent = error?.message || 'Unable to switch panel'
      } finally {
        sideSwitching = false
        updateLogo(snapshot?.presentation)
      }
    }

    const createShell = elementRoot => {
      root = elementRoot
      root.replaceChildren()
      root.className = 'yin-theme-root'
      page = element('div', 'yin-page')
      // The pre-theme build nested the masthead and the search box inside an
      // 80%-wide header, and the search box was a further 80% of that above
      // the lg breakpoint. Keep the nesting so both widths track the viewport
      // the same way instead of relying on a hard max-width.
      const headerSection = element('header', 'yin-header')
      const masthead = element('header', 'yin-masthead')
      const identity = element('div', 'yin-identity')
      logo = element('div', 'yin-logo')
      logoImage = element('img', 'yin-logo-image')
      logoImage.alt = ''
      logoText = element('span', 'yin-logo-text')
      logo.append(logoImage, logoText)
      logo.dataset.testid = 'theme-side-toggle'
      logo.addEventListener('click', () => { void togglePanelSide() })
      logo.addEventListener('keydown', event => {
        if (event.key !== 'Enter' && event.key !== ' ') return
        event.preventDefault()
        void togglePanelSide()
      })
      logoDivider = element('span', 'yin-logo-divider', '|')
      logoDivider.setAttribute('aria-hidden', 'true')
      clock = element('time', 'yin-clock')
      clock.dataset.testid = 'theme-home-clock'
      clockTime = element('span', 'yin-clock-time')
      clockDate = element('span', 'yin-clock-date')
      clock.append(clockTime, clockDate)
      identity.append(logo, logoDivider, clock)
      masthead.append(identity)
      actionBar = element('div', 'yin-actions')
      actionBar.setAttribute('aria-label', 'Bookmark actions')
      commandCenterButton = addActionButton(actionBar, envLabel('actions.commands', 'Commands'), 'theme-open-command-center', 'Open command center', () => run(() => api.commands.execute('commandCenter.open')))
      addGroupButton = addActionButton(actionBar, envLabel('actions.addGroup', 'Add group'), 'theme-add-group', 'Add group', () => showGroupDialog('create'))
      styleButton = addActionButton(actionBar, envLabel('actions.style', 'Style'), 'theme-open-style', 'Open theme style settings', () => run(() => api.ui.openCoreSurface('theme-settings')))
      pageButton = addActionButton(actionBar, envLabel('actions.page', 'Pages'), 'theme-open-page', 'Open a theme-rendered page', () => run(() => api.ui.openCoreSurface('theme-page')))
      saveButton = addActionButton(actionBar, envLabel('actions.save', 'Save'), 'theme-save-layout', 'Save layout', button => void saveLayout(button))
      cancelButton = addActionButton(actionBar, envLabel('actions.cancel', 'Cancel'), 'theme-cancel-edit', 'Cancel layout editing', () => exitEditMode())

      searchSection = element('section', 'yin-search')
      searchSection.setAttribute('role', 'search')
      searchSection.dataset.testid = 'theme-home-search'
      // The pre-theme build put the rounded, bordered surface on a wrapper and
      // left the input itself transparent inside it. Keeping that split means
      // the border, radius and padding sit where the pixel baselines expect.
      searchContainer = element('div', 'yin-search-container')
      searchInput = element('input', 'yin-search-input')
      // Not type=search: the UA cancel affordance is unstyleable and would
      // sit where the pre-theme build put its own clear slot.
      searchInput.type = 'text'
      searchInput.autocomplete = 'off'
      searchInput.setAttribute('aria-label', 'Search bookmarks')
      searchInput.placeholder = api.environment?.get?.()?.labels?.['search.placeholder'] || 'Enter search content'
      searchInput.addEventListener('input', () => {
        query = searchInput.value
        searchClearButton.hidden = query === ''
        renderCollection()
      })
      searchInput.addEventListener('keydown', event => {
        if (event.key === 'Enter') {
          event.preventDefault()
          submitSearch()
        }
      })
      // The pre-theme engine switcher revealed the engine icons and let the user
      // pick one, instead of a native select. Mirror that: a button showing the
      // active engine plus a popup of engine icons.
      engineButton = element('button', 'yin-search-engine')
      engineButton.type = 'button'
      engineButton.setAttribute('aria-label', 'Search engine')
      engineButton.setAttribute('aria-haspopup', 'listbox')
      engineButton.setAttribute('aria-expanded', 'false')
      engineButton.addEventListener('click', event => {
        event.stopPropagation()
        setEngineMenuOpen(engineMenu.hidden)
      })
      engineMenu = element('div', 'yin-search-engines')
      engineMenu.setAttribute('role', 'listbox')
      engineMenu.hidden = true
      engineMenu.addEventListener('click', event => event.stopPropagation())
      // The pre-theme build showed a 25px clear slot with a 10px right margin
      // only while a query was present, which is what squeezed the input.
      searchClearButton = element('button', 'yin-search-clear')
      searchClearButton.type = 'button'
      searchClearButton.setAttribute('aria-label', 'Clear search')
      searchClearButton.hidden = true
      searchClearButton.addEventListener('click', () => {
        searchInput.value = ''
        query = ''
        searchClearButton.hidden = true
        renderCollection()
        searchInput.focus()
      })
      searchButton = element('button', 'yin-search-submit', 'Search')
      searchButton.type = 'button'
      searchButton.setAttribute('aria-label', 'Search')
      searchButton.addEventListener('click', submitSearch)
      searchSection.append(searchContainer, engineMenu)
      searchContainer.append(engineButton, searchInput, searchClearButton, searchButton)
      directoryNav = element('nav', 'directory-folders')
      directoryNav.setAttribute('aria-label', 'Bookmark groups')
      status = element('p', 'yin-status')
      status.setAttribute('role', 'status')
      status.dataset.testid = 'theme-home-status'
      collection = element('main', 'yin-collection')
      collection.addEventListener('pointerdown', handleCollectionPointerDown)
      emptyState = element('p', 'yin-empty')
      footer = element('footer', 'yin-footer')
      groupDialog = element('div', 'yin-dialog-backdrop')
      groupDialog.hidden = true
      groupDialog.dataset.testid = 'theme-group-dialog'
      groupDialog.addEventListener('pointerdown', event => {
        if (event.target === groupDialog) closeGroupDialog()
      })
      const dialogPanel = element('section', 'yin-dialog')
      dialogPanel.setAttribute('role', 'dialog')
      dialogPanel.setAttribute('aria-modal', 'true')
      dialogPanel.setAttribute('aria-labelledby', 'yin-group-dialog-title')
      const dialogForm = element('div', 'yin-dialog-form')
      groupDialogHeading = element('h2', 'yin-dialog-title')
      groupDialogHeading.id = 'yin-group-dialog-title'
      const titleLabel = element('label', 'yin-dialog-label', envLabel('dialog.groupName', 'Group name'))
      groupDialogTitle = element('input', 'yin-dialog-input')
      groupDialogTitle.name = 'group-title'
      groupDialogTitle.dataset.testid = 'theme-group-title-input'
      groupDialogTitle.maxLength = 50
      groupDialogTitle.required = true
      titleLabel.append(groupDialogTitle)
      const iconLabel = element('label', 'yin-dialog-label', envLabel('dialog.groupIcon', 'Icon identifier (optional)'))
      groupDialogIcon = element('input', 'yin-dialog-input')
      groupDialogIcon.name = 'group-icon'
      groupDialogIcon.dataset.testid = 'theme-group-icon-input'
      groupDialogIcon.maxLength = 240
      iconLabel.append(groupDialogIcon)
      groupDialogError = element('p', 'yin-dialog-error')
      groupDialogError.setAttribute('role', 'alert')
      const dialogActions = element('div', 'yin-dialog-actions')
      const cancelGroupButton = element('button', 'yin-action-button', envLabel('dialog.cancel', 'Cancel'))
      cancelGroupButton.type = 'button'
      cancelGroupButton.addEventListener('click', closeGroupDialog)
      groupDialogSubmit = element('button', 'yin-action-button yin-action-button--primary')
      groupDialogSubmit.type = 'button'
      groupDialogSubmit.dataset.testid = 'theme-group-submit'
      groupDialogSubmit.addEventListener('click', () => { void submitGroupDialog() })
      dialogActions.append(cancelGroupButton, groupDialogSubmit)
      dialogForm.append(groupDialogHeading, titleLabel, iconLabel, groupDialogError, dialogActions)
      dialogForm.addEventListener('keydown', event => {
        if (event.key === 'Enter' && event.target !== groupDialogSubmit) {
          event.preventDefault()
          void submitGroupDialog()
        }
      })
      dialogPanel.append(dialogForm)
      groupDialog.append(dialogPanel)
      contextMenu = element('div', 'yin-context-menu')
      contextMenu.hidden = true
      contextMenu.setAttribute('role', 'menu')
      contextMenu.setAttribute('aria-label', 'Item actions')
      headerSection.append(masthead, actionBar, searchSection)
      page.append(headerSection, directoryNav, status, collection, emptyState, footer, groupDialog, contextMenu)
      page.addEventListener('keydown', event => {
        if (!groupDialog.hidden && event.key === 'Tab') {
          const focusable = [...groupDialog.querySelectorAll('input:not(:disabled), button:not(:disabled)')]
          const first = focusable[0]
          const last = focusable[focusable.length - 1]
          if (event.shiftKey && document.activeElement === first) {
            event.preventDefault()
            last?.focus()
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault()
            first?.focus()
          }
          return
        }
        if (event.key === 'Escape') {
          if (!groupDialog.hidden) closeGroupDialog()
          else if (!contextMenu.hidden) {
            contextMenu.hidden = true
            if (contextItemTrigger?.isConnected) contextItemTrigger.focus()
          }
        }
      })
      // The home lives in a cross-origin frame, so a key pressed here never
      // reaches the Core's window listener and the global shortcut (type a letter
      // to open the command centre) would be dead. Forwarding keeps the policy in
      // the Core: it decides whether a dialog owns the keyboard, and this only
      // passes on unmodified single characters typed outside a field.
      //
      // This listens on the document rather than on `page`, because a key pressed
      // with nothing focused targets <body>, which is an *ancestor* of the page
      // element — the event bubbles up from body and never reaches it.
      document.addEventListener('keydown', event => {
        const target = event.target
        const editable = target instanceof Element && !!target.closest('input, textarea, select, [contenteditable="true"]')
        if (event.isComposing || editable || event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) return
        if (event.key.length !== 1 || !groupDialog.hidden) return
        run(() => api.commands.execute('input.forwardKey', { key: event.key }))
      })
      // Panel-supplied links (the "Powered by" footer) cannot open a window from
      // inside the sandbox, so the click is handed to the Core, which applies the
      // same open policy as a bookmark. Relative links, anchors and mailto: stay
      // in the frame, where they work without any privilege.
      footer.addEventListener('click', event => {
        const anchor = event.target instanceof Element ? event.target.closest('a[data-link-url]') : null
        if (!anchor) return
        event.preventDefault()
        run(() => api.commands.execute('link.open', { url: anchor.dataset.linkUrl }))
      })
      page.addEventListener('pointerdown', event => {
        if (!contextMenu.hidden && !contextMenu.contains(event.target)) contextMenu.hidden = true
      })
      root.append(page)
      clockTimer = window.setInterval(updateClock, 1000)
    }

    const submitSearch = () => {
      const presentation = snapshot?.presentation
      if (!presentation?.search?.visible) return
      const engineId = selectedEngineId
      const engines = presentation.search.engines || []
      setEngineMenuOpen(false)
      if (engineId && engines.some(engine => engine.id === engineId)) {
        return run(() => api.commands.execute('search.submit', { action: 'engine', engineId, query }))
      }
      return run(() => api.commands.execute('search.submit', { action: query ? 'filter' : 'clear', query }))
    }

    const updateLogo = presentation => {
      logoText.textContent = presentation?.logoText || (snapshot?.activeSpaceSide === 'yang' ? 'Yang-Panel' : 'Yin-Panel')
      const src = presentation?.logoImageSrc || ''
      logoImage.hidden = !src
      logoText.hidden = !!src
      if (src && logoImage.src !== src) logoImage.src = src
      if (canToggleSide()) {
        // The brand mark itself is the side switch, matching the Core home.
        logo.setAttribute('role', 'button')
        logo.setAttribute('aria-label', `Switch to ${snapshot?.activeSpaceSide === 'yang' ? 'Yin' : 'Yang'}-Panel`)
        logo.setAttribute('aria-disabled', sideSwitching ? 'true' : 'false')
        logo.tabIndex = sideSwitching ? -1 : 0
      } else {
        logo.removeAttribute('role')
        logo.removeAttribute('aria-disabled')
        logo.setAttribute('aria-label', logoText.textContent)
        logo.tabIndex = -1
      }
    }

    // The engine picker must reflect the selection immediately. Waiting for the
    // Core round trip leaves the previous icon on screen, and the submission
    // opens a new tab that takes focus away before any snapshot arrives.
    const setEngineMenuOpen = open => {
      engineMenu.hidden = !open
      engineButton.setAttribute('aria-expanded', open ? 'true' : 'false')
    }

    const updateEngineIndicator = () => {
      const engines = snapshot?.presentation?.search?.engines || []
      const currentEngine = engines.find(engine => engine.id === selectedEngineId)
      engineButton.style.backgroundImage = currentEngine?.iconSrc
        ? `url("${currentEngine.iconSrc.replace(/["\\\n\r]/g, '')}")`
        : 'none'
      engineButton.setAttribute('aria-label', currentEngine ? `Search engine: ${currentEngine.title}` : 'Search engine')
      searchButton.hidden = !selectedEngineId
    }

    const updateSearch = presentation => {
      const search = presentation?.search
      searchSection.hidden = !search?.visible
      searchInput.hidden = !search?.itemFilterEnabled
      if (searchInput.value !== query) searchInput.value = query
      searchInput.disabled = snapshot.status === 'loading'
      const engines = search?.engines || []
      if (!engines.some(engine => engine.id === selectedEngineId))
        selectedEngineId = search?.currentEngineId || engines[0]?.id || ''
      engineMenu.replaceChildren()
      for (const engine of engines) {
        const option = element('button', 'yin-search-engine-option')
        option.type = 'button'
        option.setAttribute('role', 'option')
        option.setAttribute('aria-selected', String(engine.id === selectedEngineId))
        option.setAttribute('aria-label', engine.title)
        option.title = engine.title
        if (engine.iconSrc)
          option.style.backgroundImage = `url("${engine.iconSrc.replace(/["\\\n\r]/g, '')}")`
        else
          option.textContent = engine.title.slice(0, 1)
        option.addEventListener('click', () => {
          // Temporary switch only. Picking an engine must not launch a search on
          // its own; the next Enter or the submit button uses the new engine.
          selectedEngineId = engine.id
          setEngineMenuOpen(false)
          updateEngineIndicator()
        })
        engineMenu.append(option)
      }
      engineButton.hidden = !engines.length
      engineMenu.hidden = true
      engineButton.setAttribute('aria-expanded', 'false')
      updateEngineIndicator()
      searchButton.disabled = snapshot.status === 'loading'
    }

    const createGroupNode = group => {
      const section = element('section', 'yin-group')
      section.dataset.groupId = String(group.id)
      const heading = element('div', 'yin-group-heading')
      const title = element('h2', 'yin-group-title')
      const controls = element('div', 'yin-group-controls')
      const addItem = addActionButton(controls, envLabel('group.addItem', 'Add item'), 'theme-add-item', 'Add item', button => {
        const groupId = section.dataset.groupId
        void openEditor(`editor-new-item-${groupId}`, { groupId }, button)
      })
      const editGroup = addActionButton(controls, envLabel('group.edit', 'Edit group'), 'theme-edit-group', 'Edit group', () => {
        const current = (snapshot?.groups || []).find(candidate => String(candidate.id) === section.dataset.groupId)
        if (current) showGroupDialog('edit', current)
      })
      const deleteGroup = addActionButton(controls, envLabel('group.delete', 'Delete group'), 'theme-delete-group', 'Delete group', button => {
        void mutate(`group-delete-${section.dataset.groupId}`, button, 'group.delete', { groupId: section.dataset.groupId })
      })
      const moveGroupUp = addActionButton(controls, envLabel('group.moveUp', 'Move group up'), 'theme-reorder-group-up', 'Move group up', button => {
        const current = (snapshot?.groups || []).find(candidate => String(candidate.id) === section.dataset.groupId)
        if (current) reorderGroup(current, -1, button)
      })
      const moveGroupDown = addActionButton(controls, envLabel('group.moveDown', 'Move group down'), 'theme-reorder-group-down', 'Move group down', button => {
        const current = (snapshot?.groups || []).find(candidate => String(candidate.id) === section.dataset.groupId)
        if (current) reorderGroup(current, 1, button)
      })
      // The chevron is drawn in CSS from aria-expanded, so no glyph text.
      const toggle = element('button', 'yin-group-toggle')
      toggle.type = 'button'
      toggle.setAttribute('aria-label', 'Collapse group')
      toggle.setAttribute('aria-expanded', 'true')
      toggle.addEventListener('click', () => {
        const groupId = section.dataset.groupId
        if (collapsedGroups.has(groupId)) collapsedGroups.delete(groupId)
        else collapsedGroups.add(groupId)
        renderCollection()
      })
      heading.append(title, controls, toggle)
      const list = element('div', 'yin-group-items')
      section.append(heading, list)
      section._yin = { heading, title, toggle, list, addItem, editGroup, deleteGroup, moveGroupUp, moveGroupDown }
      return section
    }

    const createItemNode = item => {
      // 「新窗口」用真正的锚点，而不是 button + window.open。
      //
      // 锚点是浏览器原生导航，不经过主题的 JS 事件处理器：即使点击处理器因为任何原因
      // 没有执行，`target="_blank"` 也会照常开出新标签。这是「浏览器自己的打开方式能用、
      // 脚本打开不行」那类环境下唯一可靠的路径。
      //
      // 其余打开方式（当前页、面板内窗口）需要 Core 参与，所以由 JS 接管并
      // `preventDefault`，不让锚点的默认导航生效。
      const link = element('a', 'yin-item')
      link.rel = 'noopener noreferrer'
      link.draggable = false
      const icon = element('span', 'yin-item-icon')
      const copy = element('span', 'yin-item-copy')
      const title = element('strong', 'yin-item-title')
      const description = element('span', 'yin-item-description')
      copy.append(title, description)
      link.append(icon, copy)
      link.addEventListener('click', event => {
        if (editMode) { event.preventDefault(); return }
        const item = link._yin.item
        const method = Number(item && item.openMethod) || 1
        const isNewWindow = method === 2
        diagClick(isNewWindow ? 'item(新窗口)' : 'item(其他方式)', [
          `openMethod: ${item && item.openMethod}`,
          `url: ${item && item.url ? item.url : '(空!)'}`,
          `href: ${link.getAttribute('href')}`,
          `target: ${link.getAttribute('target')}`,
          `event.isTrusted: ${event.isTrusted}`,
          `event.type: ${event.type}`,
        ])
        if (isNewWindow && item.url) {
          // 交给原生 `target="_blank"`。这里不 `preventDefault`、也不再 `window.open`，
          // 否则会开两次；而且脚本开窗正是那条在部分环境下不成立的路径。
          diag(['新窗口交给原生 target=_blank，本处理器不再开窗'])
          return
        }
        event.preventDefault()
        run(() => api.commands.execute('item.open', { itemId: link.dataset.itemId }))
      })
      link._yin = { icon, title, description }
      link.addEventListener('contextmenu', event => {
        if (editMode) return
        const item = link._yin.item
        const group = (snapshot?.groups || []).find(candidate => candidate.itemIds?.map(String).includes(link.dataset.itemId))
        if (item && group) openItemMenu(event, item, group)
      })
      link.addEventListener('keydown', event => {
        if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
        const item = link._yin.item
        const group = (snapshot?.groups || []).find(candidate => candidate.itemIds?.map(String).includes(link.dataset.itemId))
        if (!item || !group) return
        openItemMenu({
          preventDefault: () => event.preventDefault(),
          stopPropagation: () => event.stopPropagation(),
          clientX: link.getBoundingClientRect().left,
          clientY: link.getBoundingClientRect().bottom,
        }, item, group)
      })
      return link
    }

    const updateItemNode = (link, item, presentation, layout) => {
      const { icon, title, description } = link._yin
      link.dataset.itemId = String(item.id)
      link.dataset.testid = 'theme-home-item'
      link.setAttribute('aria-label', `Open ${item.title}`)
      // 锚点的导航目标。新窗口靠原生 `target="_blank"`；其余方式由 JS 接管并
      // preventDefault，`href` 只是让锚点保持可聚焦、可右键、可中键。
      const isNewWindow = Number(item.openMethod) === 2
      link.href = isNewWindow && item.url ? item.url : '#'
      link.target = isNewWindow ? '_blank' : '_self'
      title.textContent = item.title || ''
      description.textContent = item.description || ''
      link._yin.item = item
      title.hidden = layout === 'directory' ? false : presentation?.iconTextIconHideTitle === true
      description.hidden = layout === 'directory'
        ? true
        : presentation?.iconStyle === 'info'
          ? presentation.iconTextInfoHideDescription === true
          : true
      icon.replaceChildren()
      icon.removeAttribute('style')
      const itemIcon = item.icon
      if (itemIcon?.backgroundColor) icon.style.backgroundColor = itemIcon.backgroundColor
      if (itemIcon?.src) {
        const image = element('img', 'yin-item-icon-image')
        image.src = itemIcon.src
        image.alt = ''
        image.loading = 'lazy'
        image.referrerPolicy = 'no-referrer'
        icon.append(image)
      } else {
        // The letter sits on its own 70x70 layer so it centres on the full tile
        // box, not on the 1px-inset content area left by the transparent border.
        const letters = element('span', 'yin-item-icon-letters')
        letters.textContent = itemIcon?.text || itemIcon?.fileName || (item.title || '?').slice(0, 1)
        icon.append(letters)
      }
      const infoStyle = layout === 'standard' && presentation?.iconStyle === 'info'
      link.classList.toggle('yin-item--info', infoStyle)
      // The detailed style paints the whole card with the item colour and keeps
      // the glyph on a transparent tile, matching the pre-theme build.
      if (infoStyle) {
        if (itemIcon?.backgroundColor) link.style.backgroundColor = itemIcon.backgroundColor
        else link.style.removeProperty('background-color')
        icon.style.removeProperty('background-color')
      } else {
        link.style.removeProperty('background-color')
        if (itemIcon?.backgroundColor) icon.style.backgroundColor = itemIcon.backgroundColor
      }
    }

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

    const updateDirectoryNav = roots => {
      const kept = new Set()
      for (const group of roots) {
        const key = String(group.id)
        kept.add(key)
        let button = directoryNodes.get(key)
        if (!button) {
          button = element('button', 'directory-folder-button')
          button.type = 'button'
          button.addEventListener('click', () => {
            directoryRootId = button.dataset.groupId
            renderCollection()
          })
          directoryNodes.set(key, button)
        }
        button.dataset.groupId = key
        button.textContent = group.title || 'Untitled group'
        button.classList.toggle('active', key === directoryRootId)
        button.setAttribute('aria-pressed', key === directoryRootId ? 'true' : 'false')
        directoryNav.append(button)
      }
      for (const [key, button] of directoryNodes) {
        if (!kept.has(key)) {
          button.remove()
          directoryNodes.delete(key)
        }
      }
      directoryNav.hidden = snapshot.presentation?.layout !== 'directory' || roots.length === 0
    }

    const updateFooter = html => {
      footer.replaceChildren()
      if (!html) {
        footer.hidden = true
        return
      }
      footer.hidden = false
      const parsed = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html').body
      const allowed = new Set(['A', 'B', 'BR', 'EM', 'I', 'P', 'SMALL', 'SPAN', 'STRONG'])
      // Layout wrappers carry no formatting of their own and their classes are
      // not carried over either, so dropping them would silently discard the
      // whole block. The default footer markup is one such wrapper, so unwrap
      // instead of reject and keep the text plus any allowed inline children.
      const transparent = new Set(['ASIDE', 'CENTER', 'DIV', 'FIGCAPTION', 'FIGURE', 'FOOTER', 'HEADER', 'MAIN', 'NAV', 'SECTION'])
      const SAFE_STYLE_PROPERTIES = new Set(['color', 'font-size', 'font-weight', 'letter-spacing', 'margin', 'margin-bottom', 'margin-left', 'margin-right', 'margin-top', 'text-align', 'text-transform'])
      const appendSafe = (source, target) => {
        for (const child of source.childNodes) {
          if (child.nodeType === Node.TEXT_NODE) {
            target.append(document.createTextNode(child.textContent || ''))
            continue
          }
          if (child.nodeType !== Node.ELEMENT_NODE) continue
          if (transparent.has(child.tagName)) {
            appendSafe(child, target)
            continue
          }
          if (!allowed.has(child.tagName)) continue
          const safe = document.createElement(child.tagName.toLowerCase())
          // Classes cannot be carried over because the theme ships no utility
          // CSS, so honour a small set of presentational inline declarations to
          // keep hand-written footer markup close to what the user authored.
          const style = child.getAttribute('style')
          if (style) {
            const kept = style
              .split(';')
              .filter(decl => {
                const prop = decl.split(':')[0]
                return prop ? SAFE_STYLE_PROPERTIES.has(prop.trim().toLowerCase()) : false
              })
              .map(decl => `${decl.trim()};`)
            if (kept.length) safe.setAttribute('style', kept.join(' '))
          }
          if (child.tagName === 'A') {
            const href = child.getAttribute('href') || ''
            if (/^(https?:|mailto:|\/|#)/i.test(href)) {
              safe.setAttribute('href', href)
              if (/^https?:/i.test(href)) {
                // No `target`: the sandbox has no `allow-popups`, so the browser
                // would refuse to open a new window and log a violation. Clicks
                // are routed to the Core instead (see the footer click handler),
                // which also keeps the link on the Core's own open policy.
                safe.dataset.linkUrl = href
              }
            }
          }
          appendSafe(child, safe)
          target.append(safe)
        }
      }
      appendSafe(parsed, footer)
    }

    const positionCollectionAfterMonitor = () => {      if (collectionLayoutFrame) cancelAnimationFrame(collectionLayoutFrame)
      collection.style.removeProperty('margin-top')
      if (directoryNav) directoryNav.style.removeProperty('margin-top')
      collectionLayoutFrame = requestAnimationFrame(() => {
        collectionLayoutFrame = undefined
        const reservedHeight = Number(snapshot?.presentation?.monitor?.reservedHeight)
        if (!Number.isFinite(reservedHeight) || reservedHeight <= 0) return
        const isDirectory = snapshot?.presentation?.layout === 'directory'
        const naturalMargin = isDirectory ? 18 : 24
        // Unified with the desktop spacing. The monitor band already reserves its
        // own height, so the old mobile-only viewport-proportional extra only
        // pushed the first group needlessly far below it.
        const standardGap = 77
        const desiredTop = reservedHeight + standardGap
        // The directory folder picker sits between the search box and the
        // collection, so it is the element that lands inside the monitor band.
        // Push the picker below the band and keep the collection at its natural
        // spacing, otherwise the picker is covered by the monitor.
        if (isDirectory && directoryNav && !directoryNav.hidden) {
          const navTop = directoryNav.getBoundingClientRect().top
          directoryNav.style.marginTop = `${Math.max(0, desiredTop - navTop)}px`
          collection.style.marginTop = `${naturalMargin}px`
        } else {
          const currentTop = collection.getBoundingClientRect().top
          collection.style.marginTop = `${naturalMargin + Math.max(0, desiredTop - currentTop)}px`
        }
      })
    }

    // The Core cannot measure inside the sandboxed frame, so report where the
    // search box ended up and let it place the monitor layer from real
    // geometry. The collection offset below never moves the search box, so this
    // cannot feed back into another layout pass.
    const reportLayout = () => {
      if (!searchSection || searchSection.hidden) return
      const bottom = Math.round(searchSection.getBoundingClientRect().bottom)
      if (!Number.isFinite(bottom) || bottom <= 0 || bottom === reportedSearchBottom) return
      reportedSearchBottom = bottom
      void Promise.resolve(api.commands.execute('layout.report', { searchBottom: bottom })).catch(() => {})
    }

    const renderCollection = () => {
      if (!snapshot || !collection) return
      // Frozen while editing: the drag mutates the DOM directly and the layout is
      // serialized from it, so a snapshot re-render would discard the drag.
      if (editMode) return
      const presentation = snapshot.presentation || {}
      const tree = groupTree(snapshot.groups)
      const isDirectory = presentation.layout === 'directory'
      updateDirectoryNav(tree.roots)
      if (!directoryRootId || !tree.roots.some(group => String(group.id) === directoryRootId)) {
        directoryRootId = tree.roots.length ? String(tree.roots[0].id) : undefined
        updateDirectoryNav(tree.roots)
      }
      const visibleGroups = []
      const addGroupTree = (group, depth) => {
        visibleGroups.push({ group, depth })
        for (const child of tree.children.get(String(group.id)) || []) addGroupTree(child, depth + 1)
      }
      if (isDirectory) {
        const selectedRoot = tree.roots.find(group => String(group.id) === directoryRootId)
        if (selectedRoot) addGroupTree(selectedRoot, 0)
      } else {
        for (const group of snapshot.groups || []) visibleGroups.push({ group, depth: 0 })
      }
      const itemById = new Map((snapshot.items || []).map(item => [String(item.id), item]))
      const needle = query.trim().toLocaleLowerCase()
      const usedGroups = new Set()
      for (const { group, depth } of visibleGroups) {
        const section = groupNodes.get(String(group.id)) || createGroupNode(group)
        groupNodes.set(String(group.id), section)
        section.dataset.groupId = String(group.id)
        section.dataset.parentId = group.parentId == null ? '' : String(group.parentId)
        section._yin.title.textContent = group.title || 'Untitled group'
        const collapsed = collapsedGroups.has(String(group.id))
        section._yin.toggle.setAttribute('aria-label', collapsed ? 'Expand group' : 'Collapse group')
        section._yin.toggle.setAttribute('aria-expanded', collapsed ? 'false' : 'true')
        section.classList.toggle('is-collapsed', collapsed)
        section.style.setProperty('--yin-group-depth', String(depth))
        // In the directory layout the selected root is already named by the
        // folder picker above, so its own heading row is redundant. Keep the
        // items, drop the heading.
        section._yin.heading.hidden = isDirectory && depth === 0
        const siblings = (snapshot.groups || []).filter(candidate => (candidate.parentId ?? null) === (group.parentId ?? null))
        const siblingIndex = siblings.findIndex(candidate => String(candidate.id) === String(group.id))
        section._yin.addItem.hidden = !canCreateItems()
        section._yin.addItem.disabled = inFlight.has(`editor-new-item-${group.id}`)
        section._yin.editGroup.hidden = !canMutateGroup('update')
        section._yin.deleteGroup.hidden = !canMutateGroup('delete')
        section._yin.deleteGroup.disabled = inFlight.has(`group-delete-${group.id}`)
        section._yin.moveGroupUp.hidden = !canMutateGroup('reorder') || siblingIndex <= 0
        section._yin.moveGroupDown.hidden = !canMutateGroup('reorder') || siblingIndex < 0 || siblingIndex >= siblings.length - 1
        section._yin.moveGroupUp.disabled = inFlight.has(`groups-reorder-${group.parentId ?? 'root'}`)
        section._yin.moveGroupDown.disabled = section._yin.moveGroupUp.disabled
        const list = section._yin.list
        const visibleItemIds = new Set()
        if (!collapsed) {
          for (const itemId of group.itemIds || []) {
            const item = itemById.get(String(itemId))
            if (!item) continue
            const searchable = `${item.title || ''} ${item.description || ''}`.toLocaleLowerCase()
            if (needle && presentation.search?.itemFilterEnabled !== false && !searchable.includes(needle)) continue
            let node = itemNodes.get(String(item.id))
            if (!node) {
              node = createItemNode(item)
              itemNodes.set(String(item.id), node)
            }
            updateItemNode(node, item, presentation, isDirectory ? 'directory' : 'standard')
            list.append(node)
            visibleItemIds.add(String(item.id))
          }
          for (const child of [...list.children]) {
            if (!visibleItemIds.has(child.dataset.itemId)) child.remove()
          }
        }
        const hasVisibleItem = (group.itemIds || []).some(itemId => {
          const item = itemById.get(String(itemId))
          return item && (!needle || presentation.search?.itemFilterEnabled === false
            || `${item.title || ''} ${item.description || ''}`.toLocaleLowerCase().includes(needle))
        })
        section.hidden = !!needle && presentation.search?.itemFilterEnabled !== false && !hasVisibleItem
        section.classList.toggle('is-directory', isDirectory)
        if (isDirectory) {
          section._yin.title.classList.add('directory-group-heading')
          section._yin.title.style.marginInlineStart = `${Math.min(8, depth) * 18}px`
        } else {
          section._yin.title.classList.remove('directory-group-heading')
          section._yin.title.style.removeProperty('margin-inline-start')
        }
        collection.append(section)
        usedGroups.add(String(group.id))
      }
      for (const [key, section] of groupNodes) {
        if (!usedGroups.has(key)) section.remove()
      }
      emptyState.hidden = snapshot.status === 'loading' || visibleGroups.some(({ group }) => {
        const section = groupNodes.get(String(group.id))
        return section && !section.hidden && section._yin.list.children.length > 0
      })
      emptyState.textContent = snapshot.status === 'loading'
        ? envLabel('collection.loading', 'Loading items…')
        : snapshot.status === 'error'
          ? snapshot.error?.message || envLabel('collection.loadFailed', 'Could not load items')
          : needle
            ? envLabel('collection.noMatch', 'No matching items')
            : envLabel('collection.empty', 'No items in this space')
      if (snapshot.status === 'loading') collection.setAttribute('aria-busy', 'true')
      else collection.removeAttribute('aria-busy')
      status.textContent = ''
      updateEditControls()
    }

    // ---- Drag edit mode ------------------------------------------------

    const canArrange = () => hasGlobalCapability('items.write')
      || (snapshot?.groups || []).some(group => (group.capabilities || []).includes('items.reorder'))

    const updateEditControls = () => {
      if (!saveButton) return
      saveButton.hidden = !editMode
      cancelButton.hidden = !editMode
      if (editMode) {
        if (commandCenterButton) commandCenterButton.hidden = true
        if (addGroupButton) addGroupButton.hidden = true
        if (styleButton) styleButton.hidden = true
        if (pageButton) pageButton.hidden = true
      }
    }

    const serializeLayout = () => {
      const itemOrder = new Map()
      const itemGroup = new Map()
      const itemIndex = new Map()
      const order = []
      for (const section of collection.querySelectorAll('.yin-group')) {
        const groupId = section.dataset.groupId
        const ids = [...section._yin.list.children]
          .filter(node => node.classList && node.classList.contains('yin-item'))
          .map(node => node.dataset.itemId)
        itemOrder.set(groupId, ids)
        ids.forEach((id, index) => { itemGroup.set(id, groupId); itemIndex.set(id, index) })
        order.push({ groupId, parentId: section.dataset.parentId || '' })
      }
      const groupIndex = new Map()
      const counters = new Map()
      for (const entry of order) {
        const index = counters.get(entry.parentId) || 0
        groupIndex.set(entry.groupId, index)
        counters.set(entry.parentId, index + 1)
      }
      return { itemOrder, itemGroup, itemIndex, groupIndex }
    }

    // Only changed rows are sent, so a drag that moved one icon does not rewrite
    // the whole space. A moved item shifts its neighbours' indices, which the
    // index comparison picks up automatically.
    const diffLayout = (baseline, current) => {
      const items = []
      for (const [groupId, ids] of current.itemOrder) {
        ids.forEach((id, index) => {
          const moved = baseline.itemGroup.get(id) !== groupId
          const reordered = baseline.itemIndex.get(id) !== index
          if (moved || reordered)
            items.push({ id: Number(id), groupId: Number(groupId), sort: index + 1 })
        })
      }
      const groups = []
      for (const [groupId, index] of current.groupIndex) {
        if (baseline.groupIndex.get(groupId) !== index)
          groups.push({ id: Number(groupId), sort: index + 1 })
      }
      return { items, groups }
    }

    const enterEditMode = () => {
      if (editMode || !canArrange()) return
      if (snapshot?.presentation?.layout === 'directory') return
      // A save serializes the whole collection, so a search filter or a collapsed
      // group would silently drop items from the layout.
      query = ''
      collapsedGroups.clear()
      renderCollection()
      editMode = true
      editBaseline = serializeLayout()
      root.dataset.editing = 'true'
      updateEditControls()
    }

    const exitEditMode = () => {
      if (!editMode) return
      editMode = false
      editBaseline = null
      delete root.dataset.editing
      renderCollection()
    }

    const saveLayout = async button => {
      if (!editMode || !editBaseline) return
      const diff = diffLayout(editBaseline, serializeLayout())
      if (!diff.items.length && !diff.groups.length) { exitEditMode(); return }
      button.disabled = true
      try {
        await api.commands.execute('layout.save', diff)
        editMode = false
        editBaseline = null
        delete root.dataset.editing
        if (status) status.textContent = envLabel('status.layoutSaved', 'Layout saved')
        renderCollection()
      }
      catch (error) {
        if (status) status.textContent = `${envLabel('status.layoutSaveFailed', 'Could not save the layout')}${error?.message ? `: ${error.message}` : ''}`
      }
      finally {
        button.disabled = false
      }
    }

    const moveGhost = (x, y) => {
      if (!dragState) return
      dragState.ghost.style.left = `${x - dragState.offsetX}px`
      dragState.ghost.style.top = `${y - dragState.offsetY}px`
      dragState.lastX = x
      dragState.lastY = y
    }

    const groupListAtPoint = (x, y) => {
      let best = null
      let bestDistance = Infinity
      for (const section of collection.querySelectorAll('.yin-group')) {
        const list = section._yin && section._yin.list
        if (!list || !list.offsetParent) continue
        const rect = list.getBoundingClientRect()
        if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) return list
        const dx = x < rect.left ? rect.left - x : x > rect.right ? x - rect.right : 0
        const dy = y < rect.top ? rect.top - y : y > rect.bottom ? y - rect.bottom : 0
        const distance = dx + dy
        if (distance < bestDistance) { bestDistance = distance; best = list }
      }
      return best
    }

    const updateItemDrop = (x, y) => {
      const list = groupListAtPoint(x, y)
      if (!list) return
      const tiles = [...list.children].filter(node => node.classList.contains('yin-item') && node !== dragState.element)
      let before = null
      for (const tile of tiles) {
        const rect = tile.getBoundingClientRect()
        const cx = rect.left + rect.width / 2
        const cy = rect.top + rect.height / 2
        if (y < cy - rect.height / 2 || (y <= cy + rect.height / 2 && x < cx)) { before = tile; break }
      }
      if (before) list.insertBefore(dragState.element, before)
      else list.append(dragState.element)
    }

    const updateGroupDrop = y => {
      const section = dragState.element
      const parentId = section.dataset.parentId || ''
      const siblings = [...collection.querySelectorAll('.yin-group')]
        .filter(node => (node.dataset.parentId || '') === parentId && node !== section)
      let before = null
      for (const node of siblings) {
        const rect = node.getBoundingClientRect()
        if (y < rect.top + rect.height / 2) { before = node; break }
      }
      if (before) collection.insertBefore(section, before)
      else if (siblings.length) siblings[siblings.length - 1].after(section)
      else collection.append(section)
    }

    const handleDragMove = event => {
      if (!dragState) return
      event.preventDefault()
      moveGhost(event.clientX, event.clientY)
      if (dragState.kind === 'item') updateItemDrop(event.clientX, event.clientY)
      else updateGroupDrop(event.clientY)
    }

    const handleDragEnd = () => {
      if (!dragState) return
      dragState.ghost.remove()
      dragState.element.classList.remove('is-dragging')
      document.removeEventListener('pointermove', handleDragMove)
      document.removeEventListener('pointerup', handleDragEnd)
      document.removeEventListener('pointercancel', handleDragEnd)
      if (autoScrollFrame) cancelAnimationFrame(autoScrollFrame)
      autoScrollFrame = undefined
      dragState = null
    }

    const autoScrollStep = () => {
      if (!dragState) return
      const edge = 64
      const { lastX, lastY } = dragState
      if (lastY < edge) window.scrollBy(0, -Math.ceil((edge - lastY) / 3))
      else if (lastY > window.innerHeight - edge) window.scrollBy(0, Math.ceil((lastY - (window.innerHeight - edge)) / 3))
      else if (lastX < edge) window.scrollBy(-Math.ceil((edge - lastX) / 3), 0)
      else if (lastX > window.innerWidth - edge) window.scrollBy(Math.ceil((lastX - (window.innerWidth - edge)) / 3), 0)
      autoScrollFrame = requestAnimationFrame(autoScrollStep)
    }

    const beginDrag = (event, target, kind) => {
      if (dragState || !editMode) return
      event.preventDefault()
      const rect = target.getBoundingClientRect()
      const ghost = target.cloneNode(true)
      ghost.classList.add('yin-drag-ghost')
      ghost.style.width = `${rect.width}px`
      ghost.style.height = `${rect.height}px`
      page.append(ghost)
      target.classList.add('is-dragging')
      dragState = { kind, element: target, ghost, offsetX: event.clientX - rect.left, offsetY: event.clientY - rect.top, lastX: event.clientX, lastY: event.clientY }
      moveGhost(event.clientX, event.clientY)
      document.addEventListener('pointermove', handleDragMove, { passive: false })
      document.addEventListener('pointerup', handleDragEnd)
      document.addEventListener('pointercancel', handleDragEnd)
      autoScrollFrame = requestAnimationFrame(autoScrollStep)
    }

    const handleCollectionPointerDown = event => {
      if (!editMode || event.button !== 0) return
      const target = event.target
      if (target.closest && target.closest('button')) return
      const tile = target.closest && target.closest('.yin-item')
      if (tile && collection.contains(tile)) { beginDrag(event, tile, 'item'); return }
      const heading = target.closest && target.closest('.yin-group-heading')
      if (heading && canMutateGroup('reorder')) {
        const section = heading.closest('.yin-group')
        if (section) beginDrag(event, section, 'group')
      }
    }

    const update = nextSnapshot => {
      snapshot = nextSnapshot || {}
      // 诊断开关只能从快照进来：主题文档是 srcdoc，读不到面板 URL 上的查询串，
      // 也读不到父窗口（SecurityError）。详见文件顶部关于 `diagOn` 的说明。
      diagOn = Boolean(snapshot.diagnostics && snapshot.diagnostics.showClickTrace)
      const presentation = snapshot.presentation || {}
      root.dataset.layout = presentation.layout === 'directory' ? 'directory' : 'standard'
      page.classList.toggle('yin-page--directory', presentation.layout === 'directory')
      // The pre-theme build switched the item grid to a 200px-minimum card grid
      // for the "detailed" icon style; the marker lets the stylesheet do the same.
      page.classList.toggle('yin-page--info', presentation.layout !== 'directory' && presentation.iconStyle === 'info')
      page.style.setProperty('--yin-viewport-height', `${viewportHeight()}px`)
      page.style.setProperty('--yin-content-max-width', `${presentation.content?.maxWidth || 1200}${presentation.content?.maxWidthUnit || 'px'}`)
      page.style.setProperty('--yin-margin-x', `${presentation.content?.marginX || 0}px`)
      page.style.setProperty('--yin-content-top', `${(viewportWidth() * (presentation.content?.marginTopPercent || 0)) / 100}px`)
      page.style.setProperty('--yin-content-bottom', `${(viewportWidth() * (presentation.content?.marginBottomPercent || 0)) / 100}px`)
      page.style.setProperty('--yin-icon-text-color', presentation.iconTextColor || '#ffffff')
      const reservedHeight = Number(presentation.monitor?.reservedHeight)
      page.style.setProperty('--yin-monitor-reserved-height', `${Number.isFinite(reservedHeight) && reservedHeight > 0 ? reservedHeight : 0}px`)
      updateLogo(presentation)
      updateClock()
      addGroupButton.hidden = !canMutateGroup('create')
      // The remaining entries open Core-owned surfaces, so they need the command
      // permission rather than a Space capability.
      // The command center needs only items.read, which an anonymous public link
      // also holds, but the Core refuses to open it without a session.
      commandCenterButton.hidden = !hasPermission('items.read') || presentation.signedIn !== true
      // `ui.openCoreSurface` is a top-level Theme API method, not a command:
      // dispatching it through commands.execute is rejected as an unknown
      // command, which is why this entry used to do nothing.
      styleButton.hidden = !hasPermission('preferences.read')
      updateSearch(presentation)
      updateFooter(presentation.footerHtml || '')
      renderCollection()
      positionCollectionAfterMonitor()
      reportLayout()
    }

    // A Core page the theme renders itself, embedding the Core's own component
    // through the bridge: the theme owns the page, the Core owns the component.
    const coreSurfaceView = (tag, title, testid) => (elementRoot) => {
      elementRoot.replaceChildren()
      const page = document.createElement('section')
      page.setAttribute('data-testid', testid)
      page.style.cssText = 'box-sizing:border-box;min-height:100%;padding:28px 24px;font:400 14px/1.6 system-ui,-apple-system,sans-serif;color:#20282c;background:#f4f7f8'
      const heading = document.createElement('h1')
      heading.textContent = title
      heading.style.cssText = 'margin:0 0 16px;font-size:20px'
      const embedded = document.createElement(tag)
      embedded.setAttribute('data-testid', `${testid}-core`)
      embedded.style.cssText = 'display:block'
      page.append(heading, embedded)
      elementRoot.append(page)
      return { update() {}, unmount() { elementRoot.replaceChildren() } }
    }

    return {
      views: {
        home(elementRoot, _api, initialSnapshot) {
          createShell(elementRoot)
          update(initialSnapshot)
          // The Core's floating edit button bumps `environment.editToken`; toggling
          // on each new value keeps the theme's edit mode in step without a shared
          // flag crossing the sandbox boundary.
          const unsubscribeEditToken = api.events.subscribe('environment.changed', event => {
            const token = Number(event?.payload?.editToken)
            if (!Number.isFinite(token) || token === lastEditToken) return
            lastEditToken = token
            if (editMode) exitEditMode()
            else enterEditMode()
          })
          // The Core monitor layer is positioned with viewport units, so the
          // reservation has to be recomputed whenever the viewport changes.
          const handleViewportResize = () => { positionCollectionAfterMonitor(); reportLayout() }
          const handleDocumentPointerDown = (event) => {
            // Fired on pointerdown, before a picker option's click. Closing while
            // the pointer is on the button or the popup hid the option before it
            // could be chosen, so switching the engine never took effect.
            if (engineButton?.contains(event.target) || engineMenu?.contains(event.target)) return
            setEngineMenuOpen(false)
          }
          window.addEventListener('resize', handleViewportResize)
          document.addEventListener('pointerdown', handleDocumentPointerDown)

          // 诊断：记录文档级事件与「哪一层在最上层」。用来区分两种失败——
          // 事件根本没到主题（文档级也没有记录），还是到了但开窗失败。
          //
          // 监听器无条件挂上，成本是每次事件多几次字符串拼接；只有开关打开时才会往
          // 屏幕上写内容。开关可以在运行中切换，所以不能在这里判断一次就定型。
          const diagSeen = []
          const onDiagEvent = event => {
            if (!diagOn) return
            const target = event.target && event.target.className ? `.${event.target.className}` : ''
            diagSeen.unshift(`${event.type} (trusted=${event.isTrusted}) → ${event.target && event.target.tagName || '?'}${target}`)
            diagSeen.length = Math.min(diagSeen.length, 8)
            const top = (() => {
              try {
                const el = document.elementFromPoint(innerWidth / 2, innerHeight / 2)
                return el ? `${el.tagName}.${el.className}` : 'null'
              } catch { return 'unavailable' }
            })()
            diag([`文档收到的最近事件（新→旧）:`, ...diagSeen, `视口中心最上层元素: ${top}`])
          }
          for (const type of ['pointerdown', 'touchstart', 'touchend', 'pointerup', 'click']) {
            document.addEventListener(type, onDiagEvent, { capture: true })
          }

          return {
            update,
            unmount() {
              unsubscribeEditToken()
              window.removeEventListener('resize', handleViewportResize)
              document.removeEventListener('pointerdown', handleDocumentPointerDown)
              for (const type of ['pointerdown', 'touchstart', 'touchend', 'pointerup', 'click']) {
                document.removeEventListener(type, onDiagEvent, { capture: true })
              }
              if (clockTimer) window.clearInterval(clockTimer)
              if (collectionLayoutFrame) cancelAnimationFrame(collectionLayoutFrame)
              root?.replaceChildren()
              root = undefined
            },
          }
        },
        'theme-settings'(elementRoot, _api, initialSnapshot) {
          // Rendered by the theme, not the Core: the Core only opens the surface
          // and hands over the element. Self-styled because the theme's own
          // stylesheet is scoped to the home container, not this overlay.
          const render = (snapshot) => {
            elementRoot.replaceChildren()
            const panel = document.createElement('section')
            panel.setAttribute('data-testid', 'theme-settings-view')
            panel.style.cssText = 'box-sizing:border-box;min-height:100%;padding:28px 24px;font:400 14px/1.6 system-ui,-apple-system,sans-serif;color:#20282c;background:#f4f7f8'
            const title = document.createElement('h1')
            title.textContent = 'Yin 主题设置'
            title.style.cssText = 'margin:0 0 6px;font-size:20px'
            const intro = document.createElement('p')
            intro.textContent = '此页面由主题渲染（theme-settings 面）。Core 不再渲染它，只提供数据与权限。'
            intro.style.cssText = 'margin:0 0 16px;color:#64737a'
            const space = document.createElement('p')
            space.textContent = `当前空间：${snapshot?.activeSpaceId ?? '-'}`
            space.style.cssText = 'margin:0'
            panel.append(title, intro, space)
            elementRoot.append(panel)
          }
          render(initialSnapshot)
          return {
            update: snapshot => render(snapshot),
            unmount() { elementRoot.replaceChildren() },
          }
        },
        'theme-page'(elementRoot, api, initialSnapshot) {
          // A whole page owned by the theme. The Core only provides the container
          // and the data channel; the layout below is the theme's. It drives the
          // same command layer as the home (`space.select`, `item.open`), which is
          // what makes a surface a real page and not a static panel.
          const render = (snapshot) => {
            elementRoot.replaceChildren()
            const page = document.createElement('section')
            page.setAttribute('data-testid', 'theme-page-view')
            page.style.cssText = 'box-sizing:border-box;min-height:100%;padding:28px 24px;font:400 14px/1.6 system-ui,-apple-system,sans-serif;color:#20282c;background:#f4f7f8'

            const header = document.createElement('div')
            header.style.cssText = 'display:flex;align-items:baseline;justify-content:space-between;gap:12px;margin-bottom:6px'
            const title = document.createElement('h1')
            title.textContent = '空间'
            title.style.cssText = 'margin:0;font-size:20px'
            const homeButton = document.createElement('button')
            homeButton.type = 'button'
            homeButton.textContent = '返回首页'
            homeButton.setAttribute('data-testid', 'theme-page-home')
            homeButton.style.cssText = 'min-height:32px;padding:0 12px;border:1px solid #c8d2d6;border-radius:6px;background:#fff;color:inherit;font:inherit;cursor:pointer'
            homeButton.addEventListener('click', () => {
              void api.navigation.navigate({ view: 'home' }).catch(() => undefined)
            })
            const manageButton = document.createElement('button')
            manageButton.type = 'button'
            manageButton.textContent = '空间管理（主题内嵌 Core）'
            manageButton.setAttribute('data-testid', 'theme-page-open-space-manage')
            manageButton.style.cssText = 'min-height:32px;padding:0 12px;border:1px solid #c8d2d6;border-radius:6px;background:#fff;color:inherit;font:inherit;cursor:pointer'
            manageButton.addEventListener('click', () => {
              // The theme does not contribute `space-manage`, so the Core opens its
              // own surface (the settings modal on the space manager).
              void api.ui.openCoreSurface('space-manage').catch(() => undefined)
            })
            header.append(title, homeButton, manageButton)

            const intro = document.createElement('p')
            intro.textContent = '此整页由主题渲染（theme-page 面），Core 只提供数据与权限。'
            intro.style.cssText = 'margin:0 0 16px;color:#64737a'

            // The Core's own monitor component, embedded through the component bridge
            // (`<yin-system-monitor>`). It mounts the exact Core component, so it
            // renders identically to the Core's own band.
            const monitor = document.createElement('yin-system-monitor')
            monitor.setAttribute('data-testid', 'theme-page-monitor')
            monitor.style.cssText = 'display:block;margin:0 0 22px'

            const list = document.createElement('ul')
            list.style.cssText = 'margin:0 0 22px;padding:0;list-style:none;display:grid;gap:8px'
            for (const space of snapshot?.spaces || []) {
              const row = document.createElement('li')
              row.style.cssText = 'display:flex;align-items:center;justify-content:space-between;gap:12px;padding:12px 14px;border:1px solid #d8e0e3;border-radius:8px;background:#fff'
              const active = String(space.id) === String(snapshot?.activeSpaceId)
              const name = document.createElement('span')
              name.textContent = `${space.name ?? space.id}${active ? '（当前）' : ''}`
              const switchButton = document.createElement('button')
              switchButton.type = 'button'
              switchButton.textContent = active ? '已选' : '切换'
              switchButton.disabled = active
              switchButton.setAttribute('data-testid', `theme-page-space-${space.id}`)
              switchButton.style.cssText = 'min-height:32px;padding:0 12px;border:1px solid #c8d2d6;border-radius:6px;background:#f2f5f6;color:inherit;font:inherit;cursor:pointer'
              switchButton.addEventListener('click', () => {
                void api.commands.execute('space.select', { spaceId: space.id }).catch(() => undefined)
              })
              row.append(name, switchButton)
              list.append(row)
            }

            const itemsHeading = document.createElement('h2')
            itemsHeading.textContent = '书签'
            itemsHeading.style.cssText = 'margin:0 0 8px;font-size:16px'
            const items = snapshot?.items || []
            if (items.length) {
              const itemsList = document.createElement('ul')
              itemsList.style.cssText = 'margin:0;padding:0;list-style:none;display:grid;gap:8px'
              for (const item of items) {
                const row = document.createElement('li')
                const button = document.createElement('button')
                button.type = 'button'
                button.textContent = item.title
                button.setAttribute('data-testid', `theme-page-item-${item.id}`)
                button.style.cssText = 'width:100%;text-align:left;padding:12px 14px;border:1px solid #d8e0e3;border-radius:8px;background:#fff;color:inherit;font:inherit;cursor:pointer'
                button.addEventListener('click', () => {
                  void api.commands.execute('item.open', { itemId: item.id }).catch(() => undefined)
                })
                row.append(button)
                itemsList.append(row)
              }
              page.append(header, intro, monitor, list, itemsHeading, itemsList)
            }
            else {
              const empty = document.createElement('p')
              empty.textContent = '当前空间没有书签。'
              empty.style.cssText = 'margin:0;color:#8a969b'
              page.append(header, intro, monitor, list, itemsHeading, empty)
            }
            elementRoot.append(page)
          }
          render(initialSnapshot)
          return {
            update: snapshot => render(snapshot),
            unmount() { elementRoot.replaceChildren() },
          }
        },
        // Core pages the theme renders itself, embedding the Core's own components.
        'user-info': coreSurfaceView('yin-user-info', '我的信息', 'theme-user-info-view'),
        'space-manage': coreSurfaceView('yin-space-manage', '空间管理', 'theme-space-manage-view'),
        'users': coreSurfaceView('yin-users', '账号管理', 'theme-users-view'),
        'about': coreSurfaceView('yin-about', '关于', 'theme-about-view'),
      },
    }
  },
}
