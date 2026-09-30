const element = (tag, className, text) => {
  const value = document.createElement(tag)
  if (className) value.className = className
  if (text !== undefined) value.textContent = text
  return value
}

const textValue = value => typeof value === 'string' ? value : ''

export default {
  apiVersion: '1.0.0',
  setup(api) {
    let root
    let snapshot
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
      groupDialogHeading.textContent = mode === 'edit' ? 'Edit group' : 'Add group'
      groupDialogSubmit.textContent = mode === 'edit' ? 'Save group' : 'Create group'
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
        groupDialogError.textContent = 'Enter a group name.'
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
      const actions = []
      if (canEditItem(item)) {
        actions.push(addActionButton(contextMenu, 'Edit item', 'theme-edit-item', `Edit ${item.title}`, button => {
          contextMenu.hidden = true
          void openEditor(`editor-item-${item.id}`, { itemId: String(item.id) }, button)
        }))
      }
      if (canDeleteItem(item)) {
        actions.push(addActionButton(contextMenu, 'Delete item', 'theme-delete-item', `Delete ${item.title}`, button => {
          contextMenu.hidden = true
          void mutate(`item-delete-${item.id}`, button, 'item.delete', { itemId: String(item.id) })
        }))
      }
      if (canReorderItems(group)) {
        const index = (group.itemIds || []).map(String).indexOf(String(item.id))
        if (index > 0) {
          actions.push(addActionButton(contextMenu, 'Move item up', 'theme-reorder-item-up', `Move ${item.title} up`, button => {
            contextMenu.hidden = true
            reorderItem(group, item, -1, button)
          }))
        }
        if (index >= 0 && index < (group.itemIds || []).length - 1) {
          actions.push(addActionButton(contextMenu, 'Move item down', 'theme-reorder-item-down', `Move ${item.title} down`, button => {
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
      const formatter = new Intl.DateTimeFormat(undefined, {
        hour: '2-digit', minute: '2-digit', second: presentation.clock.showSeconds ? '2-digit' : undefined,
        hourCycle: 'h23',
      })
      clockTime.textContent = formatter.format(now)
      const dateParts = new Intl.DateTimeFormat(undefined, { month: 'numeric', day: 'numeric', weekday: 'long' }).formatToParts(now)
      const part = type => dateParts.find(value => value.type === type)?.value || ''
      clockDate.textContent = `${part('month')}-${part('day')} ${part('weekday')}`
      clock.title = new Intl.DateTimeFormat(undefined, { dateStyle: 'full' }).format(now)
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
      commandCenterButton = addActionButton(actionBar, 'Commands', 'theme-open-command-center', 'Open command center', () => run(() => api.commands.execute('commandCenter.open')))
      addGroupButton = addActionButton(actionBar, 'Add group', 'theme-add-group', 'Add group', () => showGroupDialog('create'))
      styleButton = addActionButton(actionBar, 'Style', 'theme-open-style', 'Open theme style settings', () => run(() => api.ui.openCoreSurface('theme-settings')))

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
      searchInput.placeholder = 'Enter search content'
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
      const titleLabel = element('label', 'yin-dialog-label', 'Group name')
      groupDialogTitle = element('input', 'yin-dialog-input')
      groupDialogTitle.name = 'group-title'
      groupDialogTitle.dataset.testid = 'theme-group-title-input'
      groupDialogTitle.maxLength = 50
      groupDialogTitle.required = true
      titleLabel.append(groupDialogTitle)
      const iconLabel = element('label', 'yin-dialog-label', 'Icon identifier (optional)')
      groupDialogIcon = element('input', 'yin-dialog-input')
      groupDialogIcon.name = 'group-icon'
      groupDialogIcon.dataset.testid = 'theme-group-icon-input'
      groupDialogIcon.maxLength = 240
      iconLabel.append(groupDialogIcon)
      groupDialogError = element('p', 'yin-dialog-error')
      groupDialogError.setAttribute('role', 'alert')
      const dialogActions = element('div', 'yin-dialog-actions')
      const cancelGroupButton = element('button', 'yin-action-button', 'Cancel')
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
          selectedEngineId = engine.id
          setEngineMenuOpen(false)
          updateEngineIndicator()
          submitSearch()
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
      const addItem = addActionButton(controls, 'Add item', 'theme-add-item', 'Add item', button => {
        const groupId = section.dataset.groupId
        void openEditor(`editor-new-item-${groupId}`, { groupId }, button)
      })
      const editGroup = addActionButton(controls, 'Edit group', 'theme-edit-group', 'Edit group', () => {
        const current = (snapshot?.groups || []).find(candidate => String(candidate.id) === section.dataset.groupId)
        if (current) showGroupDialog('edit', current)
      })
      const deleteGroup = addActionButton(controls, 'Delete group', 'theme-delete-group', 'Delete group', button => {
        void mutate(`group-delete-${section.dataset.groupId}`, button, 'group.delete', { groupId: section.dataset.groupId })
      })
      const moveGroupUp = addActionButton(controls, 'Move group up', 'theme-reorder-group-up', 'Move group up', button => {
        const current = (snapshot?.groups || []).find(candidate => String(candidate.id) === section.dataset.groupId)
        if (current) reorderGroup(current, -1, button)
      })
      const moveGroupDown = addActionButton(controls, 'Move group down', 'theme-reorder-group-down', 'Move group down', button => {
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
      const button = element('button', 'yin-item')
      button.type = 'button'
      const icon = element('span', 'yin-item-icon')
      const copy = element('span', 'yin-item-copy')
      const title = element('strong', 'yin-item-title')
      const description = element('span', 'yin-item-description')
      copy.append(title, description)
      button.append(icon, copy)
      button.addEventListener('click', () => run(() => api.commands.execute('item.open', { itemId: button.dataset.itemId })))
      button._yin = { icon, title, description }
      button.addEventListener('contextmenu', event => {
        const item = button._yin.item
        const group = (snapshot?.groups || []).find(candidate => candidate.itemIds?.map(String).includes(button.dataset.itemId))
        if (item && group) openItemMenu(event, item, group)
      })
      button.addEventListener('keydown', event => {
        if (event.key !== 'ContextMenu' && !(event.shiftKey && event.key === 'F10')) return
        const item = button._yin.item
        const group = (snapshot?.groups || []).find(candidate => candidate.itemIds?.map(String).includes(button.dataset.itemId))
        if (!item || !group) return
        openItemMenu({
          preventDefault: () => event.preventDefault(),
          stopPropagation: () => event.stopPropagation(),
          clientX: button.getBoundingClientRect().left,
          clientY: button.getBoundingClientRect().bottom,
        }, item, group)
      })
      return button
    }

    const updateItemNode = (button, item, presentation, layout) => {
      const { icon, title, description } = button._yin
      button.dataset.itemId = String(item.id)
      button.dataset.testid = 'theme-home-item'
      button.setAttribute('aria-label', `Open ${item.title}`)
      title.textContent = item.title || ''
      description.textContent = item.description || ''
      button._yin.item = item
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
      button.classList.toggle('yin-item--info', infoStyle)
      // The detailed style paints the whole card with the item colour and keeps
      // the glyph on a transparent tile, matching the pre-theme build.
      if (infoStyle) {
        if (itemIcon?.backgroundColor) button.style.backgroundColor = itemIcon.backgroundColor
        else button.style.removeProperty('background-color')
        icon.style.removeProperty('background-color')
      } else {
        button.style.removeProperty('background-color')
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
        const mobileStandardGap = !isDirectory && window.matchMedia('(max-width: 640px)').matches
          ? Math.round(viewportHeight() * 0.187) + 30
          : 77
        const desiredTop = reservedHeight + mobileStandardGap
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
            let button = itemNodes.get(String(item.id))
            if (!button) {
              button = createItemNode(item)
              itemNodes.set(String(item.id), button)
            }
            updateItemNode(button, item, presentation, isDirectory ? 'directory' : 'standard')
            list.append(button)
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
        ? 'Loading items…'
        : snapshot.status === 'error'
          ? snapshot.error?.message || 'Could not load items'
          : needle
            ? 'No matching items'
            : 'No items in this space'
      if (snapshot.status === 'loading') collection.setAttribute('aria-busy', 'true')
      else collection.removeAttribute('aria-busy')
      status.textContent = ''
    }

    const update = nextSnapshot => {
      snapshot = nextSnapshot || {}
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

    return {
      views: {
        home(elementRoot, _api, initialSnapshot) {
          createShell(elementRoot)
          update(initialSnapshot)
          // The Core monitor layer is positioned with viewport units, so the
          // reservation has to be recomputed whenever the viewport changes.
          const handleViewportResize = () => { positionCollectionAfterMonitor(); reportLayout() }
          const handleDocumentPointerDown = () => setEngineMenuOpen(false)
          window.addEventListener('resize', handleViewportResize)
          document.addEventListener('pointerdown', handleDocumentPointerDown)
          return {
            update,
            unmount() {
              window.removeEventListener('resize', handleViewportResize)
              document.removeEventListener('pointerdown', handleDocumentPointerDown)
              if (clockTimer) window.clearInterval(clockTimer)
              if (collectionLayoutFrame) cancelAnimationFrame(collectionLayoutFrame)
              root?.replaceChildren()
              root = undefined
            },
          }
        },
      },
    }
  },
}
