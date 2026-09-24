const node = (tag, className, text) => {
  const el = document.createElement(tag)
  if (className) el.className = className
  if (text !== undefined) el.textContent = text
  return el
}

export default {
  apiVersion: '1.0.0',
  setup(api) {
    let root
    let status
    const run = async action => {
      try { await action() }
      catch (error) { if (status) status.textContent = error?.message || 'Action failed' }
    }
    const render = snapshot => {
      root.replaceChildren()
      root.className = 'builtin-home'
      const header = node('header', 'home-header')
      const identity = node('div', 'home-identity')
      identity.append(
        node('p', 'home-eyebrow', 'YIN PANEL'),
        node('h1', 'home-title', 'Home'),
        node('p', 'home-detail', snapshot.status === 'loading' ? 'Loading your items…' : `${(snapshot.items || []).length} items`),
      )
      header.append(identity)
      const spaces = node('nav', 'home-spaces')
      spaces.setAttribute('aria-label', 'Spaces')
      for (const space of snapshot.spaces || []) {
        const button = node('button', `space-button${space.id === snapshot.activeSpaceId ? ' is-active' : ''}`, space.name || 'Space')
        button.type = 'button'
        button.setAttribute('aria-current', space.id === snapshot.activeSpaceId ? 'page' : 'false')
        button.addEventListener('click', () => run(() => api.commands.execute('space.select', { spaceId: space.id })))
        spaces.append(button)
      }
      header.append(spaces)
      root.append(header)
      status = node('p', 'home-status')
      status.setAttribute('role', 'status')
      root.append(status)
      const collection = node('main', 'home-groups')
      const items = new Map((snapshot.items || []).map(item => [item.id, item]))
      for (const group of snapshot.groups || []) {
        const section = node('section', 'home-group')
        section.append(node('h2', 'group-title', group.title || 'Untitled group'))
        const list = node('div', 'group-items')
        for (const itemId of group.itemIds || []) {
          const item = items.get(itemId)
          if (!item) continue
          const button = node('button', 'item-button')
          button.type = 'button'
          button.setAttribute('aria-label', `Open ${item.title}`)
          const icon = node('span', 'item-icon')
          icon.setAttribute('aria-hidden', 'true')
          if (item.icon && typeof item.icon === 'object' && item.icon.src) {
            const image = node('img', 'item-icon-image')
            image.src = item.icon.src
            image.alt = ''
            image.loading = 'lazy'
            image.referrerPolicy = 'no-referrer'
            icon.append(image)
            if (item.icon.backgroundColor) icon.style.backgroundColor = item.icon.backgroundColor
          }
          else {
            icon.textContent = item.icon && (item.icon.text || item.icon.fileName) || '↗'
            if (item.icon?.backgroundColor) icon.style.backgroundColor = item.icon.backgroundColor
          }
          const copy = node('span', 'item-copy')
          copy.append(node('strong', 'item-title', item.title), node('span', 'item-description', item.description || ''))
          button.append(icon, copy)
          button.addEventListener('click', () => run(() => api.commands.execute('item.open', { itemId: item.id })))
          list.append(button)
        }
        section.append(list)
        collection.append(section)
      }
      if (snapshot.status === 'loading') collection.append(node('p', 'home-empty', 'Loading items…'))
      else if (!(snapshot.items || []).length) collection.append(node('p', 'home-empty', 'No items in this space'))
      root.append(collection)
    }
    return {
      views: {
        home(element, _api, snapshot) {
          root = element
          render(snapshot)
          return { update: render, unmount() { root.replaceChildren() } }
        },
      },
    }
  },
}
