# @yin-panel/theme-sdk

Framework-independent TypeScript contracts and runtime validators for Yin-Panel Theme API v1.

Build the distributable JavaScript and declaration files from the frontend directory with `npm run build:theme-sdk`. The package has no runtime dependencies.

```ts
import type { ThemeAPI, ThemeModule } from '@yin-panel/theme-sdk'

const theme: ThemeModule = {
  apiVersion: '1.0.0',
  setup(api: ThemeAPI) {
    return {
      views: {
        home(root, themeApi, snapshot) {
          const heading = document.createElement('h1')
          heading.textContent = snapshot.spaces.find(space => space.id === snapshot.activeSpaceId)?.name || 'Home'
          root.replaceChildren(heading)
          return { unmount: () => root.replaceChildren() }
        },
      },
    }
  },
}

export default theme
```

The package exposes data contracts and JSON envelope validators only. It does not expose Yin Core stores, Vue, Naive UI, or Core DOM structure.

The API lists the authenticated user's accessible Spaces and the current Space's Groups and Items in pages:

```ts
const spaces = await api.spaces.list({ limit: 50 })
const groups = await api.groups.list({ cursor: nextGroupCursor })
const items = await api.items.list({ groupId: groups.items[0]?.id, limit: 50 })
```

List cursors are opaque to themes. Select a Space with `commands.execute('space.select', { spaceId })`; Core then publishes the new Space snapshot. Item DTOs intentionally omit destination URLs; open an Item through `commands.execute('item.open', { itemId })` so Core retains URL and LAN/WAN policy.
