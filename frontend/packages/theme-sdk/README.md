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

The package exposes framework-independent data contracts, JSON envelope validators, and the shared `createThemeApiClient(host)` factory. Both isolated and direct runtimes use the same API methods and request envelope; the host supplies a transport plus snapshot and environment readers. The iframe runtime embeds this client factory, while a trusted runtime can use the Core direct dispatcher adapter. Neither transport bypasses Core permission checks. The package does not expose Yin Core stores, Vue, Naive UI, or Core DOM structure.

The API lists the authenticated user's accessible Spaces and the current Space's Groups and Items in pages:

Page size defaults to 50 and is limited to 200 records. Cursors are opaque and scoped to the current Theme API context.

```ts
const spaces = await api.spaces.list({ limit: 50 })
const firstSpace = spaces.items[0]
if (firstSpace) await api.commands.execute('space.select', { spaceId: firstSpace.id })
const groups = await api.groups.list({ cursor: nextGroupCursor })
const firstGroup = groups.items[0]
const items = await api.items.list({ groupId: firstGroup?.id, limit: 50 })
const firstItem = items.items[0]
if (firstItem) await api.commands.execute('item.open', { itemId: firstItem.id })
```

List cursors are opaque to themes. Select a Space with `commands.execute('space.select', { spaceId })`; Core then publishes the new Space snapshot. Item DTOs intentionally omit destination URLs; open an Item through `commands.execute('item.open', { itemId })` so Core retains URL and LAN/WAN policy.

Write commands accept Core-owned Item and Group fields. Item URLs are limited to HTTP and HTTPS; an update may omit unchanged fields. `item.delete` and `group.delete` always use Core confirmation UI. `items.reorder` requires the complete Item ID order for its Group, while `groups.reorder` requires the complete sibling ID order and optional `parentId`.

`editor.open` accepts an optional `groupId` only when that Group is in the active Space. Core validates the current Space again when applying a mutation, and Themes never supply the destination URL used by `item.open`.
